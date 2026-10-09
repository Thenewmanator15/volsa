//! The Views menu on the scene's line of the bar: save the scene as it is, and go back to a
//! view saved before.

use egui::Ui;

use crate::action::Action;
use crate::scene::SceneStatus;
use crate::state::AppState;

/// The menu itself, closed until it is pressed.
pub fn menu(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let button = ui.menu_button("Views", |ui| contents(ui, state, actions));
    button
        .response
        .on_hover_text("Save the scene as it is now, and go back to a saved view");
}

/// What the menu holds: a name to type and a button that saves the scene under it, then each
/// saved view, oldest first, with a button that forgets it.
pub fn contents(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    // What has been typed so far is kept by the interface between frames.
    let typed = egui::Id::new("voxa: the name of a view being saved");
    let mut name = ui
        .data_mut(|data| data.get_temp::<String>(typed))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let field = egui::TextEdit::singleline(&mut name).hint_text("Name (optional)");
        ui.add(field.desired_width(150.0));
        let there = state.scene.status == SceneStatus::Ready;
        let save = ui.add_enabled(there, egui::Button::new("Save this view"));
        let hint = "Keep where the scene is seen from, its look, its window and its cuts";
        if save
            .on_hover_text(hint)
            .on_disabled_hover_text("There is no 3D view to save yet")
            .clicked()
        {
            actions.push(Action::ViewSave {
                name: std::mem::take(&mut name),
            });
        }
    });
    ui.data_mut(|data| data.insert_temp(typed, name));
    if !state.views.is_empty() {
        ui.separator();
    }
    for (at, view) in state.views.iter().enumerate() {
        ui.horizontal(|ui| {
            let go = ui.button(&view.name);
            if go
                .on_hover_text("See the scene as it was when this was saved")
                .clicked()
            {
                actions.push(Action::ViewApply(at));
            }
            if ui
                .small_button("Delete")
                .on_hover_text("Forget this view")
                .clicked()
            {
                actions.push(Action::ViewDelete(at));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, RawInput, pos2};
    use voxa_core::WindowLevel;

    use super::*;
    use crate::views::SavedView;

    /// Click all over what the menu holds and gather everything that was asked for, once
    /// each, in the order met going down.
    fn sweep(state: &AppState) -> Vec<Action> {
        let ctx = egui::Context::default();
        let mut asked = Vec::new();
        let mut pass = |events: Vec<Event>| {
            let mut actions = Vec::new();
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| contents(ui, state, &mut actions));
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
            for action in actions {
                if !asked.contains(&action) {
                    asked.push(action);
                }
            }
        };
        pass(Vec::new());
        for y in (6..120u8).step_by(6) {
            for x in (4..400u16).step_by(8) {
                let pos = pos2(f32::from(x), f32::from(y));
                let button = |pressed| Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                pass(vec![Event::PointerMoved(pos), button(true)]);
                pass(vec![button(false)]);
            }
        }
        asked
    }

    fn with_views(names: &[&str]) -> AppState {
        let mut state = AppState::default();
        state.scene.status = SceneStatus::Ready;
        let window = WindowLevel::new(1.0, 2.0);
        let view = |name: &&str| SavedView::of(name, 1, &state.scene, window);
        state.views = names.iter().map(view).collect();
        state
    }

    #[test]
    fn the_menu_saves_the_scene_and_offers_each_saved_view_with_a_way_to_forget_it() {
        let asked = sweep(&with_views(&["Front", "Ear"]));
        let expected = [
            Action::ViewSave {
                name: String::new(),
            },
            Action::ViewApply(0),
            Action::ViewDelete(0),
            Action::ViewApply(1),
            Action::ViewDelete(1),
        ];
        assert_eq!(asked, expected);
    }

    #[test]
    fn with_no_volume_there_is_nothing_to_save_and_with_no_views_nothing_to_go_to() {
        let mut state = with_views(&[]);
        assert_eq!(
            sweep(&state),
            [Action::ViewSave {
                name: String::new()
            }]
        );
        state.scene.status = SceneStatus::Building { done: 1, of: 9 };
        assert_eq!(sweep(&state), []);
    }
}
