//! How wide the series list is, and the lines between the parts of the window that the
//! person drags to give one part more of the width and another less.

use egui::{CursorIcon, Id, LayerId, Order, Rangef, Rect, Sense, Stroke, Ui};

use super::theme;

/// The narrowest the series list is, in points: a card's picture and a few words beside it.
const LIST_LEAST: f32 = 220.0;
/// The widest the series list is, in points: wider than this its cards are only empty.
const LIST_MOST: f32 = 360.0;
/// The most of the window's width the series list takes, however wide it was dragged.
const LIST_MOST_SHARE: f32 = 0.35;
/// How far either side of a dividing line the pointer can take hold of it, in points.
const GRAB: f32 = 4.0;

/// The width of the series list in a window `window` points across, for its `share` of it:
/// between 220 and 360 points, and never more than 35% of the window.
pub fn list_width(window: f32, share: f32) -> f32 {
    (window * share)
        .clamp(LIST_LEAST, LIST_MOST)
        .min(window * LIST_MOST_SHARE)
}

/// The series list's share of a window `window` points across once its right edge has been
/// dragged `by` points from where its `share` puts it, as far as its limits allow.
pub fn list_dragged(window: f32, share: f32, by: f32) -> f32 {
    let wanted = list_width(window, share) + by;
    list_width(window, wanted / window) / window
}

/// A line down the window at `x`, over the rows `along`, that the pointer can take hold of
/// and drag across: how far it was dragged this frame, in points, rightwards. It is drawn
/// only while the pointer is over it or dragging it. Made after what lies beneath it, it
/// takes the drag from them.
pub fn divider(ui: &Ui, id: Id, x: f32, along: Rangef) -> f32 {
    let grip = Rect::from_x_y_ranges(Rangef::new(x - GRAB, x + GRAB), along);
    let response = ui.interact(grip, id, Sense::drag());
    let held = response.dragged();
    if held || response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        let colour = if held {
            theme::ACCENT
        } else {
            theme::QUIET_TEXT
        };
        let painter = ui.ctx().layer_painter(LayerId::new(Order::Foreground, id));
        painter.vline(x, along, Stroke::new(2.0, colour));
    }
    if held { response.drag_delta().x } else { 0.0 }
}

#[cfg(test)]
mod tests;
