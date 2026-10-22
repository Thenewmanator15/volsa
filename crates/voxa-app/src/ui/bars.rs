//! The bars of the window: the one across its top and the status line along its bottom.
//!
//! egui gives each panel its room in the order the panels are added, so the status line is
//! laid out first: the disclaimer on it has its room in a window of any size. The bar across
//! the top takes no more than [`BAR_SHARE`] of the window's height, and what it holds beyond
//! that scrolls, so that the pictures keep the rest.

use super::scene::export;
use super::{scene_bar, scrolled, status, toolbar};
use crate::action::Action;
use crate::state::AppState;

/// The most of the window's height that the bar across the top takes.
pub const BAR_SHARE: f32 = 0.4;

/// What the bar across the top was asked for that is not a change of state, and what the
/// person asked to save of the 3D scene, if anything.
pub type Asked = (toolbar::Asked, Option<export::Asked>);

/// Lay out the status line along the bottom of the window and the bar across its top, in
/// that order, and leave the rest of the window to what is added after. `list_shown` says
/// whether the series list is shown. Gives what the bar was asked for, and the disclaimer as
/// it was laid out.
pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    list_shown: bool,
    actions: &mut Vec<Action>,
) -> (Asked, egui::Response) {
    let notices = &state.notices;
    let line = egui::Panel::bottom("status").show(ui, |ui| status::show(ui, notices, actions));
    let most = BAR_SHARE * ui.ctx().content_rect().height();
    let bar = egui::Panel::top("toolbar").show(ui, |ui| {
        // In a window too small for all of the bar, it scrolls, with its bar in sight. The
        // panel keeps the height it had the frame before, so the lines are given their
        // height outright: as much as they need, up to the most.
        scrolled::solid_bars(ui);
        let lines = egui::ScrollArea::vertical()
            .id_salt("the lines of the bar")
            .auto_shrink([false, true])
            .max_height(most)
            .min_scrolled_height(most);
        let shown = lines.show(ui, |ui| {
            let asked = toolbar::show(ui, state, list_shown, actions);
            (asked, scene_bar::show(ui, state, actions))
        });
        shown.inner
    });
    (bar.inner, line.inner)
}

#[cfg(test)]
mod tests;
