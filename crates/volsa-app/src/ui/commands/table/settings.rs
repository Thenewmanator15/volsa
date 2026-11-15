//! The Settings window: opening it from the top bar's menu or by its key.

use egui::Key;

use super::super::{Command, Does, Place, Request};
use super::{PLAIN, ctrl};

/// The entries.
pub(super) const COMMANDS: &[Command] = &[Command {
    id: "settings.open",
    name: "Settings",
    label: "Settings…",
    explains: "The theme, the interface size and which side the patient's left is drawn on",
    // Not "interface size" or "theme": search finds each size and theme by those words, and
    // goes to it at once.
    words: &["settings", "preferences", "options", "appearance"],
    key: Some(ctrl(Key::Comma, "Ctrl+,")),
    place: Place::Menu,
    does: Does::Ask(Request::Settings),
    ..PLAIN
}];
