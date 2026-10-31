//! What a person asks of the window rather than of the scan, from a button or a key: the
//! windows to open or close, the size of the interface, choosing what to open and where to
//! save.

use super::commands::Request;
use super::{VoxaApp, parts, screen, tabs, toolbar};
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
            Request::ShowTab(tab) => {
                self.screen.show_tab(tab);
                ctx.request_repaint();
            }
            Request::FoldPanel => {
                self.screen.toggle_panel();
                ctx.request_repaint();
            }
            Request::FoldRail => {
                self.screen.toggle_rail();
                ctx.request_repaint();
            }
            Request::Size(size) => ctx.set_zoom_factor(size),
            Request::Larger => egui::gui_zoom::zoom_in(ctx),
            Request::Smaller => egui::gui_zoom::zoom_out(ctx),
            Request::SaveMeasurements(kind) => {
                let saved = tabs::save_measurements(kind, self.session.state());
                if let Some(notice) = saved {
                    self.dispatch(Action::Post(notice));
                }
            }
            // Made once the frame has its graphics device in hand.
            Request::SaveScene(asked) => self.scene_asked = Some(asked),
            Request::OpenLabels => self.choose(parts::Asked::OpenLabels),
            Request::Choose(asked) => self.choose(asked),
            // Made once the frame has its graphics device in hand.
            Request::LabelScan => self.label_asked = true,
            Request::Search => self.search.open(),
            // What the window has no place for yet: the theme button (Task 10, applied in Task
            // 17), and the other files (Task 11).
            Request::Theme(_) | Request::ShowOtherFiles => {}
        }
    }
}

impl VoxaApp {
    /// Ask the person for the file `asked` wants chosen, and do what comes of their choice.
    fn choose(&mut self, asked: parts::Asked) {
        let open = &self.session.state().labels;
        if let Some(action) = self.labels.chosen(asked, open) {
            self.dispatch(action);
        }
    }
}
