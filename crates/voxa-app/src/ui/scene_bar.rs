//! The second line of the bar across the top of the window: what the 3D scene offers.

use egui::Ui;
use voxa_render::{Look, MAX_CUTS, NamedView};

use super::scene::views;
use crate::action::Action;
use crate::state::AppState;

/// The looks offered, with the label and hover text of each button. The labels are the names
/// radiology uses; the hover text says what each means.
pub const LOOKS: [(Look, &str, &str); 3] = [
    (
        Look::Surface,
        "Surface",
        "A solid, lit surface where the tissue begins",
    ),
    (
        Look::Vr,
        "VR",
        "Volume rendering: see-through, brighter tissue more solid",
    ),
    (
        Look::Mip,
        "MIP",
        "Maximum-intensity projection: the brightest value on each line of sight",
    ),
];

/// The sides the patient can be seen from, with the letter on each button and what it means.
pub const VIEWS: [(NamedView, &str, &str); 6] = [
    (NamedView::Anterior, "A", "From the front (anterior)"),
    (NamedView::Posterior, "P", "From behind (posterior)"),
    (NamedView::Left, "L", "From the patient's left"),
    (NamedView::Right, "R", "From the patient's right"),
    (NamedView::Superior, "S", "From above the head (superior)"),
    (NamedView::Inferior, "I", "From below the feet (inferior)"),
];

/// Draw the scene's line of the bar: the looks, the sides to see the patient from, the cuts,
/// whether the slice shown in 2D is drawn in the scene, and the saved views.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let scene = &state.scene;
    ui.horizontal_wrapped(|ui| {
        ui.label("3D");
        for (look, label, hint) in LOOKS {
            let chosen = scene.look == look;
            if ui
                .selectable_label(chosen, label)
                .on_hover_text(hint)
                .clicked()
                && !chosen
            {
                actions.push(Action::SceneLook(look));
            }
        }
        ui.separator();
        ui.label("View");
        for (view, letter, hint) in VIEWS {
            if ui.button(letter).on_hover_text(hint).clicked() {
                actions.push(Action::SceneView(view));
            }
        }
        ui.separator();
        let room = scene.cuts.len() < MAX_CUTS;
        let cut = ui.add_enabled(room, egui::Button::new("Cut"));
        let hint = "Cut the volume open straight across the view; drag the cut's handle to move it";
        if cut
            .on_hover_text(hint)
            .on_disabled_hover_text("Three cuts at most")
            .clicked()
        {
            actions.push(Action::CutAdd);
        }
        for cut in 0..scene.cuts.len() {
            let remove = ui.button(format!("Remove cut {}", cut + 1));
            if remove
                .on_hover_text("Put back what this cut took away")
                .clicked()
            {
                actions.push(Action::CutRemove { cut });
            }
        }
        ui.separator();
        let shown = scene.slice_shown;
        let slice = ui.selectable_label(shown, "Slice in 3D");
        if slice
            .on_hover_text("Draw the slice shown in 2D inside the 3D scene")
            .clicked()
        {
            actions.push(Action::ShowSliceInScene(!shown));
        }
        ui.separator();
        views::menu(ui, state, actions);
    });
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    use super::*;

    /// Click all the way along the line and gather everything that was asked for, once each,
    /// in the order met.
    fn sweep(state: &AppState) -> Vec<Action> {
        let ctx = egui::Context::default();
        let mut asked = Vec::new();
        let mut pass = |events: Vec<Event>| {
            let mut actions = Vec::new();
            let screen = Rect::from_min_size(Pos2::ZERO, vec2(1700.0, 600.0));
            let input = RawInput {
                screen_rect: Some(screen),
                events,
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| show(ui, state, &mut actions));
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
            for action in actions {
                if !asked.contains(&action) {
                    asked.push(action);
                }
            }
        };
        pass(Vec::new());
        for x in (3..1700u16).step_by(5) {
            let pos = pos2(f32::from(x), 12.0);
            let button = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            pass(vec![Event::PointerMoved(pos), button(true)]);
            pass(vec![button(false)]);
        }
        asked
    }

    fn views() -> Vec<Action> {
        VIEWS
            .iter()
            .map(|(view, _, _)| Action::SceneView(*view))
            .collect()
    }

    #[test]
    fn the_line_offers_each_thing_once_from_left_to_right() {
        let looks = vec![Action::SceneLook(Look::Vr), Action::SceneLook(Look::Mip)];
        let rest = vec![Action::CutAdd, Action::ShowSliceInScene(true)];
        assert_eq!(sweep(&AppState::default()), [looks, views(), rest].concat());
    }

    #[test]
    fn the_look_in_use_is_not_asked_for_again_and_the_slice_can_be_put_away() {
        let mut state = AppState::default();
        state.scene.look = Look::Mip;
        state.scene.slice_shown = true;
        let asked = sweep(&state);
        let looks = [
            Action::SceneLook(Look::Surface),
            Action::SceneLook(Look::Vr),
        ];
        assert_eq!(asked[..2], looks);
        assert_eq!(asked.last(), Some(&Action::ShowSliceInScene(false)));
    }

    #[test]
    fn each_cut_has_a_button_that_removes_it_and_three_cuts_leave_no_room_for_more() {
        let mut state = AppState::default();
        let cut = voxa_render::CutPlane::facing(&state.scene.camera);
        state.scene.cuts = vec![cut; 2];
        let asked = sweep(&state);
        let removed = [Action::CutRemove { cut: 0 }, Action::CutRemove { cut: 1 }];
        assert!(
            removed.iter().all(|remove| asked.contains(remove)),
            "{asked:?}"
        );
        assert!(asked.contains(&Action::CutAdd), "there is room for a third");
        state.scene.cuts = vec![cut; MAX_CUTS];
        let asked = sweep(&state);
        assert!(
            !asked.contains(&Action::CutAdd),
            "but not for a fourth: {asked:?}"
        );
        assert!(asked.contains(&Action::CutRemove { cut: 2 }));
    }

    #[test]
    fn the_buttons_carry_the_names_and_letters_radiology_uses() {
        assert_eq!(LOOKS.map(|(_, label, _)| label), ["Surface", "VR", "MIP"]);
        assert_eq!(
            VIEWS.map(|(_, letter, _)| letter),
            ["A", "P", "L", "R", "S", "I"]
        );
        assert!(
            LOOKS.iter().all(|(_, _, hint)| hint.len() > 20),
            "each is explained"
        );
        assert!(VIEWS.iter().all(|(_, _, hint)| hint.starts_with("From ")));
    }
}
