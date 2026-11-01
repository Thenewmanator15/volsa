//! The strip in the Tools tab, where a person labels the scan with Volsa's own networks: the
//! button and what it will do, the choices behind it, how far a run has got and the way to
//! stop it, and the result kept for the series. It is also how labels that Volsa made are
//! described, in the Labels tab and over the pictures.

use egui::{RichText, Ui};
use egui_wgpu::RenderState;
use volsa_segment::Stage;

use crate::action::Action;
use crate::labelling::{LabellingAction, can_label, what_will_run};
use crate::labels::on_this_series;
use crate::state::AppState;
use crate::ui::commands::{self, Request};
use crate::ui::theme;

mod made;
mod menu;

#[cfg(test)]
pub use made::verdict_in_plain_words;
pub use made::{made_by, save_as, solids_in_words, verdict_line};
#[cfg(test)]
pub use menu::{menu_id, places_named};

/// Said while a run is on.
const SLOW: &str = "The 3D view may be slow while the graphics card is busy.";
/// Said where the labelling is not offered because labels are already open on the series.
const OPEN_ALREADY: &str =
    "Labels are open on this series. Close them in the Labels tab to label it again.";
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
/// choices) and while a result is kept for it. With labels open and nothing kept it says why
/// there is nothing to offer. Gives what the strip asks of the window: to label the scan,
/// which the window does once it can say how much of the graphics card it holds itself, and
/// what the person asked to be chosen in a dialog.
pub fn strip(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let labelling = &state.labelling;
    let none_open = !on_this_series(state);
    let mut asked = Vec::new();
    match (labelling.running, &labelling.kept) {
        (Some(reached), _) => running(ui, reached, state, (actions, &mut asked)),
        (None, kept) => {
            if none_open {
                ready(ui, state, actions, &mut asked);
            } else if kept.is_none() {
                ui.label(quiet(ui, OPEN_ALREADY));
            }
            if let Some(when) = kept {
                keeping(ui, state, when, actions, &mut asked);
            }
        }
    }
    asked
}

/// The words for a quiet line.
fn quiet(ui: &Ui, text: &str) -> RichText {
    let quiet = theme::palette(ui.ctx()).quiet_text;
    RichText::new(text).small().color(quiet)
}

/// What labels the scan once the window has been asked to, told how much of the graphics card
/// the window draws on it holds itself (see [`held_on_card`]), which only the window can say.
pub fn label_when_asked(state: &AppState, render: Option<&RenderState>) -> Action {
    Action::Labelling(LabellingAction::LabelScan {
        held: held_on_card(state, render),
    })
}

/// A stage of a run, in plain words.
pub fn stage_in_words(stage: Stage) -> String {
    match stage {
        Stage::Preparing => "Preparing the scan".to_owned(),
        Stage::Finding { step, of } => format!("Locating the structures, {step} of {of}"),
        Stage::Cortex => "Dividing the cortex".to_owned(),
        Stage::Checking => "The model is checking its work".to_owned(),
        Stage::Tidying => "Tidying up".to_owned(),
    }
}

/// A run is on: the stage it has reached, the way to stop it, and that the 3D view may be slow.
fn running(
    ui: &mut Ui,
    reached: Stage,
    state: &AppState,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    ui.horizontal(|ui| {
        ui.label(stage_in_words(reached));
        commands::offer(ui, "tools.label.stop", state, None, actions, asked);
    });
    ui.label(quiet(ui, SLOW));
}

/// No run is on and no labels are open: the button, from the list of actions, the menu beside
/// it and, beneath them, what the button will do, or why it cannot do anything.
fn ready(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    let can = can_label(state);
    ui.horizontal(|ui| {
        if let Some(does) = commands::button(ui, "tools.label", state, None) {
            asked.extend(does.run(state, actions));
        }
        menu::options(ui, state, actions, asked);
    });
    ui.label(quiet(
        ui,
        &can.err().unwrap_or_else(|| what_will_run(state)),
    ));
}

/// A result is kept for the series: when it was made, and the ways to show, make and delete it.
fn keeping(
    ui: &mut Ui,
    state: &AppState,
    when: &str,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    ui.label(format!("Labels made on {when} are kept for this series."));
    ui.horizontal_wrapped(|ui| {
        for id in [
            "tools.label.show_kept",
            "tools.label.again",
            "tools.label.delete_kept",
        ] {
            commands::offer_small(ui, id, state, None, (actions, asked));
        }
    });
}

/// How many bytes of the graphics card the window draws on Volsa holds itself, as the renderer
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
