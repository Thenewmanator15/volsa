//! The Views menu in the 3D tab: save the scene as it is, and go back to a view saved before.

use egui::Ui;

use crate::action::Action;
use crate::state::AppState;
use crate::ui::access;
use crate::ui::commands::{self, Confirm, Request, menus};
use crate::views::ViewsAction;

/// The menu itself, closed until it is pressed.
pub fn menu(
    ui: &mut Ui,
    state: &AppState,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    let views = &menus::VIEWS;
    commands::menu_button(ui, views, views.label, |ui| {
        contents(ui, state, (actions, asked));
    });
}

/// What the menu holds: a name to type and a button that saves the scene under it, then each
/// saved view, oldest first, with a button that forgets it, once the person has said they mean
/// it.
pub fn contents(
    ui: &mut Ui,
    state: &AppState,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    // What has been typed so far is kept by the interface between frames.
    let typed = egui::Id::new("voxa: the name of a view being saved");
    let mut name = ui
        .data_mut(|data| data.get_temp::<String>(typed))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let field = egui::TextEdit::singleline(&mut name).hint_text("Name (optional)");
        let field = ui.add(field.desired_width(150.0));
        access::call(&field, "Name (optional), for the view to be saved");
        // The view is saved under the name typed beside the button.
        if commands::button(ui, "scene.view.save", state, None).is_some() {
            actions.push(Action::Views(ViewsAction::Save {
                name: std::mem::take(&mut name),
            }));
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
                actions.push(Action::Views(ViewsAction::Apply(at)));
            }
            let delete = ui.small_button("Delete");
            access::call(&delete, format!("Delete {}", view.name));
            if delete.on_hover_text("Forget this view").clicked() {
                asked.push(Request::Confirm(Confirm::DeleteView(at)));
                ui.close();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, RawInput, pos2};
    use voxa_core::WindowLevel;

    use super::*;
    use crate::scene::SceneStatus;
    use crate::views::SavedView;

    /// Click all over what the menu holds and gather everything that was asked for, once
    /// each, in the order met going down: the changes of state, and what is asked of the window.
    fn sweep(state: &AppState) -> (Vec<Action>, Vec<Request>) {
        let ctx = egui::Context::default();
        let (mut asked, mut wanted) = (Vec::new(), Vec::new());
        let mut pass = |events: Vec<Event>| {
            let (mut actions, mut requests) = (Vec::new(), Vec::new());
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                contents(ui, state, (&mut actions, &mut requests));
            });
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
            for action in actions {
                if !asked.contains(&action) {
                    asked.push(action);
                }
            }
            for request in requests {
                if !wanted.contains(&request) {
                    wanted.push(request);
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
        (asked, wanted)
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
        let (asked, wanted) = sweep(&with_views(&["Front", "Ear"]));
        let expected = [
            Action::Views(ViewsAction::Save {
                name: String::new(),
            }),
            Action::Views(ViewsAction::Apply(0)),
            Action::Views(ViewsAction::Apply(1)),
        ];
        assert_eq!(asked, expected);
        // A view is forgotten only once the person has said they mean it.
        let forget = |at| Request::Confirm(Confirm::DeleteView(at));
        assert_eq!(wanted, [forget(0), forget(1)]);
    }

    #[test]
    fn with_no_volume_there_is_nothing_to_save_and_with_no_views_nothing_to_go_to() {
        let mut state = with_views(&[]);
        let save = Action::Views(ViewsAction::Save {
            name: String::new(),
        });
        assert_eq!(sweep(&state), (vec![save], vec![]));
        state.scene.status = SceneStatus::Building { done: 1, of: 9 };
        assert_eq!(sweep(&state), (vec![], vec![]));
    }
}
