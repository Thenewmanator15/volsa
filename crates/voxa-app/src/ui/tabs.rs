//! What each tab of the panel at the right of the window holds, and the window's way of
//! drawing the panel with the tab that is open.
//!
//! Each of the five tabs is in a file of its own.

use egui::{Rect, RichText, Ui};

use super::commands::{self, Request};
use super::panel::{self, Tab};
use super::parts::PartsView;
use super::{VoxaApp, shares, theme};
use crate::action::Action;
use crate::state::AppState;
use crate::tools::ToolInfo;

mod labels;
#[cfg(test)]
pub(super) mod made_up;
mod measure;
mod scene;
mod tools;
mod view;

pub use measure::{MeasureTab, NO_VOLUME, NOTHING_MEASURED, Save, save as save_measurements};
pub use scene::{LOOKS, PARTS_HINT, PARTS_NEED_LABELS, VIEWS};
pub use tools::{LABEL_BUTTON, LABEL_TOOL};
pub use view::{LINK_SAYS, SMOOTH_SAYS};

/// What the tabs keep between frames.
#[derive(Debug, Default)]
pub struct Kept {
    /// What the Measure tab keeps: the name being typed.
    pub measuring: MeasureTab,
    /// What the Labels tab keeps: the search typed, and the group to scroll to.
    pub parts: PartsView,
}

/// Draw what `tab` holds. `kept` is what the tabs keep between frames. Changes of state go onto
/// `actions`; what its buttons ask of the window is given back.
pub fn show(
    ui: &mut Ui,
    tab: Tab,
    state: &AppState,
    kept: &mut Kept,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    // The tools added by the person: the plugin engine fills this list once there is one.
    let added: &[ToolInfo] = &[];
    match tab {
        Tab::View => view::show(ui, state, actions),
        Tab::Scene => scene::show(ui, state, actions),
        Tab::Measure => measure::show(ui, state, &mut kept.measuring, actions),
        Tab::Labels => labels::show(ui, state, &mut kept.parts, actions),
        Tab::Tools => tools::show(ui, state, added, actions),
    }
}

/// The small heading above a group of controls in a tab.
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).small().color(theme::QUIET_TEXT));
}

/// A row of controls that goes onto the next line, whole, when the panel is too narrow for it.
fn row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let wrapped = ui.horizontal_wrapped(|ui| {
        // What does not fit goes onto the next line, whole, not broken inside its words.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add(ui)
    });
    wrapped.inner
}

/// Draw the button of the action `id`, and take what it does: a change of state onto
/// `actions`, anything it asks of the window onto `asked`. Nothing is left undone.
fn offer(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    selected: Option<bool>,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    if let Some(does) = commands::button(ui, id, state, selected) {
        asked.extend(does.run(state, actions));
    }
}

impl VoxaApp {
    /// Draw the tab panel down the right of the window, after the panels beside it, with the
    /// tab that is open. What a tab's buttons ask of the window is done. Gives where the panel
    /// is while it is open, for its left edge to be dragged.
    pub(super) fn tab_panel(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) -> Option<Rect> {
        let state = self.session.state();
        let kept = &mut self.tabs;
        let tab_in = |ui: &mut Ui, tab| show(ui, tab, state, kept, actions);
        let shown = panel::show(ui, &mut self.screen.panel, state, tab_in);
        for request in shown.requests {
            self.ask(ui.ctx(), request);
        }
        (!self.screen.panel.folded).then_some(shown.rect)
    }

    /// Let the left edge of the tab panel, at `at`, be dragged to give it more or less of
    /// the window's width. Made after everything beside it, so that the drag is the edge's.
    pub(super) fn drag_the_panel(&mut self, ui: &Ui, at: Option<Rect>) {
        let Some(at) = at else {
            return;
        };
        let edge = egui::Id::new("tab panel edge");
        let by = shares::divider(ui, edge, at.left(), at.y_range());
        if by.abs() > 0.0 {
            let window = ui.ctx().content_rect().width();
            let share = self.screen.panel.share;
            self.screen.panel.share = panel::dragged(window, share, by);
        }
    }
}
