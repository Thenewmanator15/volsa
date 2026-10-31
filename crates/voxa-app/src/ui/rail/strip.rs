//! The rail folded: a strip down the left of the window that unfolds it again when clicked,
//! anywhere on it. It shows `»` above a stack of three cards, the scans it holds, and under them
//! the word for what it holds.

use egui::{
    Align2, CornerRadius, FontId, Id, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2,
};

use super::UNFOLDS;
use crate::ui::commands::{self, Does};

/// Draw the strip in all the room `ui` has; gives what it does when it is clicked. A screen
/// reader calls it by the name of the action that shows the rail again, and the pointer
/// resting on it gives that action's hint.
pub fn show(ui: &mut Ui) -> Option<Does> {
    let command = commands::find(UNFOLDS)?;
    let rect = ui.max_rect();
    let response = ui.interact(rect, Id::new("voxa: the rail folded"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, command.name));
    if ui.is_rect_visible(rect) {
        let look = ui.style().interact(&response);
        let painter = ui.painter_at(rect);
        if response.hovered() {
            painter.rect_filled(rect, look.corner_radius, look.weak_bg_fill);
        }
        let colour = look.fg_stroke.color;
        let top = rect.center_top() + vec2(0.0, 18.0);
        painter.text(
            top,
            Align2::CENTER_CENTER,
            "»",
            FontId::proportional(18.0),
            colour,
        );
        // Three cards, one above the other, a small square on each for its picture.
        for at in 0..3_u8 {
            let middle = top + vec2(0.0, 30.0 + f32::from(at) * 10.0);
            let card = Rect::from_center_size(middle, vec2(22.0, 8.0));
            let stroke = Stroke::new(1.2, colour);
            painter.rect_stroke(
                card,
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
            let picture =
                Rect::from_center_size(card.left_center() + vec2(5.0, 0.0), vec2(4.0, 4.0));
            painter.rect_filled(picture, CornerRadius::ZERO, colour);
        }
        // Under the cards, the word for what they are: a strip without one is a mark to guess at.
        let word = top + vec2(0.0, 30.0 + 3.0 * 10.0 + 8.0);
        painter.text(
            word,
            Align2::CENTER_CENTER,
            command.label,
            FontId::proportional(10.0),
            colour,
        );
    }
    let response = response.on_hover_text(commands::hint(command));
    response.clicked().then(|| command.does.clone())
}
