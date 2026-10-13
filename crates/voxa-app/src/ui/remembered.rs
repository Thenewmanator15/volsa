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

    /// Keep the preferences if they have changed. While a button is held the window may be
    /// being dragged to a new size, and nothing is written until it is let go.
    pub(super) fn remember(&mut self, ctx: &egui::Context) {
        let (held, inside) = ctx.input(|input| {
            let inside = input.viewport().inner_rect;
            (input.pointer.any_down(), inside)
        });
        if held {
            return;
        }
        let size = inside.map(|rect| [rect.width(), rect.height()]);
        let now = Prefs::of(self.session.state(), size.or(self.kept.remembered().window));
        self.kept.keep(&now);
    }
}
