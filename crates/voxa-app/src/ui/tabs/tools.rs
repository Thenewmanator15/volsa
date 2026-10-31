//! The Tools tab: the tools that work on the scan. Under "Part of Voxa", labelling a brain
//! scan, with its state, progress and stop; under "Added by you", the tools the person has
//! added, of which there are none until the plugin engine is built, so it says so.

use egui::{RichText, Ui};

use super::section;
use crate::action::Action;
use crate::state::AppState;
use crate::tools::{ADDED_BY_YOU, MadeBy, ToolInfo};
use crate::ui::commands::Request;
use crate::ui::{parts::labelling, theme};

/// What the tab calls the tool that labels a brain scan, and what searching for it finds.
pub const LABEL_TOOL: &str = "Label this brain scan";
/// What the button that runs the tool says.
pub const LABEL_BUTTON: &str = "Label this scan";
/// The heading over the tools that come with Voxa.
const PART_OF_VOXA: &str = "Part of Voxa";
/// The heading over the tools the person added.
const ADDED_HEADING: &str = "Added by you";
/// Said where the tools added by the person would be listed, while there are none.
const NONE_ADDED: &str =
    "Tools you add will be listed here. Adding tools comes in a later version of Voxa.";

/// A line of quiet words.
fn quiet(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .small()
            .color(theme::palette(ui.ctx()).quiet_text),
    );
}

/// A line of the entry of a tool, if there are words to put on it.
fn line(ui: &mut Ui, what: &str, words: &str) {
    if !words.is_empty() {
        quiet(ui, &format!("{what}: {words}"));
    }
}

/// What is said of a tool's files: whether they passed their check. A check that was not
/// passed is the colour of what to look at.
fn files_checked(ui: &mut Ui, checked: Option<bool>) {
    let palette = theme::palette(ui.ctx());
    let (words, colour) = match checked {
        Some(true) => ("Its files passed their check.", palette.quiet_text),
        Some(false) => ("Its files did not pass their check.", palette.warning),
        None => ("Its files have not been checked.", palette.quiet_text),
    };
    ui.label(RichText::new(words).small().color(colour));
}

/// The entry of one tool the person added: its name, who made it, where it came from, its
/// licence, whether its files passed their check, and what it accepts and returns.
fn entry(ui: &mut Ui, tool: &ToolInfo) {
    ui.add_space(4.0);
    ui.label(RichText::new(&tool.name).strong());
    quiet(
        ui,
        match tool.made_by {
            MadeBy::Voxa => "Made by Voxa",
            MadeBy::AddedByYou => ADDED_BY_YOU,
        },
    );
    line(ui, "Came from", &tool.source);
    line(ui, "Licence", &tool.licence);
    files_checked(ui, tool.checked);
    line(ui, "Accepts", &tool.accepts);
    line(ui, "Returns", &tool.returns);
}

/// Draw the Tools tab. `added` is the tools the person has added. Changes of state go onto
/// `actions`; what its buttons ask of the window, such as labelling the scan or choosing a
/// folder for the models, is given back.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    added: &[ToolInfo],
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    section(ui, PART_OF_VOXA);
    ui.label(RichText::new(LABEL_TOOL).strong());
    let asked = labelling::strip(ui, state, actions);
    ui.add_space(8.0);
    section(ui, ADDED_HEADING);
    if added.is_empty() {
        quiet(ui, NONE_ADDED);
    }
    for tool in added {
        entry(ui, tool);
    }
    asked
}

#[cfg(test)]
mod tests;
