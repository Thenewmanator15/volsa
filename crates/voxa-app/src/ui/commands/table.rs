//! The entries of the list of actions, a file for each part of the window, and what they
//! have in common. The order of the files is the order in which the help lists the keys.

use egui::Key;

use super::{Command, Does, Place, Shortcut};
use crate::action::Action;
use crate::state::AppState;

mod measure;
mod panel;
mod scene;
mod slices;
mod tools;
mod top;
mod window;

/// Every entry, in order.
pub(super) fn all() -> Vec<Command> {
    [
        slices::COMMANDS,
        window::COMMANDS,
        panel::COMMANDS,
        measure::COMMANDS,
        scene::COMMANDS,
        tools::COMMANDS,
        top::COMMANDS,
    ]
    .concat()
}

/// What an entry is in each respect it does not set: nothing more to explain, no key, offered
/// for every profile, always usable, and only reached by a key. Each entry sets its own id,
/// name, label, words and what it does.
const PLAIN: Command = Command {
    id: "",
    name: "",
    label: "",
    explains: "",
    words: &[],
    key: None,
    place: Place::Keys,
    profiles: &[],
    why_not: usable,
    does: Does::Act(nothing),
};

/// The why-not of an action that can always be used.
fn usable(_: &AppState) -> Option<&'static str> {
    None
}

/// What an entry does until it says what it does: nothing.
fn nothing(_: &AppState) -> Option<Action> {
    None
}

/// A key pressed alone, written in the help as `shown`.
const fn alone(key: Key, shown: &'static str) -> Shortcut {
    Shortcut {
        key,
        command: false,
        shift: false,
        shown,
    }
}

/// A key pressed with Ctrl, or Command on a Mac, written in the help as `shown`.
const fn ctrl(key: Key, shown: &'static str) -> Shortcut {
    Shortcut {
        key,
        command: true,
        shift: false,
        shown,
    }
}
