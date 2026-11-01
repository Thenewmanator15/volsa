//! Measuring: the measurements themselves, the tools that draw them, taking back and doing
//! again, and saving or clearing what has been measured.

use egui::Key;

use super::super::{Command, Does, Place, Request, Tab, Why};
use super::{PLAIN, ctrl};
use crate::action::Action;
use crate::annotations::Tool;
use crate::measuring::{why_no_region, why_not_measurable};
use crate::state::AppState;
use crate::ui::tabs::{NOTHING_MEASURED, Save};

/// Why a tool cannot be used: there is nothing to measure on, neither a volume nor a slice
/// shown whose files give it a scale.
fn not_measurable(state: &AppState) -> Option<Why> {
    why_not_measurable(state).map(Why::Borrowed)
}

/// Why the region tool cannot be used: as any other, or the picture is in colour.
fn no_region(state: &AppState) -> Option<Why> {
    why_no_region(state).map(Why::Borrowed)
}

/// Why the measurements cannot be saved or cleared: there are none.
fn none_measured(state: &AppState) -> Option<Why> {
    let none = state.annotations.items.is_empty();
    none.then_some(Why::Borrowed(NOTHING_MEASURED))
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "measure.move",
        name: "Move without measuring",
        label: "Move",
        explains: "The main button moves the position, or turns the 3D view",
        words: &["navigate", "move", "pointer", "turn"],
        place: Place::Tab(Tab::Measure),
        why_not: not_measurable,
        does: Does::Act(|_| Some(Action::SetTool(Tool::Navigate))),
        ..PLAIN
    },
    Command {
        id: "measure.ruler",
        name: "Measure a length",
        label: "Ruler",
        explains: "Press at one end and let go at the other; in 3D, click two surfaces",
        words: &["ruler", "length", "distance", "size", "millimetres"],
        place: Place::Tab(Tab::Measure),
        why_not: not_measurable,
        does: Does::Act(|_| Some(Action::SetTool(Tool::Ruler))),
        ..PLAIN
    },
    Command {
        id: "measure.angle",
        name: "Measure an angle",
        label: "Angle",
        explains: "On a slice or a plane: click an arm's end, the corner, the other arm's end",
        words: &["angle", "degrees", "protractor"],
        place: Place::Tab(Tab::Measure),
        why_not: not_measurable,
        does: Does::Act(|_| Some(Action::SetTool(Tool::Angle))),
        ..PLAIN
    },
    Command {
        id: "measure.region",
        name: "Measure the values in a region",
        label: "Region",
        explains: "An ellipse on a slice or a plane: drag from its middle",
        words: &["region", "ellipse", "roi", "mean", "area"],
        place: Place::Tab(Tab::Measure),
        why_not: no_region,
        does: Does::Act(|_| Some(Action::SetTool(Tool::Region))),
        ..PLAIN
    },
    Command {
        id: "measure.marker",
        name: "Place a named marker",
        label: "Marker",
        explains: "Click where it goes",
        words: &["marker", "point", "pin", "note"],
        place: Place::Tab(Tab::Measure),
        why_not: not_measurable,
        does: Does::Act(|_| Some(Action::SetTool(Tool::Marker))),
        ..PLAIN
    },
    Command {
        id: "measure.undo",
        name: "Take back the last change to the measurements",
        label: "Undo",
        words: &["undo", "back", "revert"],
        key: Some(ctrl(Key::Z, "Ctrl+Z")),
        place: Place::Tab(Tab::Measure),
        why_not: |state| {
            let none = !state.annotations.can_undo();
            none.then_some(Why::Borrowed("There is nothing to take back."))
        },
        does: Does::Act(|_| Some(Action::Undo)),
        ..PLAIN
    },
    Command {
        id: "measure.redo",
        name: "Do again what was taken back",
        label: "Redo",
        words: &["redo", "again", "repeat"],
        // Ctrl+Shift+Z does it too: see the other keys in the list's own module.
        key: Some(ctrl(Key::Y, "Ctrl+Y or Ctrl+Shift+Z")),
        place: Place::Tab(Tab::Measure),
        why_not: |state| {
            let none = !state.annotations.can_redo();
            none.then_some(Why::Borrowed("There is nothing to do again."))
        },
        does: Does::Act(|_| Some(Action::Redo)),
        ..PLAIN
    },
    Command {
        id: "measure.save_csv",
        name: "Save the measurements as a table",
        label: "Save as CSV…",
        explains: "A table any spreadsheet opens",
        words: &["csv", "spreadsheet", "export", "save"],
        place: Place::Tab(Tab::Measure),
        why_not: none_measured,
        does: Does::Ask(Request::SaveMeasurements(Save::Csv)),
        ..PLAIN
    },
    Command {
        id: "measure.save_json",
        name: "Save the measurements as JSON",
        label: "Save as JSON…",
        explains: "For another program to read",
        words: &["json", "export", "save"],
        place: Place::Tab(Tab::Measure),
        why_not: none_measured,
        does: Does::Ask(Request::SaveMeasurements(Save::Json)),
        ..PLAIN
    },
    Command {
        id: "measure.clear",
        name: "Clear all the measurements",
        label: "Clear all",
        explains: "Remove every measurement of this series. This can be undone with Ctrl+Z.",
        words: &["clear", "delete", "remove", "erase"],
        place: Place::Tab(Tab::Measure),
        why_not: none_measured,
        does: Does::Act(|_| Some(Action::AnnotationsClear)),
        ..PLAIN
    },
];
