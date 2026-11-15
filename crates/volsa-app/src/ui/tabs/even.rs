//! Controls that use the width of the panel: rows of buttons shared out evenly across it, and
//! sliders as wide as it leaves beside their words.

use egui::{Align, Layout, Ui};

use super::super::commands::{find, offered};
use crate::state::AppState;

/// The least width a button of an even row is given; past this many to a line, the rest go
/// onto the next.
const LEAST_CELL: f32 = 76.0;

/// The room kept beside a slider for its words, such as "Brightness" or "Speed 10 /s".
const SLIDER_WORDS: f32 = 110.0;

/// The narrowest a slider is drawn, however narrow the panel.
const LEAST_SLIDER: f32 = 100.0;

/// The actions of `ids` that are offered for the scan shown, in their order.
fn shown<'a>(ids: &[&'a str], state: &AppState) -> Vec<&'a str> {
    ids.iter()
        .copied()
        .filter(|id| find(id).is_some_and(|command| offered(command, state)))
        .collect()
}

/// How many buttons go on one line in `width`, of `count` each at least `cell` wide: as many as fit,
/// at least one, and shared evenly between the lines, so that six that fit four to a line go
/// three and three.
pub(super) fn per_line(width: f32, count: usize, gap: f32, cell: f32) -> usize {
    let fit = ((width + gap) / (cell.max(LEAST_CELL) + gap)).floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of buttons on a line, from a width far smaller than usize holds"
    )]
    let fit = fit.max(1.0) as usize;
    let count = count.max(1);
    let lines = count.div_ceil(fit.min(count));
    count.div_ceil(lines)
}

/// The width the widest button of `ids` needs for its words on one line, with its margins.
fn widest(ui: &Ui, ids: &[&str]) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let margins = 2.0 * ui.spacing().button_padding.x + 2.0;
    let painter = ui.painter();
    ids.iter()
        .filter_map(|id| find(id))
        .map(|command| {
            let words = command.label.to_owned();
            painter
                .layout_no_wrap(words, font.clone(), egui::Color32::WHITE)
                .size()
                .x
        })
        .fold(0.0, f32::max)
        + margins
}

/// Draw the buttons of `ids` that are offered for the scan shown, shared out evenly across the
/// panel, each as wide as its share; onto further lines, evenly again, when they do not fit at
/// the least width. `draw` draws the button of one action.
pub(super) fn buttons(
    ui: &mut Ui,
    ids: &[&str],
    state: &AppState,
    mut draw: impl FnMut(&mut Ui, &str),
) {
    let ids = shown(ids, state);
    if ids.is_empty() {
        return;
    }
    let gap = ui.spacing().item_spacing.x;
    let line = per_line(ui.available_width(), ids.len(), gap, widest(ui, &ids));
    for chunk in ids.chunks(line) {
        ui.columns(chunk.len(), |columns| {
            for (column, id) in columns.iter_mut().zip(chunk) {
                column.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                    draw(ui, id);
                });
            }
        });
    }
}

/// Draw `add` as wide as the panel, for a switch or a single button to span it.
pub(super) fn whole_width<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(Layout::top_down_justified(Align::Min), add)
        .inner
}

/// Make the sliders drawn in `ui` as wide as the panel leaves beside their words.
pub(super) fn widen_sliders(ui: &mut Ui) {
    let wide = (ui.available_width() - SLIDER_WORDS).max(LEAST_SLIDER);
    ui.spacing_mut().slider_width = wide;
}

#[cfg(test)]
mod tests {
    use super::per_line;

    #[test]
    fn buttons_share_a_line_as_far_as_they_fit_at_the_least_width() {
        assert_eq!(per_line(400.0, 3, 8.0, 0.0), 3);
        assert_eq!(per_line(400.0, 6, 8.0, 0.0), 3, "four fit: three and three");
        assert_eq!(per_line(400.0, 4, 8.0, 130.0), 2, "wide words: two and two");
        assert_eq!(per_line(160.0, 6, 8.0, 0.0), 2);
        assert_eq!(per_line(10.0, 6, 8.0, 0.0), 1);
        assert_eq!(per_line(400.0, 0, 8.0, 0.0), 1);
    }
}
