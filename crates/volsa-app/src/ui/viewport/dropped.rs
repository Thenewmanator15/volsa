//! A series carried from its card in the rail and let go over a viewport, which then shows it.
//! While it is carried its name follows the pointer, and the viewport under the pointer is
//! outlined.

use egui::{Align2, Id, LayerId, Order, Rect, Response, Stroke, StrokeKind, Ui};
use volsa_core::SeriesId;

use crate::action::Action;

/// A series being carried from its card: which, and the name its card shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// The series.
    pub series: SeriesId,
    /// The plain name on its card.
    pub name: String,
}

/// The width of the outline of a viewport a carried series would be let go over.
const OUTLINE: f32 = 3.0;

/// While a series is carried, write its name beside the pointer, above everything else.
pub fn follow_pointer(ui: &Ui) {
    let Some(carried) = egui::DragAndDrop::payload::<Carried>(ui.ctx()) else {
        return;
    };
    let Some(at) = ui.ctx().pointer_latest_pos() else {
        return;
    };
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    let layer = LayerId::new(Order::Tooltip, Id::new("volsa: a series carried"));
    let painter = ui.ctx().layer_painter(layer);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let visuals = ui.visuals();
    let text = format!("Show {}", carried.name);
    let galley = painter.layout_no_wrap(text, font, visuals.strong_text_color());
    let place = Align2::LEFT_TOP.anchor_size(at + egui::vec2(14.0, 14.0), galley.size());
    let ground = place.expand(6.0);
    painter.rect_filled(ground, 4, visuals.window_fill);
    painter.rect_stroke(ground, 4, visuals.window_stroke, StrokeKind::Outside);
    painter.galley(place.min, galley, visuals.text_color());
}

/// Outline viewport `at` while a series is carried over it, and show the series in it when it
/// is let go there.
pub fn take(ui: &Ui, response: &Response, cell: Rect, at: usize) -> Option<Action> {
    if response.dnd_hover_payload::<Carried>().is_some() {
        let line = Stroke::new(OUTLINE, ui.visuals().selection.stroke.color);
        ui.painter()
            .rect_stroke(cell.shrink(OUTLINE), 0, line, StrokeKind::Inside);
    }
    let carried = response.dnd_release_payload::<Carried>()?;
    Some(Action::SelectSeries {
        viewport: at,
        series: carried.series.clone(),
    })
}
