//! The rail in the light and the dark theme: on its panel, with every word readable.

use egui::Event;

use super::tests::{card_at, export, faulted, pass, series, unreadable};
use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::ui::seen;
use crate::ui::theme::{self, ThemeChoice};

/// What a pass of the rail drew in `choice`, with the pointer resting on a card as well as one
/// card the one shown: the two people of an export, one of whose scans has a problem.
fn drawn_in(choice: ThemeChoice) -> (theme::Palette, seen::Seen) {
    let clean = series(1, "MR", "made-up t2 ax", 30);
    let faulty = faulted(2, "made-up flair", unreadable());
    let other = series(3, "CT", "made-up head", 40);
    let mut state = export(vec![
        ("MADE-UP-1", "Made^Up", vec![clean.clone(), faulty]),
        ("MADE-UP-2", "Also^Made", vec![other]),
    ]);
    let shown = Action::SelectSeries {
        viewport: 0,
        series: clean.id.clone(),
    };
    reduce(&mut state, shown);
    let (ctx, mut rail) = (seen::context_in(1.0, choice), RailState::default());
    let first = pass(&ctx, &state, &mut rail, Vec::new());
    // With the pointer on the card of the second scan, which is then raised.
    let faulty = state.index.as_ref().expect("open").series[1].info.clone();
    let on_it = Event::PointerMoved(card_at(&first.seen, &faulty).center());
    let _ = pass(&ctx, &state, &mut rail, vec![on_it.clone()]);
    let drawn = pass(&ctx, &state, &mut rail, vec![on_it]);
    (theme::palette(&ctx), drawn.seen)
}

#[test]
fn the_rail_is_on_the_themes_panel_and_every_word_in_it_reads_in_both() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let (palette, seen) = drawn_in(choice);
        let colours = &seen.colours;
        assert!(colours.contains(&palette.panel), "{choice:?}: the panel");
        assert!(
            colours.contains(&palette.raised),
            "{choice:?}: a card raised"
        );
        assert!(
            colours.contains(&palette.accent),
            "{choice:?}: the card shown"
        );
        assert!(
            colours.contains(&palette.warning),
            "{choice:?}: the warning"
        );
        assert!(
            colours.contains(&palette.quiet_text),
            "{choice:?}: quiet words"
        );
        // The `!` of a mark is on the picture, which is dark in both.
        for ground in [palette.panel, palette.raised] {
            let faint = seen.hard_to_read_on(ground, &["!"]);
            assert!(faint.is_empty(), "{choice:?} on {ground:?}: {faint:?}");
        }
    }
}

#[test]
fn the_mark_on_a_picture_is_amber_on_dark_in_both_themes() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let (_, seen) = drawn_in(choice);
        let mark = seen.inks.iter().find(|(said, _)| said == "!");
        let (_, ink) = mark.expect("a series with a problem is marked");
        assert_eq!(*ink, theme::WARNING, "{choice:?}");
        assert!(seen.colours.contains(&theme::BACKGROUND), "{choice:?}");
    }
}
