//! The line along the bottom of the window: the disclaimer, and the newest notice.

use egui::{Align, Layout, RichText, TextStyle, TextWrapMode, WidgetText};

use super::theme;
use crate::action::Action;
use crate::state::{Notice, NoticeKind};

/// The least of a notice that is shown beside the disclaimer, in points: a notice that would
/// be cut shorter than this goes onto a line of its own.
const LEAST_OF_A_NOTICE: f32 = 120.0;
/// What the button that dismisses a notice says.
const DISMISS: &str = "Dismiss";

/// Draw the line: the disclaimer, and the newest of the `notices` with a way to dismiss it.
/// The notice goes beside the disclaimer when the line has room for both, and onto a line of
/// its own below it when it has not; nothing is drawn over the disclaimer, which wraps in a
/// window too narrow for it. A notice too long for its room is cut short, and shown whole
/// when the pointer rests on it. Gives the disclaimer as it was laid out.
pub fn show(ui: &mut egui::Ui, notices: &[Notice], actions: &mut Vec<Action>) -> egui::Response {
    let disclaimer = RichText::new(theme::DISCLAIMER)
        .small()
        .color(theme::QUIET_TEXT);
    let Some((at, notice)) = notices.iter().enumerate().next_back() else {
        return ui.add(egui::Label::new(disclaimer).wrap());
    };
    if beside(ui, notice) {
        ui.horizontal(|ui| {
            let shown = ui.label(disclaimer);
            the_notice(ui, at, notice, actions);
            shown
        })
        .inner
    } else {
        let shown = ui.add(egui::Label::new(disclaimer).wrap());
        ui.horizontal(|ui| the_notice(ui, at, notice, actions));
        shown
    }
}

/// Whether the line has room for the disclaimer, the notice, or at least
/// [`LEAST_OF_A_NOTICE`] of it, and the button that dismisses it, side by side.
fn beside(ui: &egui::Ui, notice: &Notice) -> bool {
    let wide = |text: &str, style: TextStyle| {
        let text = WidgetText::from(RichText::new(text).text_style(style.clone()));
        let laid = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, style);
        laid.size().x
    };
    let spacing = ui.spacing();
    let button = wide(DISMISS, TextStyle::Button) + 2.0 * spacing.button_padding.x + 2.0;
    let words = wide(&notice.text, TextStyle::Small).min(LEAST_OF_A_NOTICE);
    let needed = wide(theme::DISCLAIMER, TextStyle::Small) + words + button;
    needed + 2.0 * spacing.item_spacing.x <= ui.available_width()
}

/// The notice, at the right of what is left of its line, cut short to fit, and the button
/// that dismisses it.
fn the_notice(ui: &mut egui::Ui, at: usize, notice: &Notice, actions: &mut Vec<Action>) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.small_button(DISMISS).clicked() {
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
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, RawInput, Rect, vec2};

    use super::*;

    /// The words drawn on the line, each with where it is, in a window `width` points wide
    /// in Voxa's look, with a long notice to show.
    fn drawn(width: f32) -> Vec<(String, Rect)> {
        let notice = Notice {
            kind: NoticeKind::Warning,
            text: "A notice long enough to run across the whole of a narrow window and on \
                   past the disclaimer, which it must not cover"
                .to_owned(),
            details: Vec::new(),
            key: None,
        };
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 480.0))),
            ..RawInput::default()
        };
        let mut actions = Vec::new();
        let notices = [notice];
        let mut output = ctx.run_ui(input, |ui| {
            show(ui, &notices, &mut actions);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        let words = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => {
                    let said = text.galley.text().to_owned();
                    Some((said, text.visual_bounding_rect()))
                }
                _ => None,
            });
        words.collect()
    }

    /// Fails if any two of the words cover one another.
    fn apart(words: &[(String, Rect)]) {
        for (one, (said, rect)) in words.iter().enumerate() {
            for (other, there) in words.iter().skip(one + 1) {
                assert!(!rect.intersects(*there), "{said:?} covers {other:?}");
            }
        }
    }

    #[test]
    fn a_long_notice_in_a_narrow_window_is_cut_short_beside_the_disclaimer() {
        let words = drawn(640.0);
        assert_eq!(words.len(), 3, "the disclaimer, the notice and the button");
        let lines = words.iter().map(|(_, rect)| rect.center().y.round());
        let first = words
            .first()
            .map_or(0.0, |(_, rect)| rect.center().y.round());
        assert!(
            lines.clone().all(|line| (line - first).abs() <= 2.0),
            "one line"
        );
        apart(&words);
    }

    #[test]
    fn with_no_room_beside_the_disclaimer_the_notice_goes_on_a_line_below_it() {
        // The smallest window with the interface at twice its usual size.
        let words = drawn(320.0);
        assert_eq!(words.len(), 3, "the disclaimer, the notice and the button");
        let disclaimer = words.iter().find(|(said, _)| said == theme::DISCLAIMER);
        let (_, disclaimer) = disclaimer.expect("the disclaimer, whole");
        for (said, rect) in &words {
            if said != theme::DISCLAIMER {
                assert!(rect.top() >= disclaimer.bottom(), "{said:?} is beside it");
            }
        }
        apart(&words);
    }

    #[test]
    fn with_no_notice_the_disclaimer_wraps_in_a_window_too_narrow_for_it() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(160.0, 480.0))),
            ..RawInput::default()
        };
        let mut shown = None;
        let mut output = ctx.run_ui(input, |ui| shown = Some(show(ui, &[], &mut Vec::new())));
        output.textures_delta.clear();
        let shown = shown.expect("the disclaimer").rect;
        assert!(shown.right() <= 160.0, "{shown:?}");
        let said = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            });
        assert_eq!(said.as_deref(), Some(theme::DISCLAIMER), "every word of it");
    }
}
