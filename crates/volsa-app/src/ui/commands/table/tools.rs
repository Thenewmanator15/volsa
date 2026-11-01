//! The tools of the Tools tab that are part of Volsa: so far, labelling a brain scan, with the
//! choices behind it, the way to stop a run, and what to do with the result kept for a series.

use volsa_segment::ModelChoice;

use super::super::{Command, Confirm, Does, Place, Request, Tab, Why};
use super::PLAIN;
use crate::action::Action;
use crate::labelling::{LabellingAction, can_label};
use crate::state::AppState;
use crate::ui::parts::Asked;
use crate::ui::parts::labelling::Wants;
use crate::ui::tabs::{LABEL_BUTTON, LABEL_TOOL};

/// Why the scan cannot be labelled now, in a sentence; the Tools tab says why in full.
const CANNOT_LABEL: &str = "Volsa cannot label this scan now. The Tools tab says why.";
/// Why a run cannot be stopped: none is on.
const NOT_RUNNING: &str = "No labelling is running.";
/// Why a kept result cannot be shown or deleted: none is kept for the series.
const NOTHING_KEPT: &str = "No labels are kept for this series.";

/// The words the choices behind the labelling are found by.
const CHOICES: &[&str] = &["segment", "synthseg", "options", "brain"];
/// The words each model is found by.
const MODEL: &[&str] = &["model", "network", "options", "synthseg"];
/// The words each folder is found by.
const FOLDER: &[&str] = &["folder", "files", "options", "setup", "models"];

/// Why a run cannot be stopped.
fn not_running(state: &AppState) -> Option<Why> {
    state
        .labelling
        .running
        .is_none()
        .then_some(Why::Borrowed(NOT_RUNNING))
}

/// Why a kept result cannot be shown or deleted.
fn nothing_kept(state: &AppState) -> Option<Why> {
    state
        .labelling
        .kept
        .is_none()
        .then_some(Why::Borrowed(NOTHING_KEPT))
}

/// Why the scan cannot be labelled again: the very sentence that says why it cannot be
/// labelled, where the button that labels is not in sight.
fn cannot_again(state: &AppState) -> Option<Why> {
    can_label(state).err().map(Why::Owned)
}

/// The action that chooses the model `way`, named `name`, as the menu beside the button that
/// labels offers it.
const fn model(
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
        words: MODEL,
        place: Place::Tab(Tab::Tools),
        does: Does::Act(does),
        ..PLAIN
    }
}

/// The action that asks for the folder of some files of labelling.
const fn folder(
    id: &'static str,
    name: &'static str,
    label: &'static str,
    wants: Wants,
) -> Command {
    Command {
        id,
        name,
        label,
        explains: "Where Volsa looks for these files, if not where they are usually kept",
        words: FOLDER,
        place: Place::Tab(Tab::Tools),
        does: Does::Ask(Request::Choose(Asked::Labelling(wants))),
        ..PLAIN
    }
}

/// The action of the labelling strip made from a change of the state of labelling.
fn labelling(action: LabellingAction) -> Action {
    Action::Labelling(action)
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "tools.label",
        name: LABEL_TOOL,
        label: LABEL_BUTTON,
        explains: "Outline about thirty parts of the brain on this scan, on this computer",
        words: &["segment", "parts", "synthseg", "labelling"],
        place: Place::Tab(Tab::Tools),
        why_not: |state| {
            can_label(state)
                .is_err()
                .then_some(Why::Borrowed(CANNOT_LABEL))
        },
        // The window says how much of the graphics card it holds itself, which the run is told.
        does: Does::Ask(Request::LabelScan),
        ..PLAIN
    },
    Command {
        id: "tools.label.stop",
        name: "Stop labelling the scan",
        label: "Stop",
        explains: "No labels are made",
        words: &["cancel", "abort", "segment"],
        place: Place::Tab(Tab::Tools),
        why_not: not_running,
        does: Does::Act(|_| Some(labelling(LabellingAction::StopLabelling))),
        ..PLAIN
    },
    Command {
        id: "tools.label.show_kept",
        name: "Show the labels kept for this series",
        label: "Show",
        explains: "Open the result kept on this computer",
        words: &["kept", "result", "open", "segment"],
        place: Place::Tab(Tab::Tools),
        why_not: nothing_kept,
        does: Does::Act(|_| Some(labelling(LabellingAction::ShowKept))),
        ..PLAIN
    },
    Command {
        id: "tools.label.again",
        name: "Label this scan again",
        label: "Make again",
        explains: "Run the labelling again, in place of showing the result kept",
        words: &["redo", "repeat", "segment"],
        place: Place::Tab(Tab::Tools),
        why_not: cannot_again,
        does: Does::Ask(Request::LabelScan),
        ..PLAIN
    },
    Command {
        id: "tools.label.delete_kept",
        name: "Delete the labels kept for this series",
        label: "Delete",
        explains: "They are removed from this computer and cannot be brought back",
        words: &["kept", "result", "forget", "remove"],
        place: Place::Tab(Tab::Tools),
        why_not: nothing_kept,
        does: Does::Ask(Request::Confirm(Confirm::DeleteKept)),
        ..PLAIN
    },
    model(
        "tools.model.automatic",
        "Pick the labelling model automatically",
        "Automatic",
        "Robust when the slices are more than 3 mm apart, otherwise Standard",
        |_| {
            Some(labelling(LabellingAction::SetModelChoice(
                ModelChoice::Automatic,
            )))
        },
    ),
    model(
        "tools.model.standard",
        "Use the standard labelling model",
        "Standard",
        "The standard model, whatever the scan",
        |_| {
            Some(labelling(LabellingAction::SetModelChoice(
                ModelChoice::Standard,
            )))
        },
    ),
    model(
        "tools.model.robust",
        "Use the robust labelling model",
        "Robust",
        "The robust model, whatever the scan",
        |_| {
            Some(labelling(LabellingAction::SetModelChoice(
                ModelChoice::Robust,
            )))
        },
    ),
    Command {
        id: "tools.label.cortex",
        name: "Label the cortex areas",
        label: "Cortex areas",
        explains: "Divide the cortex into its areas",
        words: CHOICES,
        place: Place::Tab(Tab::Tools),
        does: Does::Act(|state| {
            Some(labelling(LabellingAction::SetCortex(
                !state.labelling.cortex,
            )))
        }),
        ..PLAIN
    },
    Command {
        id: "tools.label.keep",
        name: "Keep results on this computer",
        label: "Keep results on this computer",
        explains: "Keep each result, to show again when the same series is opened",
        words: CHOICES,
        place: Place::Tab(Tab::Tools),
        does: Does::Act(|state| Some(labelling(LabellingAction::SetKeep(!state.labelling.keep)))),
        ..PLAIN
    },
    folder(
        "tools.label.models_folder",
        "Choose the folder of the models",
        "Folder of the models…",
        Wants::ModelsFolder,
    ),
    folder(
        "tools.label.runtime_folder",
        "Choose the folder of the runtime",
        "Folder of the runtime…",
        Wants::RuntimeFolder,
    ),
];
