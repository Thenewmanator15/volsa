//! The window keeping what the person set from one run to the next.

use super::VoxaApp;
use crate::prefs::Prefs;

impl VoxaApp {
    /// Put the state as the run before left it.
    pub(super) fn recall(&mut self) {
        let remembered = self.kept.remembered().clone();
        for action in remembered.actions(self.session.state()) {
            self.session.dispatch(action);
        }
    }

    /// Keep the preferences if they have changed, once the window has opened. While a button
    /// is held the window, the series list or the column may be being dragged to a new size,
    /// and nothing is written until it is let go.
    pub(super) fn remember(&mut self, ctx: &egui::Context) {
        let held = ctx.input(|input| input.pointer.any_down());
        if held || !self.screen.has_opened() {
            return;
        }
        self.screen.look(ctx);
        let arrangement = self.screen.arrangement(ctx.zoom_factor());
        let now = Prefs::of(self.session.state(), arrangement);
        self.kept.keep(&now);
    }
}
