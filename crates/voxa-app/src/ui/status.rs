//! The line along the bottom of the window: the disclaimer, and the newest notice.

use egui::RichText;

use super::theme;
use crate::action::Action;
use crate::state::{Notice, NoticeKind};

/// Draw the line: the disclaimer, and the newest of the `notices` with a way to dismiss it.
/// A notice too long for the room beside the disclaimer is cut short, and shown whole when
/// the pointer rests on it.
pub fn show(ui: &mut egui::Ui, notices: &[Notice], actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(theme::DISCLAIMER)
                .small()
                .color(theme::QUIET_TEXT),
        );
        let Some((at, notice)) = notices.iter().enumerate().next_back() else {
            return;
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Dismiss").clicked() {
                actions.push(Action::DismissNotice(at));
            }
            let colour = match notice.kind {
                NoticeKind::Info => theme::QUIET_TEXT,
                NoticeKind::Warning | NoticeKind::Error => theme::WARNING,
            };
            let words = RichText::new(&notice.text).small().color(colour);
            let shown = ui.add(egui::Label::new(words).truncate());
            if !notice.details.is_empty() {
                shown.on_hover_text(notice.details.join("\n"));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, RawInput, Rect, vec2};

    use super::*;

    #[test]
    fn a_long_notice_in_a_narrow_window_is_cut_short_beside_the_disclaimer() {
        let notice = Notice {
            kind: NoticeKind::Warning,
            text: "A notice long enough to run across the whole of a narrow window and on \
                   past the disclaimer, which it must not cover"
                .to_owned(),
            details: Vec::new(),
            key: None,
        };
        let ctx = egui::Context::default();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
            ..RawInput::default()
        };
        let mut actions = Vec::new();
        let notices = [notice];
        let mut output = ctx.run_ui(input, |ui| show(ui, &notices, &mut actions));
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        let words: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.visual_bounding_rect()),
                _ => None,
            })
            .collect();
        assert_eq!(words.len(), 3, "the disclaimer, the notice and the button");
        for (one, rect) in words.iter().enumerate() {
            for other in words.iter().skip(one + 1) {
                assert!(!rect.intersects(*other), "{rect:?} covers {other:?}");
            }
        }
    }
}
