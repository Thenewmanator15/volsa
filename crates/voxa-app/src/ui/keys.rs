//! What the keys do: each key pressed, with what is held and what is in hand, turned into
//! what it asks for.

use egui::Key;

use super::VoxaApp;
use super::overlay::shortcut_action;
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
    /// To show or hide the series list.
    SeriesList,
    /// To go into full screen, or out of it.
    FullScreen,
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
/// measurements window; `B` shows or hides the series list and F11 is full screen, as in
/// most programs; Esc puts the tool in hand down before it does anything else; the rest are
/// [`shortcut_action`]'s.
pub fn pressed(key: Key, held: Held, state: &AppState) -> Option<Pressed> {
    let act = |action| Some(Pressed::Act(action));
    match key {
        Key::Z if held.command && held.shift => act(Action::Redo),
        Key::Z if held.command => act(Action::Undo),
        Key::Y if held.command => act(Action::Redo),
        Key::M if !held.command => Some(Pressed::Measurements),
        Key::B if !held.command => Some(Pressed::SeriesList),
        Key::F11 => Some(Pressed::FullScreen),
        // A question mark is its own key on some keyboards and Shift with slash on others.
        Key::Questionmark => Some(Pressed::Keys),
        Key::Slash if held.shift => Some(Pressed::Keys),
        Key::Escape if state.tool != Tool::Navigate => act(Action::SetTool(Tool::Navigate)),
        _ => shortcut_action(key, state.active).map(Pressed::Act),
    }
}

impl VoxaApp {
    /// Turn the keys pressed this frame into actions, unless the person is typing in a field.
    pub(super) fn keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed_now: Vec<(Key, Held)> = ctx.input(|input| {
            let down = |event: &egui::Event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => {
                    let (command, shift) = (modifiers.command, modifiers.shift);
                    Some((*key, Held { command, shift }))
                }
                _ => None,
            };
            input.events.iter().filter_map(down).collect()
        });
        for (key, held) in pressed_now {
            match pressed(key, held, self.session.state()) {
                Some(Pressed::Act(action)) => actions.push(action),
                Some(Pressed::Keys) => self.help_open = !self.help_open,
                Some(Pressed::Measurements) => self.measure.open = !self.measure.open,
                Some(Pressed::SeriesList) => self.screen.list_shown = !self.screen.list_shown,
                Some(Pressed::FullScreen) => super::screen::toggle_full_screen(ctx),
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
    fn b_shows_or_hides_the_series_list_and_f11_is_full_screen() {
        assert_eq!(on(Key::B, false, false), Some(Pressed::SeriesList));
        assert_eq!(on(Key::B, true, false), None, "Ctrl+B is not B");
        assert_eq!(on(Key::F11, false, false), Some(Pressed::FullScreen));
    }

    #[test]
    fn the_list_of_keys_has_the_ones_for_the_screen() {
        for key in ["B", "F11", "Ctrl+= / Ctrl+- / Ctrl+0"] {
            assert!(SHORTCUTS.iter().any(|(listed, _)| *listed == key), "{key}");
        }
        let (_, size) = SHORTCUTS
            .iter()
            .find(|(key, _)| key.starts_with("Ctrl+="))
            .expect("listed");
        assert!(size.contains("interface"), "{size}");
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
