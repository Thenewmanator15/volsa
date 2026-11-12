//! The Tools tab: the tools that work on the scan. Under "Part of Volsa", labelling a brain
//! scan, with its state, progress and stop; under "Added by you", the plugins the person has
//! added, and the button that adds one.

use egui::{RichText, Ui};

use super::section;
use crate::action::Action;
use crate::plugins::tool_entries;
use crate::state::AppState;
use crate::ui::commands::Request;
use crate::ui::parts::labelling;
use crate::ui::plugins::plugin_section;

/// What the tab calls the tool that labels a brain scan, and what searching for it finds.
pub const LABEL_TOOL: &str = "Label this brain scan";
/// What the button that runs the tool says.
pub const LABEL_BUTTON: &str = "Label this scan";
/// The heading over the tools that come with Volsa.
const PART_OF_VOLSA: &str = "Part of Volsa";

/// Every sentence the tab keeps in a constant, for the check that none suggests a finding.
#[cfg(test)]
pub(super) const TEXTS: [&str; 1] = [PART_OF_VOLSA];

/// Draw the Tools tab: under "Part of Volsa", labelling a brain scan; then the plugins the
/// person has added, each with its buttons, and the button that adds one. Changes of state go
/// onto `actions`; what its buttons ask of the window, such as labelling the scan, choosing a
/// folder for the models or choosing a plugin's folder, is given back.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    section(ui, PART_OF_VOLSA);
    ui.label(RichText::new(LABEL_TOOL).strong());
    let mut asked = labelling::strip(ui, state, actions);
    ui.add_space(8.0);
    let entries = tool_entries(&state.plugins);
    let busy = state.plugins.busy.as_deref();
    // What the window holds of the graphics card is only known with the device in hand: a run
    // asked for here is told it as the window takes the action in.
    if plugin_section(ui, &entries, busy, 0, actions) {
        asked.push(Request::AddPlugin);
    }
    asked
}

#[cfg(test)]
mod tests;
