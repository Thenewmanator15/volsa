//! What the keys do: each key pressed, with what is held, looked up in the list of actions.

use egui::{Context, EventFilter, Id, Key, Response};

use super::VoxaApp;
use super::commands::{self, Does, Request};
use super::overlay::steps_slices;
use super::typing;
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
    (command.why_not)(state)
        .is_none()
        .then(|| command.does.clone())
}

/// Where the person has the keyboard when a key is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keyboard {
    /// Nowhere in particular: every key is Voxa's.
    Free,
    /// On a button, a card or another widget, whose keys are egui's to move and press with.
    OnAWidget,
    /// On a picture, which holds the arrows against egui's moving of the focus, so that they
    /// are Voxa's: they step through the slices.
    OnAPicture,
}

/// The keys egui moves the keyboard and presses buttons with: Voxa leaves them to it while a
/// widget has the keyboard.
const EGUIS: [Key; 12] = [
    Key::Tab,
    Key::Escape,
    Key::Enter,
    Key::Space,
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::Home,
    Key::End,
    Key::PageUp,
    Key::PageDown,
];

/// The keys egui keeps while a picture has the keyboard: it moves on with Tab, and presses with
/// Enter; a picture's other keys are Voxa's.
const EGUIS_ON_A_PICTURE: [Key; 3] = [Key::Tab, Key::Escape, Key::Enter];

/// Where a pass noted that a picture has the keyboard.
const PICTURE_HAS_THE_KEYBOARD: &str = "voxa: the pass in which a picture had the keyboard";

/// Let a picture that has the keyboard keep the arrow keys: egui would move the keyboard to the
/// widget beside it with them, and they step through the slices instead. Called for the
/// response of each picture as it is drawn. A tool in hand can take the arrows from the slices
/// by consuming them before [`pressed_keys`] looks, in the pass in which the picture is drawn.
/// With `keeps_escape`, Esc is the picture's too, and does not take the keyboard from it: it
/// gives up what is half drawn on the picture, and the next Esc, with nothing begun, leaves it.
pub fn hold_the_arrows(ctx: &Context, picture: &Response, keeps_escape: bool) {
    if !picture.has_focus() {
        return;
    }
    let arrows = EventFilter {
        horizontal_arrows: true,
        vertical_arrows: true,
        escape: keeps_escape,
        ..EventFilter::default()
    };
    ctx.memory_mut(|memory| memory.set_focus_lock_filter(picture.id, arrows));
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| data.insert_temp(Id::new(PICTURE_HAS_THE_KEYBOARD), now));
}

/// Where the person has the keyboard in the pass now being drawn.
fn keyboard(ctx: &Context) -> Keyboard {
    let noted = ctx.data(|data| data.get_temp::<u64>(Id::new(PICTURE_HAS_THE_KEYBOARD)));
    if noted == Some(ctx.cumulative_pass_nr()) {
        Keyboard::OnAPicture
    } else if ctx.egui_wants_keyboard_input() {
        Keyboard::OnAWidget
    } else {
        Keyboard::Free
    }
}

/// The keys pressed this frame that are Voxa's to answer, with what was held with each: none
/// while a text field has the keyboard, as when the person types in a field, nor while the
/// search box or a question is open. While another widget has it, every key but those egui
/// moves and presses with (Tab, Enter, Space, the arrows, Home, End and the Page keys); while a
/// picture has it, the arrows and those keys step through the slices. A key held down is taken
/// again as the system repeats it only when it steps through the slices; any other acts once.
pub fn pressed_keys(ctx: &Context) -> Vec<(Key, Held)> {
    if typing::keys_are_taken(ctx) {
        return Vec::new();
    }
    let left_to_egui: &[Key] = match keyboard(ctx) {
        Keyboard::Free => &[],
        Keyboard::OnAWidget => &EGUIS,
        Keyboard::OnAPicture => &EGUIS_ON_A_PICTURE,
    };
    ctx.input(|input| {
        let down = |event: &egui::Event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } if !left_to_egui.contains(key) && (!*repeat || steps_slices(*key)) => {
                let (command, shift) = (modifiers.command, modifiers.shift);
                Some((*key, Held { command, shift }))
            }
            _ => None,
        };
        input.events.iter().filter_map(down).collect()
    })
}

/// What the keys pressed this frame ask for in `state`: the changes of state, and what is
/// asked of the window. Nothing while the person is typing in a field. Once a key asks for the
/// search, the keys after it in the frame are not taken: they are meant for the search box
/// that opens, which has not yet the keyboard in this frame.
pub fn answered(ctx: &egui::Context, state: &AppState) -> (Vec<Action>, Vec<Request>) {
    let mut actions = Vec::new();
    let mut requests = Vec::new();
    for (key, held) in pressed_keys(ctx) {
        let asked = pressed(key, held, state).and_then(|does| does.run(state, &mut actions));
        let opens_search = asked == Some(Request::Search);
        requests.extend(asked);
        if opens_search {
            break;
        }
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
    fn once_a_key_asks_for_the_search_the_keys_after_it_in_the_frame_are_not_taken() {
        let state = crate::testing::thick_scan_ready();
        let answered_to = |keys: &[Key]| {
            // A context of its own each time: a key pressed again before it is let go is taken
            // for the system repeating it.
            let ctx = egui::Context::default();
            let events = keys.iter().map(|key| down(*key, false)).collect();
            let input = egui::RawInput {
                events,
                ..egui::RawInput::default()
            };
            let mut answer = None;
            let mut output = ctx.run_ui(input, |ui| answer = Some(answered(ui.ctx(), &state)));
            // No renderer is behind this pass to take the font texture.
            output.textures_delta.clear();
            answer.expect("the keys are answered")
        };
        // A slash opens the search, and the letters typed after it in the same frame go to the
        // box, not to the actions they are the keys of.
        let keys = [Key::H, Key::R, Key::Num2];
        let (acted, asked) = answered_to(&keys);
        assert!(acted.len() >= 2, "with no slash they act: {acted:?}");
        assert_eq!(asked, []);
        for opening in [vec![Key::Slash], vec![Key::H, Key::Slash]] {
            let before = opening.iter().position(|key| *key == Key::Slash);
            let all: Vec<Key> = opening.iter().copied().chain(keys).collect();
            let (acted, asked) = answered_to(&all);
            assert_eq!(asked, [Request::Search], "after {opening:?}");
            // Only what came before the slash acts.
            assert_eq!(
                acted.len(),
                before.unwrap_or(0),
                "after {opening:?}: {acted:?}"
            );
        }
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
