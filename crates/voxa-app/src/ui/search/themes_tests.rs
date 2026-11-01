//! The search box in the light and the dark theme: a window of the theme's panel colour, with
//! every word in it readable.

use egui::Event;

use super::box_tests::{pressed, typed};
use super::*;
use crate::ui::seen::{self, Seen};
use crate::ui::theme::{self, ThemeChoice};

/// What the open box drew in `choice` after `typed` was typed into it and the arrow key moved
/// the choice down once, with the palette of the window: a made-up search that finds a tool
/// that cannot be used, which says why, and a part to choose.
fn drawn_in(choice: ThemeChoice, typed_in: &str) -> (theme::Palette, Seen) {
    let state = AppState::default();
    let ctx = seen::context_in(1.0, choice);
    let mut search = Search::default();
    search.open();
    let mut run = |events: Vec<Event>| {
        let ((), seen) = seen::pass(&ctx, [900.0, 600.0], events, |ui| {
            search.show(ui.ctx(), &state);
        });
        seen
    };
    // The box is drawn unseen once, to learn its size.
    let _ = run(Vec::new());
    let _ = run(Vec::new());
    let _ = run(vec![typed(typed_in)]);
    let _ = run(vec![pressed(egui::Key::ArrowDown)]);
    let drawn = run(Vec::new());
    (theme::palette(&ctx), drawn)
}

#[test]
fn the_box_and_its_results_read_in_both_themes() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        // Before anything is typed, and with results beneath the field.
        for typed_in in ["", "ruler", "zzzz-matches-nothing"] {
            let (palette, drawn) = drawn_in(choice, typed_in);
            let words = drawn.words.len();
            assert!(words > 0, "{choice:?} {typed_in:?}: the box is drawn");
            assert!(drawn.drew(palette.panel), "{choice:?}");
            for ground in [palette.panel, palette.raised] {
                let faint = drawn.hard_to_read_on(ground, &[]);
                assert!(
                    faint.is_empty(),
                    "{choice:?} {typed_in:?} on {ground:?}: {faint:?}"
                );
            }
        }
    }
}

#[test]
fn the_chosen_result_is_outlined_in_the_themes_accent() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let (palette, drawn) = drawn_in(choice, "ruler");
        let said = drawn
            .words
            .iter()
            .any(|(word, _)| word.contains("Measure a length"));
        assert!(said, "{choice:?}: a result is listed");
        assert!(drawn.drew(palette.accent), "{choice:?}");
        assert!(drawn.drew(palette.raised), "{choice:?}");
    }
}
