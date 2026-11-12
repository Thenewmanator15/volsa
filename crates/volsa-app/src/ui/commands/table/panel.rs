//! The panel at the right of the window: showing each of its tabs, and folding it away.

use egui::Key;

use super::super::{Command, Does, Place, Request, Shortcut, Tab};
use super::{PLAIN, alone};

/// The entry that shows `tab`, named by `name`, found by `words` as well, and shown by `key` if
/// it has one.
const fn show(
    tab: Tab,
    name: &'static str,
    explains: &'static str,
    key: Option<Shortcut>,
    words: &'static [&'static str],
) -> Command {
    Command {
        id: tab.command(),
        name,
        label: tab.title(),
        explains,
        words,
        key,
        place: Place::Panel,
        does: Does::Ask(Request::ShowTab(tab)),
        ..PLAIN
    }
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    show(
        Tab::View,
        "Show the View tab",
        "Brightness, which way round the patient is drawn, the slices and how they are shown",
        None,
        &["view", "tab", "panel", "brightness"],
    ),
    show(
        Tab::Measure,
        "Show the Measure tab",
        "Rulers, angles, regions and markers, and the list of what has been measured",
        Some(alone(Key::M, "M")),
        &[
            "measure",
            "measurements",
            "tab",
            "panel",
            "rulers",
            "tools",
            "list",
        ],
    ),
    show(
        Tab::Scene,
        "Show the 3D tab",
        "The looks of the 3D scene, the sides it is seen from, its cuts and saving it",
        None,
        &["tab", "panel", "scene"],
    ),
    show(
        Tab::Labels,
        "Show the Labels tab",
        "The labelled parts of the scan",
        None,
        &["tab", "panel", "parts"],
    ),
    show(
        Tab::Tools,
        "Show the Tools tab",
        "Tools that work on the scan, such as labelling it",
        None,
        &["tab", "panel", "labelling"],
    ),
    Command {
        id: "panel.fold",
        name: "Show or hide the tab panel",
        label: "»",
        explains: "Fold the panel away to a strip of icons, or open it again",
        words: &["panel", "tabs", "sidebar", "hide", "fold"],
        key: Some(alone(Key::P, "P")),
        place: Place::Panel,
        does: Does::Ask(Request::FoldPanel),
        ..PLAIN
    },
];
