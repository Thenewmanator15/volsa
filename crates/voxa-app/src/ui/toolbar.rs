//! The bar across the top of the window.

use egui::Ui;
use voxa_core::WindowLevel;
use voxa_volume::PRESETS;

use crate::action::Action;
use crate::state::{AppState, Layout};

/// What the person asked for that is not a change of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Asked {
    /// To choose a scan to open.
    pub open: bool,
    /// To see the About window.
    pub about: bool,
    /// To see the list of keys and what the mouse does.
    pub shortcuts: bool,
}

/// The layouts offered, with the label and hover text of each button.
const LAYOUTS: [(Layout, &str, &str); 3] = [
    (Layout::One, "1", "One viewport"),
    (Layout::TwoAcross, "2", "Two viewports side by side"),
    (Layout::TwoByTwo, "4", "Four viewports"),
];

/// Draw the toolbar: open, the layouts, the window of the active viewport, the display
/// switches, and at the far end the list of keys and About.
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
        ui.separator();
        window(ui, state, actions);
        ui.separator();
        switches(ui, state, actions);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            asked.about = ui.button("About").clicked();
            asked.shortcuts = ui.button("?").on_hover_text("Keys and mouse").clicked();
        });
    });
    asked
}

/// The window and level of the active viewport, as numbers that can be typed or dragged,
/// and the presets.
fn window(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let Some(shown) = state.viewports.get(viewport) else {
        return;
    };
    ui.label("Window");
    let (mut width, mut level) = (shown.wl.width, shown.wl.center);
    let step = f64::from((shown.wl.width / 100.0).max(0.1));
    let width_field = egui::DragValue::new(&mut width)
        .speed(step)
        .prefix("W ")
        .range(1.0..=f32::MAX);
    let level_field = egui::DragValue::new(&mut level).speed(step).prefix("L ");
    let width_changed = ui
        .add(width_field)
        .on_hover_text("Width: the span of values from black to white")
        .changed();
    let level_changed = ui
        .add(level_field)
        .on_hover_text("Level: the value shown as mid-grey")
        .changed();
    if width_changed || level_changed {
        actions.push(Action::SetWindow {
            viewport,
            wl: WindowLevel::new(level, width),
        });
    }
    for (preset, named) in PRESETS.iter().enumerate() {
        let hint = format!(
            "From the {} to the {} percentile of the slice shown",
            named.low_pct, named.high_pct
        );
        if ui.button(named.name).on_hover_text(hint).clicked() {
            actions.push(Action::ApplyPreset { viewport, preset });
        }
    }
}

/// The display convention and whether the patient's details are shown.
fn switches(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let convention = "On: the patient's left is on the right of the screen, as a radiologist reads it.\n\
                      Off: neurological, the patient's left on the left.";
    if ui
        .selectable_label(state.radiological, "Radiological")
        .on_hover_text(convention)
        .clicked()
    {
        actions.push(Action::ToggleRadiological);
    }
    let patient = "Show the patient's name and identifier in the viewports and the tag browser";
    if ui
        .selectable_label(state.show_patient, "Patient details")
        .on_hover_text(patient)
        .clicked()
    {
        actions.push(Action::TogglePatient);
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    use super::*;

    fn pass(ctx: &egui::Context, state: &AppState, events: Vec<Event>) -> (Vec<Action>, Asked) {
        let mut actions = Vec::new();
        let mut asked = Asked::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 600.0));
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
    fn sweep(state: &AppState) -> (Vec<Action>, Asked) {
        let ctx = egui::Context::default();
        let _ = pass(&ctx, state, Vec::new());
        let (mut actions, mut all) = (Vec::new(), Asked::default());
        for x in (3..1400u16).step_by(5) {
            let (asked_for, asked) = click(&ctx, state, pos2(f32::from(x), 12.0));
            for action in asked_for {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            all.open |= asked.open;
            all.about |= asked.about;
            all.shortcuts |= asked.shortcuts;
        }
        (actions, all)
    }

    #[test]
    fn the_bar_offers_each_thing_once_from_left_to_right() {
        let (actions, asked) = sweep(&AppState::default());
        let preset = |preset| Action::ApplyPreset {
            viewport: 0,
            preset,
        };
        let expected = vec![
            Action::SetLayout(Layout::TwoAcross),
            Action::SetLayout(Layout::TwoByTwo),
            preset(0),
            preset(1),
            preset(2),
            Action::ToggleRadiological,
            Action::TogglePatient,
        ];
        assert_eq!(
            actions, expected,
            "the layout in use is not asked for again"
        );
        assert_eq!(
            asked,
            Asked {
                open: true,
                about: true,
                shortcuts: true
            }
        );
    }

    #[test]
    fn the_window_controls_act_on_the_active_viewport() {
        let state = AppState {
            layout: Layout::TwoByTwo,
            active: 2,
            ..AppState::default()
        };
        let (actions, _) = sweep(&state);
        let layouts = [Layout::One, Layout::TwoAcross].map(Action::SetLayout);
        assert!(
            layouts.iter().all(|layout| actions.contains(layout)),
            "{actions:?}"
        );
        assert!(!actions.contains(&Action::SetLayout(Layout::TwoByTwo)));
        for preset in 0..3 {
            assert!(
                actions.contains(&Action::ApplyPreset {
                    viewport: 2,
                    preset
                }),
                "{actions:?}"
            );
        }
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::SetWindow { .. })),
            "a click changes no number"
        );
    }
}
