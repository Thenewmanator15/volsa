//! What each tab of the panel at the right of the window holds, and the window's way of
//! drawing the panel with the tab that is open.
//!
//! Each of the five tabs is in a file of its own.

use egui::{Rect, RichText, Ui};

use super::commands::{Request, offer, offer_small};
use super::panel::{self, Tab};
use super::parts::PartsView;
use super::{VolsaApp, layout, shares, theme};
use crate::action::Action;
use crate::state::AppState;

mod labels;
#[cfg(test)]
pub(super) mod made_up;
mod measure;
mod scene;
#[cfg(test)]
mod themes_tests;
mod tools;
mod view;

pub use measure::{MeasureTab, NOTHING_MEASURED, Save, save as save_measurements};
pub use scene::{LOOKS, PARTS_HINT, PARTS_NEED_LABELS, VIEWS};
pub use tools::{LABEL_BUTTON, LABEL_TOOL, VEINS_BUTTON, VEINS_TOOL};
pub use view::{LINK_SAYS, SMOOTH_SAYS};

/// Every sentence the tabs keep in a constant, and the label and hint of each look and side of
/// the 3D scene, for the check that none suggests a finding.
#[cfg(test)]
pub(super) fn fixed_texts() -> Vec<&'static str> {
    let own = [
        PARTS_HINT,
        PARTS_NEED_LABELS,
        scene::NO_3D_VIEW,
        LINK_SAYS,
        SMOOTH_SAYS,
        NOTHING_MEASURED,
        LABEL_TOOL,
        LABEL_BUTTON,
    ];
    let looks = LOOKS.iter().flat_map(|(_, label, hint)| [*label, *hint]);
    let sides = VIEWS.iter().flat_map(|(_, label, hint)| [*label, *hint]);
    let kept = [
        labels::TEXTS.as_slice(),
        tools::TEXTS.as_slice(),
        view::TEXTS.as_slice(),
    ];
    own.into_iter()
        .chain(crate::naming::about_texts())
        .chain(looks)
        .chain(sides)
        .chain(kept.concat())
        .collect()
}

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
    match tab {
        Tab::View => view::show(ui, state, actions),
        Tab::Scene => scene::show(ui, state, actions),
        Tab::Measure => measure::show(ui, state, &mut kept.measuring, actions),
        Tab::Labels => labels::show(ui, state, &mut kept.parts, actions),
        Tab::Tools => tools::show(ui, state, actions),
    }
}

/// The small heading above a group of controls in a tab.
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title)
            .small()
            .color(theme::palette(ui.ctx()).quiet_text),
    );
}

/// Draw the small button that chooses one way of several, drawn pressed while it is the way in
/// force and then not asked for again.
fn choose_small(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    chosen: bool,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    if chosen {
        let nothing = (&mut Vec::new(), &mut Vec::new());
        offer_small(ui, id, state, Some(true), nothing);
    } else {
        offer_small(ui, id, state, Some(false), (actions, asked));
    }
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

impl VolsaApp {
    /// Draw the tab panel down the right of the window, after the panels beside it, with the
    /// tab that is open, as wide as the window gives it. What a tab's buttons ask of the window
    /// is done. Gives where the panel is while it is open, for its left edge to be dragged.
    pub(super) fn tab_panel(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) -> Option<Rect> {
        let state = self.session.state();
        let kept = &mut self.tabs;
        let tab_in = |ui: &mut Ui, tab| show(ui, tab, state, kept, actions);
        let wide = self.screen.widths(ui.ctx().content_rect().width()).panel;
        let shown = panel::show(ui, &mut self.screen.panel, state, wide, tab_in);
        for request in shown.requests {
            self.ask(ui.ctx(), request);
        }
        (!self.screen.panel.folded).then_some(shown.rect)
    }

    /// Draw the pictures in the middle of the window, then the tab panel at their right. The
    /// Tab key goes through what is drawn in the order it is drawn, and is to go through the
    /// pictures before the panel (spec 7.14), so the pictures are drawn first, in the room the
    /// panel leaves, and the panel after them. Gives where the panel is while it is open, for
    /// its left edge to be dragged, and where the pictures are.
    pub(super) fn pictures_then_panel(
        &mut self,
        ui: &mut Ui,
        frame: &eframe::Frame,
        actions: &mut Vec<Action>,
    ) -> (Option<Rect>, Rect) {
        let wide = self.screen.widths(ui.ctx().content_rect().width()).panel;
        let mut middle_ui = layout::beside_the_panel(ui, wide);
        let pictures =
            egui::CentralPanel::default()
                .frame(super::middle())
                .show(&mut middle_ui, |ui| {
                    self.pictures(ui, frame.wgpu_render_state(), actions);
                });
        (self.tab_panel(ui, actions), pictures.response.rect)
    }

    /// Let the left edge of the tab panel, at `at`, be dragged to give it more or less of
    /// the window's width. Made after everything beside it, so that the drag is the edge's.
    pub(super) fn drag_the_panel(&mut self, ui: &Ui, at: Option<Rect>) {
        let Some(at) = at else {
            return;
        };
        let edge = egui::Id::new("tab panel edge");
        let by = shares::divider(ui, edge, at.left(), at.y_range(), "Width of the tab panel");
        if by.abs() > 0.0 {
            let window = ui.ctx().content_rect().width();
            let share = self.screen.panel.share;
            self.screen.panel.share = panel::dragged(window, share, by);
        }
    }
}
