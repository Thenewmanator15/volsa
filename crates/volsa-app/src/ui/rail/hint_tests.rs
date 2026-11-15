//! The quiet line above the scans that says a card can be dragged onto a view.

use egui::{Rect, TextStyle};

use super::tests::{WIDE, export, pass, pass_wide, series};
use super::*;
use crate::pictures::Pictures;
use crate::state::Layout;
use crate::ui::seen;
use crate::ui::shares::LIST_LEAST;

/// Two scans of one study, shown in `layout`.
fn two_scans_in(layout: Layout) -> AppState {
    let scans = vec![
        series(1, "MR", "made-up t1", 20),
        series(2, "MR", "made-up t2", 20),
    ];
    let mut state = export(vec![("MADE-UP-1", "Made^Up", scans)]);
    state.layout = layout;
    state
}

/// Where the hint was drawn, if it was.
fn hint_in(words: &[(String, Rect)]) -> Option<Rect> {
    words
        .iter()
        .find_map(|(said, at)| (said == DRAG_HINT).then_some(*at))
}

#[test]
fn the_hint_is_said_while_there_is_more_than_one_view_to_drag_a_scan_to() {
    let ctx = seen::context(1.0);
    for (layout, said) in [
        (Layout::One, false),
        (Layout::TwoAcross, true),
        (Layout::TwoByTwo, true),
    ] {
        let mut rail = RailState::default();
        let drawn = pass(&ctx, &two_scans_in(layout), &mut rail, Vec::new());
        let found = hint_in(&drawn.seen.words);
        assert_eq!(found.is_some(), said, "{layout:?}: {:?}", drawn.seen.words);
    }
    // The 3D view alone has no view of slices to drag a scan to.
    let mut alone = two_scans_in(Layout::TwoByTwo);
    alone.pictures = Pictures::Scene;
    alone.scene.status = crate::scene::SceneStatus::Ready;
    let drawn = pass(&ctx, &alone, &mut RailState::default(), Vec::new());
    assert_eq!(hint_in(&drawn.seen.words), None);
    // With nothing to drag, nothing is said of dragging.
    let empty = export(Vec::new());
    let drawn = pass(&ctx, &empty, &mut RailState::default(), Vec::new());
    assert_eq!(hint_in(&drawn.seen.words), None);
}

#[test]
fn the_hint_is_quiet_above_the_cards_and_wraps_in_the_narrowest_rail() {
    let ctx = seen::context(1.0);
    let state = two_scans_in(Layout::TwoAcross);
    let thumbnails = Thumbnails::default();
    let mut rail = RailState::default();
    let wide = pass_wide(&ctx, &state, &mut rail, &thumbnails, WIDE, Vec::new());
    let narrow = pass_wide(&ctx, &state, &mut rail, &thumbnails, LIST_LEAST, Vec::new());
    let hint = hint_in(&narrow.seen.words).expect("said");
    // Inside the rail, whole, on more than one line rather than cut short.
    assert!(
        narrow.rect.contains_rect(hint),
        "{hint:?} in {:?}",
        narrow.rect
    );
    let line = ctx.global_style().text_styles[&TextStyle::Small].size;
    assert!(hint.height() > 1.5 * line, "one line: {hint:?}");
    // Above the first card, and in the quiet colour.
    let card = super::tests::card_at(&narrow.seen, &series(1, "MR", "made-up t1", 20));
    assert!(hint.bottom() <= card.top(), "{hint:?} over {card:?}");
    let quiet = theme::palette(&ctx).quiet_text;
    let ink = narrow.seen.inks.iter().find(|(said, _)| said == DRAG_HINT);
    assert_eq!(ink.map(|(_, colour)| *colour), Some(quiet));
    let wide_hint = hint_in(&wide.seen.words).expect("said in a wider rail too");
    assert!(wide.rect.contains_rect(wide_hint));
}
