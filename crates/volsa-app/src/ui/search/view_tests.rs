//! The result that is chosen is always in view, in a list too tall for the window, however the
//! person got to it: with the arrows, by typing more, or by opening the box again.

use egui::{Context, Event, Key};

use super::box_tests::{Pass, pass_sized, pressed, typed, window};
use super::*;
use crate::testing::thick_scan_ready;

/// A window too short for the whole list of results, which scrolls.
const SHORT: f32 = 300.0;

/// One pass in the short window.
fn pass(ctx: &Context, search: &mut Search, state: &AppState, events: Vec<Event>) -> Pass {
    pass_sized(ctx, search, state, events, SHORT, |_| {})
}

/// The box opened in the short window, and drawn until it is seen.
fn opened(ctx: &Context, state: &AppState) -> Search {
    let mut search = Search::default();
    search.open();
    pass(ctx, &mut search, state, Vec::new());
    pass(ctx, &mut search, state, Vec::new());
    search
}

/// Press the down arrow `times` times in one pass, then draw once more.
fn arrow_down(ctx: &Context, search: &mut Search, state: &AppState, times: usize) {
    let events = vec![pressed(Key::ArrowDown); times];
    pass(ctx, search, state, events);
    pass(ctx, search, state, Vec::new());
}

/// Whether the result that is chosen is drawn whole inside the part of the box that scrolls.
fn chosen_is_in_view(search: &Search, drawn: &Pass) -> bool {
    let place = search
        .results
        .get(search.chosen)
        .expect("a result is chosen");
    let name = &search.entries.get(*place).expect("listed").name;
    let seen = drawn.places.iter().zip(&drawn.clips);
    let mut named = seen.filter(|((word, _), _)| word.starts_with(name.as_str()));
    named.any(|((_, text), clip)| clip.contains_rect(*text))
}

#[test]
fn typing_brings_the_chosen_result_back_into_view_after_the_list_was_scrolled() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("a")]);
    assert!(search.results.len() > 9, "a list too tall for the window");
    arrow_down(&ctx, &mut search, &state, 8);
    assert_eq!(search.chosen, 8);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    assert!(chosen_is_in_view(&search, &drawn), "following the arrows");
    // The list is scrolled: the first result is out of view now.
    let first = search.chosen;
    search.chosen = 0;
    assert!(
        !chosen_is_in_view(&search, &drawn),
        "scrolled away from the first"
    );
    search.chosen = first;
    // Another letter leaves a list as tall, with the first result chosen.
    pass(&ctx, &mut search, &state, vec![typed("n")]);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    assert!(search.results.len() > 9, "{:?}", search.query);
    assert_eq!(search.chosen, 0);
    assert!(
        chosen_is_in_view(&search, &drawn),
        "after typing: the first result is out of view"
    );
}

#[test]
fn opening_the_box_again_shows_its_first_result() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("a")]);
    arrow_down(&ctx, &mut search, &state, 8);
    pass(&ctx, &mut search, &state, vec![pressed(Key::Escape)]);
    assert!(!search.open);
    // Opened again, a short query leaves the list as tall as before.
    search.open();
    pass(&ctx, &mut search, &state, Vec::new());
    pass(&ctx, &mut search, &state, Vec::new());
    pass(&ctx, &mut search, &state, vec![typed("a")]);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    assert!(search.results.len() > 9);
    assert_eq!(search.chosen, 0);
    assert!(
        chosen_is_in_view(&search, &drawn),
        "after opening again: the first result is out of view"
    );
}

#[test]
fn a_list_made_again_while_the_box_is_open_starts_at_its_first_result() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("a")]);
    arrow_down(&ctx, &mut search, &state, 8);
    // Labels open in the meantime: what can be found changes.
    let (labelled, _) = crate::testing::labelled_head();
    pass(&ctx, &mut search, &labelled, Vec::new());
    let drawn = pass(&ctx, &mut search, &labelled, Vec::new());
    assert_eq!(search.chosen, 0);
    assert!(chosen_is_in_view(&search, &drawn));
}
