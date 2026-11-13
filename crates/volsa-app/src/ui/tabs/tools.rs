//! The Tools tab: the tools that work on the scan. Under "Part of Volsa", labelling a brain
//! scan, with its state, progress and stop; under "Added by you", the plugins the person has
//! added, and the button that adds one.

use egui::{RichText, Ui};

use super::section;
use crate::action::Action;
use crate::plugins::tool_entries;
use crate::state::AppState;
use crate::ui::commands::{self, Request};
use crate::ui::parts::labelling;
use crate::ui::plugins::plugin_section;
use crate::ui::theme;

/// What the tab calls the tool that labels a brain scan, and what searching for it finds.
pub const LABEL_TOOL: &str = "Label this brain scan";
/// What the button that runs the tool says.
pub const LABEL_BUTTON: &str = "Label this scan";
/// What the tab calls the tool that highlights veins, and what searching for it finds.
pub const VEINS_TOOL: &str = "Highlight veins";
/// What the button that runs it says.
pub const VEINS_BUTTON: &str = "Highlight veins";
/// What the tool does, said under its button.
pub const VEINS_ABOUT: &str = "Highlights thin dark tubes, mostly veins on an SWI or T2* scan, \
                               over the slices and in 3D, on top of any labels open; in 3D \
                               they show through the brain. A filter, \
                               not a trained model: some are not veins, and not every vein is \
                               found.";
/// What the switch that shows or hides the veins found says.
pub const VEINS_SHOW: &str = "Show veins";
/// What the slider that sets how strongly they show says. In 3D they show through the brain,
/// fainter where they lie behind its surface.
pub const VEINS_STRENGTH: &str = "Strength";
/// The heading over the tools that come with Volsa.
const PART_OF_VOLSA: &str = "Part of Volsa";

/// Every sentence the tab keeps in a constant, for the check that none suggests a finding.
#[cfg(test)]
pub(super) const TEXTS: [&str; 5] = [
    PART_OF_VOLSA,
    VEINS_TOOL,
    VEINS_ABOUT,
    VEINS_SHOW,
    VEINS_STRENGTH,
];

/// Draw the Tools tab: under "Part of Volsa", labelling a brain scan; then the plugins the
/// person has added, each with its buttons, and the button that adds one. Changes of state go
/// onto `actions`; what its buttons ask of the window, such as labelling the scan, choosing a
/// folder for the models or choosing a plugin's folder, is given back.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    section(ui, PART_OF_VOLSA);
    ui.label(RichText::new(LABEL_TOOL).strong());
    let mut asked = labelling::strip(ui, state, actions);
    ui.add_space(8.0);
    ui.label(RichText::new(VEINS_TOOL).strong());
    // Once found, the veins are a part of the labels open: a switch shows or hides them.
    if let Some(part) = crate::labels::veins_part(state) {
        let mut shown = state.labels.is_shown(part);
        if ui.checkbox(&mut shown, VEINS_SHOW).changed() {
            actions.push(Action::PartShow { part, shown });
        }
        let mut opacity = state.labels.shown.get(part).map_or(1.0, |how| how.opacity);
        let slider = egui::Slider::new(&mut opacity, crate::labels::FAINTEST..=1.0)
            .text(VEINS_STRENGTH)
            .show_value(false);
        if ui.add_enabled(shown, slider).changed() {
            actions.push(Action::PartOpacity { part, opacity });
        }
    } else if let Some(does) = commands::button(ui, "tools.veins", state, None) {
        asked.extend(does.run(state, actions));
    }
    let why = crate::labels::can_find_veins(state).err();
    let quiet = theme::palette(ui.ctx()).quiet_text;
    ui.label(
        RichText::new(why.unwrap_or(VEINS_ABOUT))
            .small()
            .color(quiet),
    );
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
