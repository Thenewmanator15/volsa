//! What the keys do: each key pressed, with what is held, looked up in the list of actions.

use egui::Key;

use super::VoxaApp;
use super::commands::{self, Does, Request};
use super::overlay::steps_slices;
use crate::action::Action;
use crate::state::AppState;

/// The keys held that change what another key means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    /// Ctrl, or Command on a Mac.
    pub command: bool,
    /// Shift.
    pub shift: bool,
}

/// What pressing `key` does in `state`: what the action it is the key of does, while that
/// action can be used. A key whose action cannot be used now does nothing.
pub fn pressed(key: Key, held: Held, state: &AppState) -> Option<Does> {
    let command = commands::for_key(key, held, state)?;
    (command.why_not)(state).is_none().then_some(command.does)
}

/// The keys pressed this frame that are Voxa's to answer, with what was held with each: none
/// while a text field or another widget has the keyboard, as when the person types in the
/// search field. A key held down is taken again as the system repeats it only when it steps
/// through the slices; any other acts once.
pub fn pressed_keys(ctx: &egui::Context) -> Vec<(Key, Held)> {
    if ctx.egui_wants_keyboard_input() {
        return Vec::new();
    }
    ctx.input(|input| {
        let down = |event: &egui::Event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } if !*repeat || steps_slices(*key) => {
                let (command, shift) = (modifiers.command, modifiers.shift);
                Some((*key, Held { command, shift }))
            }
            _ => None,
        };
        input.events.iter().filter_map(down).collect()
    })
}

/// What the keys pressed this frame ask for in `state`: the changes of state, and what is
/// asked of the window. Nothing while the person is typing in a field.
pub fn answered(ctx: &egui::Context, state: &AppState) -> (Vec<Action>, Vec<Request>) {
    let mut actions = Vec::new();
    let mut requests = Vec::new();
    for (key, held) in pressed_keys(ctx) {
        let asked = pressed(key, held, state).and_then(|does| does.run(state, &mut actions));
        requests.extend(asked);
    }
    (actions, requests)
}

impl VoxaApp {
    /// Turn the keys pressed this frame into actions, and do what they ask of the window,
    /// unless the person is typing in a field.
    pub(super) fn keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let (acted, asked) = answered(ctx, self.session.state());
        actions.extend(acted);
        for request in asked {
            self.ask(ctx, request);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key pressed with nothing held, or the system's repeat of one held down.
    fn down(key: Key, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// The keys Voxa takes in a frame of these events, after a frame in which a text field
    /// was drawn, and was given the keyboard if `typing`.
    fn taken(events: Vec<egui::Event>, typing: bool) -> Vec<(Key, Held)> {
        let ctx = egui::Context::default();
        let mut text = String::new();
        let field = egui::Id::new("a field");
        let mut taken = Vec::new();
        for frame in [Vec::new(), events] {
            let input = egui::RawInput {
                events: frame,
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let drawn = ui.add(egui::TextEdit::singleline(&mut text).id(field));
                if typing {
                    drawn.request_focus();
                }
                taken = pressed_keys(ui.ctx());
            });
            // No renderer is behind this pass to take the font texture.
            output.textures_delta.clear();
        }
        taken
    }

    #[test]
    fn a_key_that_steps_through_slices_steps_again_while_it_is_held_down() {
        let none = Held::default();
        // A key pressed again before it is let go is the system repeating it: egui marks the
        // second press of each as a repeat, whatever the event said.
        let held_down = [Key::ArrowDown, Key::PageUp, Key::H, Key::Space];
        let events = held_down
            .iter()
            .flat_map(|key| [down(*key, false), down(*key, true)])
            .collect();
        let expected = [
            (Key::ArrowDown, none),
            (Key::ArrowDown, none),
            (Key::PageUp, none),
            (Key::PageUp, none),
            (Key::H, none),
            (Key::Space, none),
        ];
        assert_eq!(taken(events, false), expected, "H and Space act once");
    }

    #[test]
    fn no_key_is_taken_while_a_text_field_has_the_keyboard() {
        let events = || {
            vec![
                down(Key::ArrowUp, false),
                down(Key::End, false),
                down(Key::H, false),
            ]
        };
        assert_eq!(taken(events(), true), []);
        assert_eq!(taken(events(), false).len(), 3, "with the field let go");
    }
}
