//! The rail drawn with no window behind it: what it holds and in what order, whose scans they
//! are, what opens first, and how little of a long list is drawn.

use egui::{Context, Event, Rect};
use voxa_core::{SeriesId, SeriesInfo, SeriesProblem, StudyId, StudyInfo};
use voxa_dicom::{Index, IndexedSeries, SliceRef};

use super::*;
use crate::action::Action;
use crate::state::Layout;
use crate::testing::{FINE_SCAN, LOCALISER, SCOUT, SINGLE, THICK_SCAN, id, with_odd_series};
use crate::ui::seen::{self, Seen, click_at};

/// How wide the rail is in these passes, unless a test says otherwise.
pub(super) const WIDE: f32 = 240.0;
/// The window the rail is drawn in.
const WINDOW: [f32; 2] = [900.0, 600.0];

/// What one pass of the rail gave and drew.
pub(super) struct Drawn {
    pub actions: Vec<Action>,
    pub requests: Vec<Request>,
    pub seen: Seen,
    /// Where the rail is.
    pub rect: Rect,
}

/// One pass of the rail, open and `wide` across, with these events.
pub(super) fn pass_wide(
    ctx: &Context,
    state: &AppState,
    rail: &mut RailState,
    thumbnails: &Thumbnails,
    wide: f32,
    events: Vec<Event>,
) -> Drawn {
    let mut actions = Vec::new();
    let draw = |ui: &mut Ui| column(ui, wide, true, state, thumbnails, rail, &mut actions);
    let ((rect, requests), seen) = seen::pass(ctx, WINDOW, events, draw);
    Drawn {
        actions,
        requests,
        seen,
        rect,
    }
}

/// One pass of the rail, open and [`WIDE`] across, with no thumbnails.
pub(super) fn pass(
    ctx: &Context,
    state: &AppState,
    rail: &mut RailState,
    events: Vec<Event>,
) -> Drawn {
    pass_wide(ctx, state, rail, &Thumbnails::default(), WIDE, events)
}

/// A press and a release of the main button at the middle of `rect`, a pass each, the last
/// of which is given.
pub(super) fn click(ctx: &Context, state: &AppState, rail: &mut RailState, rect: Rect) -> Drawn {
    let [press, release] = click_at(rect.center());
    let _ = pass(ctx, state, rail, press);
    pass(ctx, state, rail, release)
}

/// What a screen reader calls the card of a series, without whether it is selected or has a
/// problem, which are said after it.
pub(super) fn card_name(info: &SeriesInfo) -> String {
    card::name_of(info)
}

/// A made-up series of 4 by 4 pixels, numbered `number`, of this modality and description.
pub(super) fn series(number: i32, modality: &str, description: &str, slices: usize) -> SeriesInfo {
    SeriesInfo {
        id: SeriesId(format!("1.2.9.{number}")),
        number,
        description: description.to_owned(),
        modality: modality.to_owned(),
        rows: 4,
        cols: 4,
        slices,
        spacing_mm: [1.0, 1.0, 5.0],
        ..SeriesInfo::default()
    }
}

/// A made-up export: for each study, the identifier and name of whose it is, and its series.
pub(super) fn export(studies: Vec<(&str, &str, Vec<SeriesInfo>)>) -> AppState {
    let mut index = Index::default();
    for (at, (who, name, series)) in studies.into_iter().enumerate() {
        for info in &series {
            let slices = (0..info.slices).map(|place| SliceRef {
                entry: format!("{}/{place}.dcm", info.id.0),
                frame: None,
            });
            index.series.push(IndexedSeries {
                info: info.clone(),
                slices: slices.collect(),
            });
        }
        index.studies.push(StudyInfo {
            id: StudyId(format!("1.2.8.{at}")),
            date: Some(format!("2020010{}", at + 1)),
            description: format!("made-up study {}", at + 1),
            patient_name: Some(name.to_owned()),
            patient_id: Some(who.to_owned()),
            series,
        });
    }
    AppState {
        index: Some(index),
        ..AppState::default()
    }
}

/// Where the card of `info` was drawn.
pub(super) fn card_at(seen: &Seen, info: &SeriesInfo) -> Rect {
    let name = card_name(info);
    seen.place_starting(&name)
        .unwrap_or_else(|| panic!("no card {name}: {:?}", seen.names))
}

/// A made-up series of thirty slices, numbered `number`, with this description and problem.
pub(super) fn faulted(number: i32, description: &str, problem: SeriesProblem) -> SeriesInfo {
    let mut info = series(number, "MR", description, 30);
    info.problems = vec![problem];
    info
}

/// What stops a series being read: a made-up compression.
pub(super) fn unreadable() -> SeriesProblem {
    SeriesProblem::UnsupportedTransferSyntax("made-up".to_owned())
}

/// The top of the card of `info`, as a screen reader is told it.
pub(super) fn top_of(seen: &Seen, info: &SeriesInfo) -> Option<f32> {
    seen.place_starting(&card_name(info)).map(|rect| rect.top())
}

fn info_of<'a>(state: &'a AppState, series: &str) -> &'a SeriesInfo {
    &state.series(&id(series)).expect("listed").info
}

#[test]
fn scans_come_before_other_files_which_start_closed() {
    let state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    let index = state.index.as_ref().expect("open");
    let people = crate::people::people_of(index, false);
    let order = |other_open| {
        let asked = rows::Asked {
            index,
            people: &people,
            shown: None,
            other_open,
            show_patient: false,
        };
        let rows = rows::rows(&asked).into_iter();
        let named = rows.map(|row| match row {
            Row::Card { study, series } => index.studies[study].series[series].id.0.clone(),
            Row::Other(count) => format!("other files: {count}"),
            heading => format!("{heading:?}"),
        });
        named.collect::<Vec<_>>()
    };
    let scans = [THICK_SCAN, FINE_SCAN, SCOUT, SINGLE].map(str::to_owned);
    let heading = "other files: 1".to_owned();
    assert_eq!(
        order(false),
        [&scans[..], std::slice::from_ref(&heading)].concat()
    );
    let open = [&scans[..], &[heading, LOCALISER.to_owned()]].concat();
    assert_eq!(order(true), open);
    // As drawn: the scans in that order, then the heading of the other files, closed.
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let tops: Vec<f32> = [THICK_SCAN, FINE_SCAN, SCOUT, SINGLE]
        .iter()
        .map(|series| top_of(&drawn.seen, info_of(&state, series)).expect("drawn"))
        .collect();
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    let other = drawn
        .seen
        .place_of("Other files, 1")
        .expect("the other files' heading");
    assert!(tops.iter().all(|top| *top < other.top()));
    let locating = info_of(&state, LOCALISER);
    assert_eq!(top_of(&drawn.seen, locating), None, "closed");
    // Its heading opens it.
    let opened = click(&ctx, &state, &mut rail, other);
    assert!(rail.other_open);
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let below = top_of(&drawn.seen, locating).expect("open");
    assert!(below > other.top(), "{below} under {other:?}");
    assert_eq!((opened.actions, opened.requests), (vec![], vec![]));
}

#[test]
fn clicking_a_card_shows_that_series_in_the_active_viewport() {
    let mut state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    (state.layout, state.active) = (Layout::TwoAcross, 1);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let card = drawn
        .seen
        .place_starting(&card_name(info_of(&state, FINE_SCAN)));
    let clicked = click(&ctx, &state, &mut rail, card.expect("drawn"));
    let shown = Action::SelectSeries {
        viewport: 1,
        series: id(FINE_SCAN),
    };
    assert_eq!(clicked.actions, [shown]);
}

#[test]
fn with_nothing_open_the_rail_asks_for_an_export() {
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &AppState::default(), &mut rail, Vec::new());
    let asks = drawn
        .seen
        .words
        .iter()
        .any(|(word, _)| word.starts_with("Open a scan export"));
    assert!(asks, "{:?}", drawn.seen.words);
}

#[test]
fn only_cards_in_sight_are_laid_out() {
    let many =
        (1..=500).map(|number| series(number, "MR", &format!("made-up series {number}"), 20));
    let state = export(vec![("MADE-UP-1", "Made^Up", many.collect())]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    // A card laid out is told to a screen reader, whether or not any of it is painted.
    let infos = &state.index.as_ref().expect("open").series;
    let names: Vec<String> = infos.iter().map(|listed| card_name(&listed.info)).collect();
    let told = drawn
        .seen
        .names
        .iter()
        .filter(|(name, _)| names.contains(name));
    let laid = told.count();
    let drawn_words = drawn.seen.words.iter();
    let painted = drawn_words
        .filter(|(word, _)| word.starts_with("made-up series"))
        .count();
    let in_sight = (drawn.rect.height() / (card::HEIGHT + rows::GAP)).ceil();
    assert!(laid >= 1 && painted >= 1, "some are drawn");
    assert!(
        laid as f32 <= in_sight + 2.0,
        "{laid} cards for {in_sight} in sight"
    );
    assert!(painted as f32 <= in_sight + 2.0, "{painted} painted");
}

#[test]
fn a_folded_rail_is_a_strip_that_opens_it() {
    let state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let mut fold = |events| {
        let mut actions = Vec::new();
        let thumbnails = Thumbnails::default();
        let draw = |ui: &mut Ui| {
            column(
                ui,
                STRIP,
                false,
                &state,
                &thumbnails,
                &mut rail,
                &mut actions,
            )
        };
        seen::pass(&ctx, WINDOW, events, draw)
    };
    let ((rect, _), seen) = fold(Vec::new());
    assert!((rect.width() - STRIP).abs() < 0.5, "{rect:?}");
    let named = commands::find("rail.fold").expect("listed").name;
    let strip = seen.place_of(named).expect("named as its action is");
    assert!(
        strip.height() > rect.height() / 2.0,
        "the strip is one tall button: {strip:?}"
    );
    assert!(
        seen.words.iter().all(|(word, _)| word == "»"),
        "{:?}",
        seen.words
    );
    let [press, release] = click_at(strip.center() + egui::vec2(0.0, 100.0));
    let _ = fold(press);
    let ((_, asked), _) = fold(release);
    assert_eq!(asked, [Request::FoldRail]);
}

#[test]
fn the_rows_are_worked_out_again_only_when_what_they_come_from_changes() {
    let mut state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = |state: &AppState, rail: &mut RailState| {
        let _ = pass(&ctx, state, rail, Vec::new());
        rail.builds
    };
    assert_eq!(drawn(&state, &mut rail), 1);
    assert_eq!(
        drawn(&state, &mut rail),
        1,
        "drawn again with nothing changed"
    );
    state.show_patient = true;
    assert_eq!(drawn(&state, &mut rail), 2, "the patient's details shown");
    rail.other_open = true;
    assert_eq!(drawn(&state, &mut rail), 3, "the other files opened");
    let series = id(FINE_SCAN);
    crate::reducer::reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series,
        },
    );
    assert_eq!(drawn(&state, &mut rail), 4, "another series shown");
    crate::reducer::reduce(&mut state, Action::IndexProgress(with_odd_series()));
    assert_eq!(drawn(&state, &mut rail), 5, "the index read again");
    assert_eq!(drawn(&state, &mut rail), 5);
}

#[test]
fn an_open_rail_has_a_button_at_its_top_that_folds_it_as_the_panels_does() {
    let state = AppState {
        index: Some(with_odd_series()),
        ..AppState::default()
    };
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let named = commands::find("rail.fold").expect("listed").name;
    let button = drawn.seen.place_of(named).expect("named as its action is");
    assert!(
        drawn.rect.contains_rect(button),
        "{button:?} in {:?}",
        drawn.rect
    );
    assert!(
        button.top() < drawn.rect.top() + 40.0,
        "at the top: {button:?}"
    );
    assert!(
        button.center().x > drawn.rect.center().x,
        "at the right: {button:?}"
    );
    // Nothing else is under it: the cards begin below.
    for (name, at) in &drawn.seen.names {
        assert!(
            name == named || at.top() >= button.bottom() - 0.5,
            "{name:?} at {at:?} is over the button at {button:?}"
        );
    }
    // A click on it asks to fold the rail, and nothing more.
    let clicked = click(&ctx, &state, &mut rail, button);
    assert_eq!(clicked.requests, [Request::FoldRail]);
    assert_eq!(clicked.actions, []);
}
