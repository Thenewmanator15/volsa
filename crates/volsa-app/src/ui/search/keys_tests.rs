//! What the keys and clicks do in the box: the arrows, Enter and Esc, a click on a result or
//! away from the box, and what is typed.

use egui::{Event, Key, Modifiers, Pos2, Rect};

use super::box_tests::{opened, pass, pass_with, pressed, typed, window};
use super::entries::{Target, entries_for};
use super::matching::ranked;
use super::*;
use crate::testing::thick_scan_ready;
use crate::ui::keys;

#[test]
fn the_box_takes_the_keyboard_while_it_is_open() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    assert!(ctx.egui_wants_keyboard_input(), "the field has focus");
    // Typing the keys of single-letter actions types them into the box.
    let letters = [Key::H, Key::V, Key::R, Key::F, Key::Num1, Key::M, Key::B];
    let mut taken = None;
    let events: Vec<Event> = letters.into_iter().map(pressed).collect();
    pass_with(&ctx, &mut search, &state, events, |ctx| {
        taken = Some(keys::answered(ctx, &state));
    });
    assert_eq!(taken, Some((Vec::new(), Vec::new())));
    pass(&ctx, &mut search, &state, vec![typed("hvrf1mb")]);
    assert_eq!(search.query, "hvrf1mb");
    assert!(search.open);
}

#[test]
fn arrows_move_and_enter_runs_what_is_chosen() {
    let state = thick_scan_ready();
    let entries = entries_for(&state);
    let listed = ranked("interface size", &entries);
    assert!(listed.len() >= 3);
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("interface size")]);
    assert_eq!(search.chosen, 0);
    pass(
        &ctx,
        &mut search,
        &state,
        vec![pressed(Key::ArrowDown), pressed(Key::ArrowDown)],
    );
    assert_eq!(search.chosen, 2);
    pass(&ctx, &mut search, &state, vec![pressed(Key::ArrowUp)]);
    assert_eq!(search.chosen, 1);
    let drawn = pass(&ctx, &mut search, &state, vec![pressed(Key::Enter)]);
    assert_eq!(drawn.chosen.as_ref(), Some(&entries[listed[1]].target));
    assert!(!search.open, "choosing closes the box");
}

#[test]
fn enter_runs_what_the_text_typed_in_the_same_pass_finds() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    let events = vec![typed("ruler"), pressed(Key::Enter)];
    let drawn = pass(&ctx, &mut search, &state, events);
    assert_eq!(drawn.chosen, Some(Target::Command("measure.ruler")));
}

#[test]
fn enter_with_nothing_found_runs_nothing_and_leaves_the_box_open() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("qqqqqq")]);
    let drawn = pass(&ctx, &mut search, &state, vec![pressed(Key::Enter)]);
    assert_eq!(drawn.chosen, None);
    assert!(search.open);
    assert!(
        drawn
            .words
            .iter()
            .any(|word| word.contains("Nothing matches"))
    );
}

#[test]
fn the_choice_goes_back_to_the_first_when_the_query_changes() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("interface size")]);
    pass(&ctx, &mut search, &state, vec![pressed(Key::ArrowDown)]);
    assert_eq!(search.chosen, 1);
    pass(&ctx, &mut search, &state, vec![typed("s")]);
    assert_eq!(search.chosen, 0);
}

#[test]
fn escape_closes_the_box_and_does_nothing_else() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    let mut taken = None;
    let escape = vec![pressed(Key::Escape)];
    let drawn = pass_with(&ctx, &mut search, &state, escape, |ctx| {
        taken = Some(keys::answered(ctx, &state));
    });
    assert_eq!(
        taken,
        Some((Vec::new(), Vec::new())),
        "Esc is not also the key of an action"
    );
    assert_eq!(drawn.chosen, None);
    assert!(!search.open);
    // The keyboard is let go, so that the keys of actions work again.
    pass(&ctx, &mut search, &state, Vec::new());
    assert!(!ctx.egui_wants_keyboard_input());
}

#[test]
fn clicking_outside_the_box_closes_it() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, Vec::new());
    let at = Pos2::new(20.0, 560.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(
        &ctx,
        &mut search,
        &state,
        vec![Event::PointerMoved(at), click(true), click(false)],
    );
    assert!(!search.open);
}

#[test]
fn clicking_a_result_chooses_it() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("ruler")]);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    // The pointer goes to where the first result's name was drawn.
    let named = |(word, _): &&(String, Rect)| word.contains("Measure a length");
    let (_, place) = drawn
        .places
        .iter()
        .find(named)
        .expect("the result is drawn");
    let at = place.center();
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let events = vec![Event::PointerMoved(at), click(true), click(false)];
    let drawn = pass(&ctx, &mut search, &state, events);
    assert_eq!(drawn.chosen, Some(Target::Command("measure.ruler")));
    assert!(!search.open);
}

#[test]
fn the_slash_that_opens_the_box_is_not_typed_into_it() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = Search::default();
    // The pass in which `/` is pressed: the window's keys open the box before it is drawn,
    // and the text the key makes arrives in that same pass.
    search.open();
    pass(
        &ctx,
        &mut search,
        &state,
        vec![pressed(Key::Slash), typed("/")],
    );
    assert_eq!(search.query, "");
    // Typing after that is typed.
    pass(&ctx, &mut search, &state, vec![typed("/")]);
    assert_eq!(search.query, "/");
}

#[test]
fn opening_the_box_again_starts_with_nothing_typed() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("ruler")]);
    assert_eq!(search.query, "ruler");
    pass(&ctx, &mut search, &state, vec![pressed(Key::Escape)]);
    search.open();
    assert_eq!(search.query, "");
    assert_eq!(search.chosen, 0);
}
