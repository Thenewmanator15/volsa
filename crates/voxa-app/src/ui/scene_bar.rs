//! The second line of the bar across the top of the window: what the 3D scene offers.

use egui::Ui;
use voxa_render::{Look, MAX_CUTS, NamedView};

use super::commands;
use super::scene::{export, views};
use crate::action::Action;
use crate::labels::look_drawn;
use crate::state::AppState;

/// The looks offered, with the label and hover text of each button. The labels are the names
/// radiology uses; the hover text says what each means. The list of actions takes its entries
/// for the looks from these.
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

/// What the Parts look is, and why it cannot be chosen while there are no labels.
pub const PARTS_HINT: &str = "The labelled parts of the head as solids, each in its own colour";
pub const PARTS_NEED_LABELS: &str = "Open labels in the Parts tab to see them as solids here.";

/// The sides the patient can be seen from, with the letter on each button and what it means,
/// from which the list of actions takes its entries for them.
pub const VIEWS: [(NamedView, &str, &str); 6] = [
    (NamedView::Anterior, "A", "From the front (anterior)"),
    (NamedView::Posterior, "P", "From behind (posterior)"),
    (NamedView::Left, "L", "From the patient's left"),
    (NamedView::Right, "R", "From the patient's right"),
    (NamedView::Superior, "S", "From above the head (superior)"),
    (NamedView::Inferior, "I", "From below the feet (inferior)"),
];

/// The looks to choose between, each by the action that chooses it: the three of the scan,
/// and then the labelled parts as solids, which can be chosen when there are labels on this
/// series to draw.
const LOOK_IDS: [(Look, &str); 4] = [
    (Look::Surface, "scene.look.surface"),
    (Look::Vr, "scene.look.vr"),
    (Look::Mip, "scene.look.mip"),
    (Look::Parts, "scene.look.parts"),
];

/// The sides the patient can be seen from, by their actions, in the order of [`VIEWS`].
const SIDE_IDS: [&str; 6] = [
    "scene.side.anterior",
    "scene.side.posterior",
    "scene.side.left",
    "scene.side.right",
    "scene.side.superior",
    "scene.side.inferior",
];

/// The actions that remove each cut there can be, the first cut's first.
const UNCUT_IDS: [&str; MAX_CUTS] = ["scene.uncut.1", "scene.uncut.2", "scene.uncut.3"];

/// Draw the button of the action `id`, and put the change it makes onto `actions`.
fn offer(ui: &mut Ui, id: &str, state: &AppState, on: Option<bool>, actions: &mut Vec<Action>) {
    if let Some(does) = commands::button(ui, id, state, on) {
        let _ = does.run(state, actions);
    }
}

/// The looks to choose between. The look in use is not asked for again.
fn looks(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let drawn = look_drawn(state);
    for (look, id) in LOOK_IDS {
        let chosen = drawn == look;
        let pressed = commands::button(ui, id, state, Some(chosen));
        if let Some(does) = pressed.filter(|_| !chosen) {
            let _ = does.run(state, actions);
        }
    }
}

/// Draw the scene's line of the bar: the looks, the sides to see the patient from, the cuts,
/// whether the slice shown in 2D is drawn in the scene, the saved views, and saving to a
/// file. Gives what the person asked to save, if anything.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Option<export::Asked> {
    let scene = &state.scene;
    let mut saving = None;
    ui.horizontal_wrapped(|ui| {
        // In a narrow window what does not fit goes onto the next line, whole.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        super::theme::closer_in_a_short_window(ui);
        ui.label("3D");
        looks(ui, state, actions);
        ui.separator();
        ui.label("View");
        for id in SIDE_IDS {
            offer(ui, id, state, None, actions);
        }
        ui.separator();
        offer(ui, "scene.cut", state, None, actions);
        for id in UNCUT_IDS.iter().take(scene.cuts.len()) {
            offer(ui, id, state, None, actions);
        }
        ui.separator();
        // The slice is a picture of the scan: it is not drawn among the labelled solids.
        offer(ui, "scene.slice", state, Some(scene.slice_shown), actions);
        ui.separator();
        views::menu(ui, state, actions);
        let menu = ui.menu_button("Export", |ui| saving = export::contents(ui, state));
        menu.response
            .on_hover_text("Save the 3D view as a picture or as a turntable");
    });
    saving
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
            let mut output = ctx.run_ui(input, |ui| {
                let asked = show(ui, state, &mut actions);
                assert_eq!(asked, None, "the Export menu is closed");
            });
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
    fn the_parts_look_is_offered_when_there_are_labels_on_this_series_and_only_then() {
        let parts = Action::SceneLook(Look::Parts);
        let unlabelled = sweep(&AppState::default());
        assert!(!unlabelled.contains(&parts), "nothing to draw as solids");
        let (mut state, _) = crate::testing::labelled_head();
        let scans = [Look::Surface, Look::Vr, Look::Mip].map(Action::SceneLook);
        let asked = sweep(&state);
        let offered = [&scans[1..], std::slice::from_ref(&parts)].concat();
        assert_eq!(asked[..3], offered, "after the looks of the scan");
        assert!(asked.contains(&Action::ShowSliceInScene(true)));
        crate::reducer::reduce(&mut state, parts.clone());
        let asked = sweep(&state);
        assert_eq!(asked[..3], scans, "the look in use is not asked for again");
        assert!(!asked.contains(&parts));
        let slice = Action::ShowSliceInScene(true);
        assert!(!asked.contains(&slice), "no slice among the solids");
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
