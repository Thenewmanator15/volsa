//! The bars of the window: the one across its top and the line along its bottom.
//!
//! egui gives each panel its room in the order the panels are added, so the line along the
//! bottom is laid out first: the disclaimer on it has its room in a window of any size. The
//! bar across the top is one line, and takes what that needs.

use super::commands::Request;
use super::search::Search;
use super::theme::ThemeChoice;
use super::{notices, top};
use crate::action::Action;
use crate::state::AppState;

/// What the bars keep between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Kept {
    /// The theme the top bar's button last chose, which the window will be drawn in.
    pub theme: ThemeChoice,
    /// Whether the list of notices above the bottom line is open.
    pub notices_open: bool,
}

/// Lay out the line along the bottom of the window and the bar across its top, in that order,
/// and leave the rest of the window to what is added after. `search` is told where the bar's
/// search box is; `kept` says which theme the window is drawn in and whether the list of
/// notices is open, and is changed by the bars. Gives what the bar was asked for, and the disclaimer as it
/// was laid out.
pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    search: &mut Search,
    kept: &mut Kept,
    actions: &mut Vec<Action>,
) -> (Vec<Request>, egui::Response) {
    let Kept {
        theme,
        notices_open,
    } = kept;
    let notices = &state.notices;
    let line = egui::Panel::bottom("notice line")
        .show(ui, |ui| notices::show(ui, notices, notices_open, actions));
    let bar =
        egui::Panel::top("top bar").show(ui, |ui| top::show(ui, state, search, *theme, actions));
    (bar.inner, line.inner)
}

#[cfg(test)]
mod tests;
