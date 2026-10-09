//! The bar across the top of the window.

use egui::Ui;

use crate::action::Action;
use crate::state::{AppState, Layout};

/// What the person asked for that is not a change of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Asked {
    /// To choose a scan to open.
    pub open: bool,
    /// To see the About window.
    pub about: bool,
}

/// The layouts offered, with the label and hover text of each button.
const LAYOUTS: [(Layout, &str, &str); 3] = [
    (Layout::One, "1", "One viewport"),
    (Layout::TwoAcross, "2", "Two viewports side by side"),
    (Layout::TwoByTwo, "4", "Four viewports"),
];

/// Draw the toolbar: open, the layouts, and About at the far end.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Asked {
    let mut asked = Asked::default();
    ui.horizontal(|ui| {
        let open = ui.button("Open…");
        asked.open = open
            .on_hover_text("Open a folder, a zip file or a DICOMDIR")
            .clicked();
        ui.separator();
        ui.label("Layout");
        for (layout, label, hint) in LAYOUTS {
            let chosen = state.layout == layout;
            if ui
                .selectable_label(chosen, label)
                .on_hover_text(hint)
                .clicked()
                && !chosen
            {
                actions.push(Action::SetLayout(layout));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            asked.about = ui.button("About").clicked();
        });
    });
    asked
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    use super::*;

    fn pass(ctx: &egui::Context, state: &AppState, events: Vec<Event>) -> (Vec<Action>, Asked) {
        let mut actions = Vec::new();
        let mut asked = Asked::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let input = RawInput {
            screen_rect: Some(screen),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| asked = show(ui, state, &mut actions));
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        (actions, asked)
    }

    /// Press and release the main button at a point.
    fn click(ctx: &egui::Context, state: &AppState, at: Pos2) -> (Vec<Action>, Asked) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = pass(ctx, state, vec![Event::PointerMoved(at), button(true)]);
        pass(ctx, state, vec![button(false)])
    }

    /// Click all the way along the bar and gather everything that was asked for.
    fn sweep(state: &AppState) -> (Vec<Action>, usize, usize) {
        let ctx = egui::Context::default();
        let _ = pass(&ctx, state, Vec::new());
        let (mut actions, mut opens, mut abouts) = (Vec::new(), 0, 0);
        for x in (3..800u16).step_by(5) {
            let (asked_for, asked) = click(&ctx, state, pos2(f32::from(x), 12.0));
            for action in asked_for {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            opens += usize::from(asked.open);
            abouts += usize::from(asked.about);
        }
        (actions, opens, abouts)
    }

    #[test]
    fn the_bar_offers_open_the_other_layouts_and_about() {
        let (actions, opens, abouts) = sweep(&AppState::default());
        let expected = [Layout::TwoAcross, Layout::TwoByTwo].map(Action::SetLayout);
        assert_eq!(
            actions, expected,
            "the layout in use is not asked for again"
        );
        assert!(opens > 0 && abouts > 0, "{opens} {abouts}");
    }

    #[test]
    fn the_layout_in_use_is_the_one_left_out() {
        let state = AppState {
            layout: Layout::TwoByTwo,
            ..AppState::default()
        };
        let (actions, _, _) = sweep(&state);
        assert_eq!(
            actions,
            [Layout::One, Layout::TwoAcross].map(Action::SetLayout)
        );
    }
}
