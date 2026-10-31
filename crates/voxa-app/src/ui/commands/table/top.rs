//! The window itself: opening a scan, the scans shown or hidden, full screen, the size of the
//! interface, the theme, the keys and About, where the log is, the tags of a file, and
//! opening labels.

use egui::Key;

use super::super::{Command, Does, Place, Request, Tab, Why};
use super::{PLAIN, alone, ctrl};
use crate::planes::grid;
use crate::ui::parts::NO_VOLUME;
use crate::ui::theme::ThemeChoice;
use crate::ui::top::Pick;

/// The words each size of the interface is found by.
const SIZE: &[&str] = &["interface size", "scale", "text size"];

/// A size the interface can be drawn at, as the menu offers it.
const fn size(id: &'static str, label: &'static str, name: &'static str, at: f32) -> Command {
    Command {
        id,
        name,
        label,
        words: SIZE,
        place: Place::Menu,
        does: Does::Ask(Request::Size(at)),
        ..PLAIN
    }
}

/// The action that draws the window in `choice`, as the top bar's theme button offers it.
const fn theme(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    words: &'static [&'static str],
    choice: ThemeChoice,
) -> Command {
    Command {
        id,
        name,
        label,
        words,
        place: Place::TopBar,
        does: Does::Ask(Request::Theme(choice)),
        ..PLAIN
    }
}

/// The entries. egui itself answers the keys that size the interface: they are listed here
/// for the help and for search, and the list's lookup leaves them to egui.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "open.folder",
        name: "Open a folder of DICOM files",
        label: "Folder…",
        words: &["open", "folder", "directory", "load", "import"],
        place: Place::TopBar,
        does: Does::Ask(Request::Open(Pick::Folder)),
        ..PLAIN
    },
    Command {
        id: "open.file",
        name: "Open a zip file or a DICOMDIR",
        label: "Zip file or DICOMDIR…",
        words: &["open", "zip", "dicomdir", "file", "load"],
        place: Place::TopBar,
        does: Does::Ask(Request::Open(Pick::File)),
        ..PLAIN
    },
    Command {
        id: "rail.fold",
        name: "Show or hide the scans",
        label: "Scans",
        explains: "The column of scans at the left",
        words: &["scans", "series", "list", "sidebar", "hide", "fold"],
        key: Some(alone(Key::B, "B")),
        place: Place::Menu,
        does: Does::Ask(Request::FoldRail),
        ..PLAIN
    },
    Command {
        id: "window.full_screen",
        name: "Full screen, or back to a window",
        label: "Full screen",
        words: &["fullscreen", "maximise", "whole screen"],
        key: Some(alone(Key::F11, "F11")),
        place: Place::Menu,
        does: Does::Ask(Request::FullScreen),
        ..PLAIN
    },
    Command {
        id: "size.larger",
        name: "Draw the interface larger",
        label: "Larger",
        words: &["zoom in", "bigger", "enlarge"],
        key: Some(ctrl(Key::Equals, "Ctrl+=")),
        place: Place::Menu,
        does: Does::Ask(Request::Larger),
        ..PLAIN
    },
    Command {
        id: "size.smaller",
        name: "Draw the interface smaller",
        label: "Smaller",
        words: &["zoom out", "smaller", "shrink"],
        key: Some(ctrl(Key::Minus, "Ctrl+-")),
        place: Place::Menu,
        does: Does::Ask(Request::Smaller),
        ..PLAIN
    },
    size("size.75", "75%", "Draw the interface at 75%", 0.75),
    size("size.90", "90%", "Draw the interface at 90%", 0.9),
    Command {
        key: Some(ctrl(Key::Num0, "Ctrl+0")),
        ..size(
            "size.100",
            "100%",
            "Draw the interface at its usual size",
            1.0,
        )
    },
    size("size.110", "110%", "Draw the interface at 110%", 1.1),
    size("size.125", "125%", "Draw the interface at 125%", 1.25),
    size("size.150", "150%", "Draw the interface at 150%", 1.5),
    size("size.175", "175%", "Draw the interface at 175%", 1.75),
    size("size.200", "200%", "Draw the interface at 200%", 2.0),
    Command {
        id: "search.open",
        name: "Search actions, parts, scans and help",
        label: "Search",
        explains: "Type a few letters of what you want to do or find",
        words: &["find", "search", "look up", "go to", "command"],
        // A slash does it too: see the other keys in the list's own module.
        key: Some(ctrl(Key::K, "Ctrl+K or /")),
        place: Place::TopBar,
        does: Does::Ask(Request::Search),
        ..PLAIN
    },
    Command {
        id: "help.keys",
        name: "Show the keys and what the mouse does",
        label: "?",
        words: &["help", "shortcuts", "keyboard", "mouse", "keys"],
        // Shift with slash does it too: see the other keys in the list's own module.
        key: Some(alone(Key::Questionmark, "?")),
        place: Place::TopBar,
        does: Does::Ask(Request::Help(None)),
        ..PLAIN
    },
    theme(
        "theme.system",
        "System",
        "Follow the system's light or dark setting",
        &["theme", "appearance", "automatic", "system"],
        ThemeChoice::System,
    ),
    theme(
        "theme.light",
        "Light",
        "Use the light theme",
        &["theme", "appearance", "light mode", "bright"],
        ThemeChoice::Light,
    ),
    theme(
        "theme.dark",
        "Dark",
        "Use the dark theme",
        &["theme", "appearance", "dark mode", "night"],
        ThemeChoice::Dark,
    ),
    Command {
        id: "about",
        name: "About Voxa",
        label: "About",
        words: &["version", "licence", "credits", "disclaimer"],
        place: Place::Menu,
        does: Does::Ask(Request::About),
        ..PLAIN
    },
    Command {
        id: "log.where",
        name: "Show where the log is",
        label: "Where the log is",
        explains: "The log holds nothing about a patient and none of your files' names",
        words: &["log", "diagnostics", "support", "crash"],
        place: Place::Menu,
        does: Does::Ask(Request::WhereTheLogIs),
        ..PLAIN
    },
    Command {
        id: "tags.show",
        name: "Show the file's tags",
        label: "Tags",
        explains: "Every element of the file behind the slice shown",
        words: &["tags", "dicom", "header", "elements", "metadata"],
        // In the menu until the scans' cards have a menu of their own.
        place: Place::Menu,
        does: Does::Ask(Request::Tags),
        ..PLAIN
    },
    Command {
        id: "labels.open",
        name: "Open a file of labels",
        label: "Open labels…",
        words: &["labels", "segmentation", "nifti", "open"],
        place: Place::Tab(Tab::Labels),
        why_not: |state| grid(state).is_none().then_some(Why::Borrowed(NO_VOLUME)),
        does: Does::Ask(Request::OpenLabels),
        ..PLAIN
    },
];
