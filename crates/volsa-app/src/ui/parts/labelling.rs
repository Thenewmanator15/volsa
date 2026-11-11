//! The strip at the top of the Parts tab, where a person labels the scan with Voxa's own
//! networks: the button and what it will do, the choices behind it, how far a run has got and
//! the way to stop it, and the result kept for the series. It is also how labels that Voxa
//! made are described, here and over the pictures.

use egui::{RichText, Ui};
use egui_wgpu::RenderState;
use voxa_segment::Stage;

use super::super::theme;
use crate::action::Action;
use crate::labelling::{LabellingAction, can_label, what_will_run};
use crate::labels::on_this_series;
use crate::state::AppState;

mod made;
mod menu;

pub use made::{made_by, save_as, solids_in_words, verdict_line};
#[cfg(test)]
pub use menu::menu_id;

/// The button that starts a run.
const LABEL: &str = "Label this scan";
/// Said while a run is on.
const SLOW: &str = "The 3D view may be slow while the graphics card is busy.";
/// Where the labels are saved from the dialog, unless the person names another file.
const FILE_NAME: &str = "labels.nii.gz";

/// What the strip asks the person to choose with a dialog of the system's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wants {
    /// The folder the model files are in.
    ModelsFolder,
    /// The folder the runtime files are in.
    RuntimeFolder,
    /// Where to save the labels that are open.
    SaveLabels,
}

/// Draw the strip. It is shown while a run is on (the stage and a way to stop), and otherwise
/// while no labels are open on the series (the button, what it will do, and the menu of
/// choices) and while a result is kept for it. `held` is how many bytes of the graphics card
/// the window draws on Voxa holds itself, which a run is told. With labels open and nothing
/// kept it draws nothing: the tab is as it was before Voxa could label. Gives what the person
/// asked to be chosen in a dialog.
pub fn strip(ui: &mut Ui, state: &AppState, held: u64, actions: &mut Vec<Action>) -> Option<Wants> {
    let labelling = &state.labelling;
    let none_open = !on_this_series(state);
    if labelling.running.is_none() && !none_open && labelling.kept.is_none() {
        return None;
    }
    let mut wants = None;
    match (labelling.running, &labelling.kept) {
        (Some(reached), _) => running(ui, reached, actions),
        (None, kept) => {
            if none_open {
                wants = ready(ui, state, held, actions);
            }
            if let Some(when) = kept {
                keeping(ui, state, when, held, actions);
            }
        }
    }
    ui.separator();
    wants
}

/// The words for a quiet line.
fn quiet(text: &str) -> RichText {
    RichText::new(text).small().color(theme::QUIET_TEXT)
}

/// What asks for a run.
fn label_scan(held: u64) -> Action {
    Action::Labelling(LabellingAction::LabelScan { held })
}

/// A stage of a run, in plain words.
pub fn stage_in_words(stage: Stage) -> String {
    match stage {
        Stage::Preparing => "Preparing the scan".to_owned(),
        Stage::Finding { step, of } => format!("Finding the structures, {step} of {of}"),
        Stage::Cortex => "Dividing the cortex".to_owned(),
        Stage::Checking => "The model is checking its work".to_owned(),
        Stage::Tidying => "Tidying up".to_owned(),
    }
}

/// A run is on: the stage it has reached, the way to stop it, and that the 3D view may be slow.
fn running(ui: &mut Ui, stage: Stage, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(stage_in_words(stage));
        if ui.button("Stop").clicked() {
            actions.push(Action::Labelling(LabellingAction::StopLabelling));
        }
    });
    ui.label(quiet(SLOW));
}

/// No run is on and no labels are open: the button, the menu beside it and, beneath them,
/// what the button will do, or why it cannot do anything.
fn ready(ui: &mut Ui, state: &AppState, held: u64, actions: &mut Vec<Action>) -> Option<Wants> {
    let can = can_label(state);
    let wants = ui
        .horizontal(|ui| {
            if ui
                .add_enabled(can.is_ok(), egui::Button::new(LABEL))
                .clicked()
            {
                actions.push(label_scan(held));
            }
            menu::options(ui, &state.labelling, actions)
        })
        .inner;
    ui.label(quiet(&can.err().unwrap_or_else(|| what_will_run(state))));
    wants
}

/// A result is kept for the series: when it was made, and the ways to show, make and delete it.
fn keeping(ui: &mut Ui, state: &AppState, when: &str, held: u64, actions: &mut Vec<Action>) {
    ui.label(format!("Labels made on {when} are kept for this series."));
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("Show").clicked() {
            actions.push(Action::Labelling(LabellingAction::ShowKept));
        }
        let can = can_label(state);
        let again = ui.add_enabled(can.is_ok(), egui::Button::new("Make again").small());
        if let Err(why) = &can {
            again.on_disabled_hover_text(why.as_str());
        } else if again.clicked() {
            actions.push(label_scan(held));
        }
        if ui.small_button("Delete").clicked() {
            actions.push(Action::Labelling(LabellingAction::DeleteKept));
        }
    });
}

/// How many bytes of the graphics card the window draws on Voxa holds itself, as the renderer
/// counts what it has asked the device for. A run on that card is told, so that it leaves room
/// for it. Nothing where there is no graphics device, where the renderer keeps no count, and
/// while labelling cannot run (nothing then uses the figure).
pub fn held_on_card(state: &AppState, render: Option<&RenderState>) -> u64 {
    if can_label(state).is_err() {
        return 0;
    }
    let report = render.and_then(|render| render.device.generate_allocator_report());
    report.map_or(0, |report| report.total_reserved_bytes)
}

/// Ask the person for what the strip wants chosen, and give what comes of it. `None` when
/// they chose nothing.
pub fn chosen(wants: Wants) -> Option<Action> {
    let labelling = |action| Some(Action::Labelling(action));
    match wants {
        Wants::ModelsFolder => {
            let dialog = rfd::FileDialog::new().set_title("Choose the folder of the model files");
            labelling(LabellingAction::SetModelsFolder(dialog.pick_folder()?))
        }
        Wants::RuntimeFolder => {
            let dialog = rfd::FileDialog::new().set_title("Choose the folder of the runtime files");
            labelling(LabellingAction::SetRuntimeFolder(dialog.pick_folder()?))
        }
        Wants::SaveLabels => {
            let dialog = rfd::FileDialog::new()
                .set_title("Save the labels as a file")
                .set_file_name(FILE_NAME)
                .add_filter("NIfTI label volume", &["gz"]);
            labelling(LabellingAction::SaveLabelsAs(dialog.save_file()?))
        }
    }
}

#[cfg(test)]
mod made_tests;
#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod tests;
