//! The questions asked before something happens to a plugin: before a run on a scan whose
//! description names another contrast than the plugin expects, and before a plugin is removed.

use egui::{RichText, Ui};

use super::{question, quiet};
use crate::action::Action;
use crate::plugins::{PluginAction, Warning};
use crate::ui::theme;

/// What is said beneath the question before a plugin is removed.
const REMOVING: &str = "Volsa deletes its own copy of the plugin. The folder you added it from \
                        is left as it is.";

/// Draw the warning before a run. Gives a run only when Run anyway is pressed; Cancel when
/// that is.
pub fn contrast_warning(ctx: &egui::Context, warning: &Warning) -> Option<Action> {
    let body = |ui: &mut Ui| {
        ui.label(RichText::new(&warning.warning).color(theme::WARNING));
        let note = format!(
            "If it runs, its result carries this note: {}",
            warning.stamp
        );
        ui.label(quiet(&note));
    };
    let run = PluginAction::ConfirmRun;
    question(ctx, "Run this plugin?", body, "Run anyway", run)
}

/// Draw the question before the plugin `name` is removed. Gives the removal only when Remove
/// is pressed; Cancel when that is.
pub fn remove_question(ctx: &egui::Context, name: &str) -> Option<Action> {
    let body = |ui: &mut Ui| {
        ui.label(format!("Remove {name} from Volsa?"));
        ui.label(quiet(REMOVING));
    };
    let remove = PluginAction::ConfirmRemove;
    question(ctx, "Remove a plugin", body, "Remove", remove)
}
