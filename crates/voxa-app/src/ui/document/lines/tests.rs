//! A long report drawn: only the lines in sight are laid out, and what was made for them is
//! kept, not made again.

use egui::{Context, Event, Modifiers, MouseWheelUnit, Pos2, vec2};

use super::super::tests::{WINDOW, line, opened, opened_with, pass_with, where_is};
use super::*;
use crate::state::OpenDocument;
use crate::testing::documents::{NAMED, REPORT, document_files, named_report_file};
use crate::testing::{indexed, three_series_files};
use crate::ui::seen::{self, Seen};

/// How many lines the longest report holds.
const LINES: usize = 5_000;

/// A report of this many lines, each `Item n: ...`, nested up to four deep.
fn long_report(count: usize) -> Report {
    let lines = (0..count).map(|n| {
        let text = format!("Item {n}: a made-up line of text that runs on, as long as a real one.");
        line(u8::try_from(n % 5).expect("small"), None, &text)
    });
    Report {
        lines: lines.collect(),
        cut_short: false,
    }
}

/// The numbers of the `Item n` lines a pass drew, in the order they were drawn.
fn items_drawn(seen: &Seen) -> Vec<usize> {
    let words = seen.words.iter().map(|(word, _)| word.as_str());
    let numbers = words.filter_map(|word| word.strip_prefix("Item "));
    let numbers = numbers.filter_map(|rest| rest.split(':').next());
    numbers.filter_map(|number| number.parse().ok()).collect()
}

/// Turn the wheel by `by` points with the pointer over the lines, then draw passes with nothing
/// happening until the scrolling has settled.
fn scroll(ctx: &Context, state: &AppState, kept: &mut Kept, by: f32) -> Seen {
    let over = Pos2::new(WINDOW[0] / 2.0, WINDOW[1] / 2.0);
    let wheel = Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, by),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    };
    let mut drawn = pass_with(ctx, state, kept, vec![Event::PointerMoved(over), wheel]);
    let mut before = items_drawn(&drawn.seen);
    for _ in 0..200 {
        drawn = pass_with(ctx, state, kept, Vec::new());
        let now = items_drawn(&drawn.seen);
        if now == before {
            break;
        }
        before = now;
    }
    drawn.seen
}

#[test]
fn a_long_report_lays_out_only_the_lines_in_sight() {
    let state = opened(REPORT, Some(Ok(long_report(LINES))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let drawn = pass_with(&ctx, &state, &mut kept, Vec::new());
    let items = items_drawn(&drawn.seen);
    // A few dozen of the five thousand, from the first, one after the other.
    assert!(items.len() > 5 && items.len() < 60, "{}", items.len());
    assert_eq!(items, (0..items.len()).collect::<Vec<_>>());
    assert_eq!(kept.drawn_count, items.len(), "laid out are those drawn");
    assert_eq!(kept.prepared_count, items.len(), "and only those prepared");
    // What the screen reader is told is no more than that.
    let told = drawn
        .seen
        .names
        .iter()
        .filter(|(name, _)| name.starts_with("Item "));
    assert!(told.count() <= items.len());
}

#[test]
fn a_frame_with_nothing_changed_makes_nothing_again() {
    let state = opened(REPORT, Some(Ok(long_report(LINES))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let _ = pass_with(&ctx, &state, &mut kept, Vec::new());
    let (prepared, heights) = (kept.prepared_count, kept.heights.clone());
    for _ in 0..3 {
        let drawn = pass_with(&ctx, &state, &mut kept, Vec::new());
        assert_eq!(kept.prepared_count, prepared, "no line is prepared again");
        assert_eq!(kept.heights, heights, "nor measured again");
        assert_eq!(items_drawn(&drawn.seen).first(), Some(&0));
    }
}

#[test]
fn scrolling_brings_other_lines_in_sight_and_makes_only_those() {
    let state = opened(REPORT, Some(Ok(long_report(LINES))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let top = pass_with(&ctx, &state, &mut kept, Vec::new());
    let on_top = items_drawn(&top.seen).len();
    let down = items_drawn(&scroll(&ctx, &state, &mut kept, -3000.0));
    assert!(
        down.first().is_some_and(|first| *first > on_top),
        "{down:?}"
    );
    assert!(down.len() < 60, "{}", down.len());
    // Only lines up to the last in sight were prepared, those passed on the way among them,
    // and a small part of the report.
    let last = down.last().copied().unwrap_or(0);
    assert!(
        kept.prepared_count <= last + 1,
        "{} prepared",
        kept.prepared_count
    );
    assert!(
        kept.prepared_count < LINES / 10,
        "{} prepared",
        kept.prepared_count
    );
    // Back at the top the first lines are drawn again with nothing made again for them.
    let prepared = kept.prepared_count;
    let back = items_drawn(&scroll(&ctx, &state, &mut kept, 30_000.0));
    assert_eq!(back.first(), Some(&0));
    assert_eq!(kept.prepared_count, prepared, "kept from the first time");
}

#[test]
fn the_last_line_can_be_reached() {
    let state = opened(REPORT, Some(Ok(long_report(LINES))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let _ = pass_with(&ctx, &state, &mut kept, Vec::new());
    let mut last = Vec::new();
    for _ in 0..30 {
        last = items_drawn(&scroll(&ctx, &state, &mut kept, -400_000.0));
        if last.last() == Some(&(LINES - 1)) {
            break;
        }
    }
    assert_eq!(last.last(), Some(&(LINES - 1)), "{last:?}");
}

#[test]
fn a_line_that_wraps_takes_room_for_its_rows_and_pushes_the_next_down() {
    let long = "a made-up sentence that goes on and on ".repeat(30);
    let report = Report {
        lines: vec![line(0, None, &long), line(0, None, "A short one.")],
        cut_short: false,
    };
    let state = opened(REPORT, Some(Ok(report)));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let drawn = pass_with(&ctx, &state, &mut kept, Vec::new());
    let (first, next) = (
        where_is(&drawn.seen, &long),
        where_is(&drawn.seen, "A short one."),
    );
    assert!(first.height() > 3.0 * next.height(), "{first:?} {next:?}");
    assert!(first.bottom() <= next.top(), "{first:?} over {next:?}");
    // And the room it was measured to take is kept: more than the guess for one row, whole.
    assert!(
        kept.heights[0] > 3.0 * kept.heights[1],
        "{:?}",
        kept.heights
    );
    assert!((kept.height() - kept.heights.iter().sum::<f32>()).abs() < 0.5);
}

#[test]
fn a_change_of_width_measures_the_lines_again_but_prepares_nothing_again() {
    let state = opened(REPORT, Some(Ok(long_report(200))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let _ = pass_with(&ctx, &state, &mut kept, Vec::new());
    let (prepared, narrow) = (kept.prepared_count, kept.heights.clone());
    // A window half as wide.
    let (width, high) = (WINDOW[0] / 2.0, WINDOW[1]);
    let area = egui::Rect::from_min_size(Pos2::ZERO, vec2(width, high));
    let open = state.document.as_ref().expect("open");
    let mut actions = Vec::new();
    let draw = |ui: &mut Ui| super::super::show(ui, area, &state, open, &mut kept, &mut actions);
    let _ = seen::pass(&ctx, [width, high], Vec::new(), draw);
    assert_eq!(kept.prepared_count, prepared);
    assert_ne!(
        kept.heights, narrow,
        "what is guessed or measured is for the new width"
    );
}

/// What a screen reader has to tell each line of the `Item n` kind apart by, as drawn in a
/// pass with nothing happening, by n.
fn identities(
    ctx: &Context,
    state: &AppState,
    kept: &mut Kept,
) -> std::collections::BTreeMap<usize, egui::accesskit::NodeId> {
    let area = egui::Rect::from_min_size(Pos2::ZERO, vec2(WINDOW[0], WINDOW[1]));
    let input = egui::RawInput {
        screen_rect: Some(area),
        ..egui::RawInput::default()
    };
    let open = state.document.as_ref().expect("a document is open");
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(input, |ui| {
        let _ = super::super::show(ui, area, state, open, kept, &mut actions);
    });
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update;
    let update = update.expect("the interface is described");
    let item = |node: &egui::accesskit::Node| {
        let label = node.role() == egui::accesskit::Role::Label;
        let words = node.value().filter(|_| label)?.strip_prefix("Item ")?;
        words.split(':').next()?.parse::<usize>().ok()
    };
    let found = update
        .nodes
        .iter()
        .filter_map(|(id, node)| Some((item(node)?, *id)));
    found.collect()
}

#[test]
fn a_line_keeps_the_identity_a_screen_reader_knows_it_by_as_the_report_scrolls() {
    let state = opened(REPORT, Some(Ok(long_report(LINES))));
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let _ = pass_with(&ctx, &state, &mut kept, Vec::new());
    let before = identities(&ctx, &state, &mut kept);
    // A few lines on, so most of those in sight stay in sight but are no longer the first.
    let _ = scroll(&ctx, &state, &mut kept, -90.0);
    let after = identities(&ctx, &state, &mut kept);
    assert!(
        before.contains_key(&0) && !after.contains_key(&0),
        "scrolled past the first"
    );
    let shared: Vec<usize> = before
        .keys()
        .filter(|n| after.contains_key(n))
        .copied()
        .collect();
    assert!(shared.len() > 5, "{shared:?}");
    for n in shared {
        assert_eq!(before[&n], after[&n], "line {n}");
    }
}

#[test]
fn a_second_report_opens_at_its_top_not_where_the_first_was_left() {
    let files = [
        three_series_files(),
        document_files(),
        vec![named_report_file()],
    ];
    let mut state = opened_with(
        indexed(files.concat()),
        REPORT,
        Some(Ok(long_report(LINES))),
    );
    let (ctx, mut kept) = (seen::context(1.0), Kept::default());
    let _ = pass_with(&ctx, &state, &mut kept, Vec::new());
    let down = items_drawn(&scroll(&ctx, &state, &mut kept, -3000.0));
    assert!(down.first().is_some_and(|first| *first > 10), "{down:?}");
    // Another report opened in its place starts from its own first line.
    let entry = NAMED.to_owned();
    let report = Some(Ok(long_report(LINES)));
    state.document = Some(OpenDocument { entry, report });
    let drawn = pass_with(&ctx, &state, &mut kept, Vec::new());
    assert_eq!(items_drawn(&drawn.seen).first(), Some(&0));
}
