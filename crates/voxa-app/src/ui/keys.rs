//! What the keys do: each key pressed, with what is held and what is in hand, turned into
//! what it asks for.

use egui::Key;

use super::VoxaApp;
use super::overlay::{shortcut_action, steps_slices};
use crate::action::Action;
use crate::annotations::Tool;
use crate::state::AppState;

/// What a key asks for.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "made and taken apart in the same frame, never kept"
)]
pub enum Pressed {
    /// A change of state.
    Act(Action),
    /// To open or close the list of keys.
    Keys,
    /// To open or close the measurements window.
    Measurements,
}

/// The keys held that change what another key means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    /// Ctrl, or Command on a Mac.
    pub command: bool,
    /// Shift.
    pub shift: bool,
}

/// What pressing `key` asks for. Undo and redo are where every program has them; `M` is the
/// measurements window; Esc puts the tool in hand down before it does anything else; the
/// rest are [`shortcut_action`]'s.
pub fn pressed(key: Key, held: Held, state: &AppState) -> Option<Pressed> {
    let act = |action| Some(Pressed::Act(action));
    match key {
        Key::Z if held.command && held.shift => act(Action::Redo),
        Key::Z if held.command => act(Action::Undo),
        Key::Y if held.command => act(Action::Redo),
        Key::M if !held.command => Some(Pressed::Measurements),
        // A question mark is its own key on some keyboards and Shift with slash on others.
        Key::Questionmark => Some(Pressed::Keys),
        Key::Slash if held.shift => Some(Pressed::Keys),
        Key::Escape if state.tool != Tool::Navigate => act(Action::SetTool(Tool::Navigate)),
        _ => shortcut_action(key, state.active).map(Pressed::Act),
    }
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

impl VoxaApp {
    /// Turn the keys pressed this frame into actions, unless the person is typing in a field.
    pub(super) fn keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        for (key, held) in pressed_keys(ctx) {
            match pressed(key, held, self.session.state()) {
                Some(Pressed::Act(action)) => actions.push(action),
                Some(Pressed::Keys) => self.help_open = !self.help_open,
                Some(Pressed::Measurements) => self.measure.open = !self.measure.open,
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::overlay::SHORTCUTS;
    use super::*;
    use crate::scene::MainPanel;

    fn on(key: Key, command: bool, shift: bool) -> Option<Pressed> {
        pressed(key, Held { command, shift }, &AppState::default())
    }

    #[test]
    fn undo_and_redo_are_where_every_program_has_them() {
        assert_eq!(on(Key::Z, true, false), Some(Pressed::Act(Action::Undo)));
        assert_eq!(on(Key::Y, true, false), Some(Pressed::Act(Action::Redo)));
        assert_eq!(on(Key::Z, true, true), Some(Pressed::Act(Action::Redo)));
        assert_eq!(
            on(Key::Z, false, false),
            None,
            "Z alone is no key of Voxa's"
        );
        assert_eq!(on(Key::Y, false, false), None);
    }

    #[test]
    fn m_is_the_measurements_window_and_a_question_mark_the_list_of_keys() {
        assert_eq!(on(Key::M, false, false), Some(Pressed::Measurements));
        assert_eq!(on(Key::M, true, false), None, "Ctrl+M is not M");
        assert_eq!(on(Key::Questionmark, false, false), Some(Pressed::Keys));
        assert_eq!(on(Key::Slash, false, true), Some(Pressed::Keys));
        assert_eq!(on(Key::Slash, false, false), None);
    }

    #[test]
    fn escape_puts_the_tool_down_before_it_does_anything_else() {
        let mut state = AppState::default();
        for tool in [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker] {
            state.tool = tool;
            let down = Pressed::Act(Action::SetTool(Tool::Navigate));
            assert_eq!(pressed(Key::Escape, Held::default(), &state), Some(down));
        }
        state.tool = Tool::Navigate;
        let scene = Pressed::Act(Action::SetMain(MainPanel::Scene));
        assert_eq!(pressed(Key::Escape, Held::default(), &state), Some(scene));
    }

    #[test]
    fn the_other_keys_act_on_the_active_viewport_as_they_did() {
        let state = AppState {
            active: 2,
            ..AppState::default()
        };
        let fit = Pressed::Act(Action::Fit { viewport: 2 });
        assert_eq!(pressed(Key::F, Held::default(), &state), Some(fit));
        assert_eq!(pressed(Key::Num3, Held::default(), &state), None);
    }

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

    #[test]
    fn the_list_of_keys_has_the_measuring_ones() {
        for key in ["Ctrl+Z", "Ctrl+Y", "M", "Esc"] {
            assert!(SHORTCUTS.iter().any(|(listed, _)| *listed == key), "{key}");
        }
        let (_, escape) = SHORTCUTS
            .iter()
            .find(|(key, _)| *key == "Esc")
            .expect("listed");
        assert!(
            escape.contains("tool"),
            "Esc says it puts the tool down: {escape}"
        );
    }
}
