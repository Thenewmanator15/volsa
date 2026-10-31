//! What the rail opens: the other files when an export holds no scans, and whenever one of
//! them comes to be shown.

use voxa_core::{SeriesInfo, SeriesProblem};

use super::tests::{card_at, click, export, pass, series, top_of};
use super::*;
use crate::action::Action;
use crate::naming::plain_name;
use crate::reducer::reduce;
use crate::state::{Layout, NoticeKind};
use crate::testing::{LOCALISER, id, with_odd_series};
use crate::ui::seen;

/// The state once an export of these series, of one person, has been read to the end.
fn opened(series: Vec<SeriesInfo>) -> AppState {
    let made = export(vec![("MADE-UP-1", "Made^Up", series)]);
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(made.index.expect("made")));
    reduce(&mut state, Action::IndexDone);
    state
}

#[test]
fn an_export_of_no_scans_opens_other_files_and_shows_the_first() {
    let mut locating = series(3, "MR", "made-up localiser", 3);
    locating.problems = vec![SeriesProblem::MixedOrientations];
    let others = vec![
        series(1, "SC", "made-up page", 1),
        series(2, "DOC", "", 2),
        locating,
    ];
    let state = opened(others);
    let first = id("1.2.9.1");
    assert_eq!(
        state.viewports[0].series,
        Some(first.clone()),
        "the first is shown"
    );
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(rail.other_open, "the other files are open");
    assert!(drawn.seen.place_of("Other files, 3").is_some());
    let page = &state.series(&first).expect("listed").info;
    assert!(top_of(&drawn.seen, page).is_some(), "its card is in sight");
    // A scan that cannot be read yet is still a scan, and opens first.
    let mut unread = series(4, "MR", "made-up t2", 20);
    unread.problems = vec![SeriesProblem::UnsupportedTransferSyntax(
        "made-up".to_owned(),
    )];
    let state = opened(vec![series(1, "SC", "", 1), unread]);
    assert_eq!(state.viewports[0].series, Some(id("1.2.9.4")));
    let said = state
        .notices
        .iter()
        .find(|notice| notice.key == Some("series"));
    assert_eq!(said.map(|notice| notice.kind), Some(NoticeKind::Warning));
}

#[test]
fn showing_one_of_the_other_files_opens_them_and_they_can_be_closed_again() {
    let mut state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let _ = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(!rail.other_open);
    // As search shows a scan, in the active viewport.
    let series = id(LOCALISER);
    reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series,
        },
    );
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(rail.other_open);
    let heading = drawn.seen.place_of("Other files, 1").expect("drawn");
    let _ = click(&ctx, &state, &mut rail, heading);
    let _ = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(!rail.other_open, "closed while the same series is shown");
}
#[test]
fn opening_a_faulted_series_posts_its_sentence() {
    let mut uneven = series(1, "MR", "made-up t2 ax", 30);
    uneven.problems = vec![SeriesProblem::UnevenSpacing, SeriesProblem::MixedSizes];
    let mut unread = series(2, "MR", "made-up flair", 30);
    unread.problems = vec![SeriesProblem::UnsupportedTransferSyntax(
        "made-up".to_owned(),
    )];
    let clean = series(3, "MR", "made-up pd", 30);
    let all = vec![uneven.clone(), unread.clone(), clean.clone()];
    let mut state = export(vec![("MADE-UP-1", "Made^Up", all)]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let mut open = |state: &mut AppState, info: &SeriesInfo| {
        let drawn = pass(&ctx, state, &mut rail, Vec::new());
        let clicked = click(&ctx, state, &mut rail, card_at(&drawn.seen, info));
        for action in clicked.actions {
            reduce(state, action);
        }
        let said = state
            .notices
            .iter()
            .filter(|notice| notice.key == Some("series"));
        said.cloned().collect::<Vec<_>>()
    };
    let said = open(&mut state, &uneven);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].kind, NoticeKind::Warning, "amber, not red");
    let sentence = "The slices are not evenly spaced, so this is shown as a plain stack.";
    assert_eq!(said[0].text, sentence);
    assert_eq!(said[0].details[0], plain_name(&uneven), "says which series");
    assert!(
        said[0].details[1].contains("not all the same size"),
        "and every reason"
    );
    // Another opened in its place says its own, in place of the first.
    let said = open(&mut state, &unread);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].text.contains("cannot read yet (made-up)"),
        "{}",
        said[0].text
    );
    // And once no series shown has anything wrong, nothing is said of one.
    let said = open(&mut state, &clean);
    assert_eq!(said, []);
}

#[test]
fn a_faulted_series_still_shown_beside_keeps_its_notice() {
    let mut uneven = series(1, "MR", "made-up t2 ax", 30);
    uneven.problems = vec![SeriesProblem::UnevenSpacing];
    let clean = series(2, "MR", "made-up pd", 30);
    let mut state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![uneven.clone(), clean.clone()],
    )]);
    state.layout = Layout::TwoAcross;
    let select = |viewport, info: &SeriesInfo| Action::SelectSeries {
        viewport,
        series: info.id.clone(),
    };
    reduce(&mut state, select(0, &uneven));
    reduce(&mut state, select(1, &clean));
    let said = state
        .notices
        .iter()
        .filter(|notice| notice.key == Some("series"));
    assert_eq!(said.count(), 1, "the stack is still shown at the left");
}
