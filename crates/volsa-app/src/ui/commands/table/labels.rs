//! The Labels tab: opening labels, closing them, choosing their names, saving them, showing
//! every part, and how they are drawn over the planes.

use volsa_render::Overlay;

use super::super::{Command, Confirm, Does, Place, Request, Tab, Why};
use super::PLAIN;
use crate::action::Action;
use crate::labels::{LabelsStatus, on_this_series};
use crate::planes::grid;
use crate::state::AppState;
use crate::ui::parts::labelling::Wants;
use crate::ui::parts::{Asked, NO_VOLUME};

/// Why labels cannot be saved, listed or changed: none are open on the series shown.
const NO_LABELS: &str = "No labels are open on this series.";
/// Why labels cannot be closed: none are open.
const NONE_TO_CLOSE: &str = "No labels are open.";
/// Why labels from a file cannot be saved as a file.
const FROM_A_FILE: &str = "Labels from a file are saved already.";

/// The words each way of drawing the labels over the planes is found by.
const OVERLAY: &[&str] = &["outline", "overlay", "segmentation"];

/// Why nothing can be done with the labels: none are open on the series shown.
fn no_labels(state: &AppState) -> Option<Why> {
    (!on_this_series(state)).then_some(Why::Borrowed(NO_LABELS))
}

/// Why the labels cannot be saved as a file: none are open, or they came from a file.
fn not_made_here(state: &AppState) -> Option<Why> {
    no_labels(state).or_else(|| {
        state
            .labels
            .made
            .is_none()
            .then_some(Why::Borrowed(FROM_A_FILE))
    })
}

/// The action that draws the labels over the planes the way `overlay` says.
const fn overlay(
    id: &'static str,
    name: &'static str,
    label: &'static str,
    explains: &'static str,
    does: fn(&AppState) -> Option<Action>,
) -> Command {
    Command {
        id,
        name,
        label,
        explains,
        words: OVERLAY,
        place: Place::Tab(Tab::Labels),
        why_not: no_labels,
        does: Does::Act(does),
        ..PLAIN
    }
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
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
    Command {
        id: "labels.names",
        name: "Choose a table of names for the labels",
        label: "Names…",
        explains: "Choose a table of names for these labels",
        words: &["names", "table"],
        place: Place::Tab(Tab::Labels),
        why_not: no_labels,
        does: Does::Ask(Request::Choose(Asked::OpenNames)),
        ..PLAIN
    },
    Command {
        id: "labels.show_all",
        name: "Show every part",
        label: "Show all",
        words: &["unhide", "all", "every"],
        place: Place::Tab(Tab::Labels),
        why_not: no_labels,
        does: Does::Act(|_| Some(Action::PartsShowAll)),
        ..PLAIN
    },
    Command {
        id: "labels.close",
        name: "Close the labels",
        label: "Close",
        explains: "Close these labels",
        words: &["remove", "forget"],
        place: Place::Tab(Tab::Labels),
        why_not: |state| {
            (state.labels.status != LabelsStatus::Ready).then_some(Why::Borrowed(NONE_TO_CLOSE))
        },
        does: Does::Ask(Request::Confirm(Confirm::CloseLabels)),
        ..PLAIN
    },
    Command {
        id: "labels.save_csv",
        name: "Save the parts as a table",
        label: "Save as CSV…",
        explains: "Save every part, with its volume, as a table for a spreadsheet",
        words: &["csv", "spreadsheet", "export", "volumes"],
        place: Place::Tab(Tab::Labels),
        why_not: no_labels,
        does: Does::Ask(Request::Choose(Asked::SaveCsv)),
        ..PLAIN
    },
    Command {
        id: "labels.save",
        name: "Save the labels as a file",
        label: "Save as…",
        explains: "Save these labels as a file for other programs, with a table of their names \
                   beside it",
        words: &["nifti", "export", "file", "segmentation"],
        place: Place::Tab(Tab::Labels),
        why_not: not_made_here,
        does: Does::Ask(Request::Choose(Asked::Labelling(Wants::SaveLabels))),
        ..PLAIN
    },
    overlay(
        "labels.overlay.off",
        "Draw no outlines over the planes",
        "Off",
        "No labels over the plane views",
        |_| Some(Action::SetOverlay(Overlay::Off)),
    ),
    overlay(
        "labels.overlay.outline",
        "Outline each part over the planes",
        "Outline",
        "A line round each part",
        |_| Some(Action::SetOverlay(Overlay::Outline)),
    ),
    overlay(
        "labels.overlay.filled",
        "Fill each part over the planes",
        "Filled",
        "The line, and a tint of the part's colour inside it",
        |_| Some(Action::SetOverlay(Overlay::Filled)),
    ),
];
