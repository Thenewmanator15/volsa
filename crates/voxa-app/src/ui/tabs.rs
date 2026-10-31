//! What each tab of the panel at the right of the window holds, and the window's way of
//! drawing the panel with the tab that is open.
//!
//! The View, Measure and 3D tabs are here in full. Labels and Tools still have their content
//! elsewhere, until Task 8 brings it in; each says where.

use egui::{Rect, RichText, Ui};

use super::commands::{self, Request};
use super::panel::{self, Tab};
use super::{VoxaApp, shares, theme};
use crate::action::Action;
use crate::state::AppState;

mod measure;
mod scene;
mod view;

pub use measure::{MeasureTab, NO_VOLUME, NOTHING_MEASURED, Save, save as save_measurements};
pub use scene::{LOOKS, PARTS_HINT, PARTS_NEED_LABELS, VIEWS};
pub use view::{LINK_SAYS, SMOOTH_SAYS};

/// What the Labels tab says until the labelled parts are in it.
const LABELS_ELSEWHERE: &str =
    "The labelled parts are in the Parts tab above the column of pictures for now.";
/// What the Tools tab says until labelling a scan is in it.
const TOOLS_ELSEWHERE: &str =
    "Labelling a scan is in the Parts tab above the column of pictures for now.";

/// Draw what `tab` holds. `measuring` is what the Measure tab keeps between frames. Changes of
/// state go onto `actions`; what its buttons ask of the window is given back.
pub fn show(
    ui: &mut Ui,
    tab: Tab,
    state: &AppState,
    measuring: &mut MeasureTab,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    match tab {
        Tab::View => view::show(ui, state, actions),
        Tab::Scene => scene::show(ui, state, actions),
        Tab::Measure => measure::show(ui, state, measuring, actions),
        Tab::Labels => elsewhere(ui, LABELS_ELSEWHERE),
        Tab::Tools => elsewhere(ui, TOOLS_ELSEWHERE),
    }
}

/// A tab whose content is not in it yet: one line that says where it is today.
fn elsewhere(ui: &mut Ui, says: &str) -> Vec<Request> {
    ui.label(RichText::new(says).color(theme::QUIET_TEXT));
    Vec::new()
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
        let measuring = &mut self.measure;
        let tab_in = |ui: &mut Ui, tab| show(ui, tab, state, measuring, actions);
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
