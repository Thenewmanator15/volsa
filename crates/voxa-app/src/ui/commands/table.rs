//! The entries of the list of actions, a file for each part of the window, and what they
//! have in common. The order of the files is the order in which the help lists the keys.

use egui::Key;

use super::{Command, Does, Place, Shortcut, Why};
use crate::action::Action;
use crate::state::AppState;

mod labels;
mod measure;
mod panel;
mod pictures;
mod scene;
mod slices;
mod tools;
mod top;
mod window;

pub(in crate::ui) use pictures::no_3d_view;

/// Every entry, in order.
pub(super) fn all() -> Vec<Command> {
    [
        slices::COMMANDS,
        pictures::COMMANDS,
        window::COMMANDS,
        panel::COMMANDS,
        measure::COMMANDS,
        scene::COMMANDS,
        labels::COMMANDS,
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
fn usable(_: &AppState) -> Option<Why> {
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

/// Why a control of the pictures can do nothing: a document is open in their place, or no scan
/// is shown in the viewport the keys act on.
pub(in crate::ui) fn no_picture(state: &AppState) -> Option<Why> {
    if state.document.is_some() {
        return Some(Why::Borrowed(
            "A document is open: close it to see the pictures.",
        ));
    }
    state
        .shown_info(state.active)
        .is_none()
        .then_some(Why::Borrowed("Open a scan to see it here."))
}

/// Why the arrangement of the pictures can do nothing to be seen: a document is open in their
/// place.
pub(super) fn covered_by_a_document(state: &AppState) -> Option<Why> {
    state.document.is_some().then_some(Why::Borrowed(
        "A document is open: close it to see the pictures.",
    ))
}
