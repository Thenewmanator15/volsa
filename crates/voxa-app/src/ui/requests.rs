//! What a person asks of the window rather than of the scan, from a button or a key: the
//! windows to open or close, the size of the interface, choosing what to open and where to
//! save.

use super::commands::{Request, Tab};
use super::{VoxaApp, measure, parts, screen, toolbar};
use crate::action::Action;

impl VoxaApp {
    /// Do what was asked of the window.
    pub(super) fn ask(&mut self, ctx: &egui::Context, request: Request) {
        match request {
            Request::Open(pick) => {
                if let Some(path) = toolbar::picked(pick) {
                    let opening = self.labels.opening(path);
                    self.dispatch(opening);
                }
            }
            Request::Help(None) => self.help_open = !self.help_open,
            // Opened at its topic once there are topics (Task 19).
            Request::Help(Some(_)) => self.help_open = true,
            Request::About => self.about_open = true,
            Request::Tags => self.tags.open = !self.tags.open,
            Request::FullScreen => screen::toggle_full_screen(ctx),
            // Until measuring is a tab (Task 7), its tab is the measurements window.
            Request::ShowTab(Tab::Measure) => self.measure.open = !self.measure.open,
            Request::FoldRail => self.screen.list_shown = !self.screen.list_shown,
            Request::Size(size) => ctx.set_zoom_factor(size),
            Request::Larger => egui::gui_zoom::zoom_in(ctx),
            Request::Smaller => egui::gui_zoom::zoom_out(ctx),
            Request::SaveMeasurements(kind) => {
                let saved = measure::save(kind, self.session.state());
                if let Some(notice) = saved {
                    self.dispatch(Action::Post(notice));
                }
            }
            // Made once the frame has its graphics device in hand.
            Request::SaveScene(asked) => self.scene_asked = Some(asked),
            Request::OpenLabels => {
                let open = &self.session.state().labels;
                if let Some(opening) = self.labels.chosen(parts::Asked::OpenLabels, open) {
                    self.dispatch(opening);
                }
            }
            Request::Search => self.search.open(),
            // What the window has no place for yet: the other tabs and folding the panel come
            // with the panel (Task 6), the theme button (Task 10, applied in Task 17), and the
            // other files (Task 11).
            Request::ShowTab(_)
            | Request::FoldPanel
            | Request::Theme(_)
            | Request::ShowOtherFiles => {}
        }
    }
}
