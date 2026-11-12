//! The tabs of the panel in the light and the dark theme: every word in them readable.

use super::*;
use crate::state::AppState;
use crate::testing::thick_scan_ready;
use crate::ui::seen;
use crate::ui::theme::ThemeChoice;

#[test]
fn every_tab_reads_in_both_themes_with_a_scan_open_and_with_none() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let ctx = seen::context_in(1.0, choice);
        let palette = theme::palette(&ctx);
        // What is chosen is drawn in the selection's colour on the selection's own fill, which
        // `theme`'s tests hold to the same contrast.
        let rgb = |colour: egui::Color32| {
            let [red, green, blue, _] = colour.to_srgba_unmultiplied();
            [red, green, blue]
        };
        let chosen_ink = rgb(ctx.global_style().visuals.selection.stroke.color);
        for state in [AppState::default(), thick_scan_ready()] {
            for tab in Tab::ALL {
                let mut kept = Kept::default();
                let ((), drawn) = seen::pass(&ctx, [360.0, 900.0], Vec::new(), |ui| {
                    egui::Panel::right("tab panel").show(ui, |ui| {
                        show(ui, tab, &state, &mut kept, &mut Vec::new());
                    });
                });
                assert!(!drawn.words.is_empty(), "{choice:?} {tab:?}: it is drawn");
                for ground in [palette.panel, palette.raised] {
                    let faint = drawn.hard_to_read_on(ground, &[]);
                    let faint: Vec<_> = faint
                        .iter()
                        .filter(|(_, ink)| rgb(*ink) != chosen_ink)
                        .collect();
                    assert!(
                        faint.is_empty(),
                        "{choice:?} {tab:?} on {ground:?}: {faint:?}"
                    );
                }
            }
        }
    }
}
