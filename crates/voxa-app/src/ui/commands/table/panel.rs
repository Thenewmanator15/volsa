//! The panel at the right of the window: showing each of its tabs, and folding it away.

use egui::Key;

use super::super::{Command, Does, Place, Request, Tab};
use super::{PLAIN, alone};

/// The entry that shows `tab`, named by `name`, found by `words` as well.
const fn show(
    tab: Tab,
    name: &'static str,
    explains: &'static str,
    words: &'static [&'static str],
) -> Command {
    Command {
        id: tab.command(),
        name,
        label: tab.title(),
        explains,
        words,
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
        &["view", "tab", "panel", "brightness"],
    ),
    show(
        Tab::Measure,
        "Show the Measure tab",
        "The tools that measure, and what has been measured",
        &["tab", "panel", "rulers"],
    ),
    show(
        Tab::Scene,
        "Show the 3D tab",
        "The looks of the 3D scene, the sides it is seen from, its cuts and saving it",
        &["tab", "panel", "scene"],
    ),
    show(
        Tab::Labels,
        "Show the Labels tab",
        "The labelled parts of the scan",
        &["tab", "panel", "parts"],
    ),
    show(
        Tab::Tools,
        "Show the Tools tab",
        "Tools that work on the scan, such as labelling it",
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
