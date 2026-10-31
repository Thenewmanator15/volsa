//! What a person asks of the window rather than of the scan, from a button or a key: the
//! windows to open or close, the size of the interface, choosing what to open and where to
//! save.

use super::commands::{Request, Tab};
use super::{VoxaApp, about, document, help, parts, screen, tabs, top};
use crate::action::Action;

/// The tab of the panel that a request also shows, besides doing what it asks: labelling the
/// scan shows the Tools tab, where its progress and Stop are, however it was asked for.
fn also_shows(request: &Request) -> Option<Tab> {
    matches!(request, Request::LabelScan).then_some(Tab::Tools)
}

impl VoxaApp {
    /// Do what was asked of the window.
    pub(super) fn ask(&mut self, ctx: &egui::Context, request: Request) {
        if let Some(tab) = also_shows(&request) {
            self.ask(ctx, Request::ShowTab(tab));
        }
        match request {
            Request::Open(pick) => {
                if let Some(path) = top::picked(pick) {
                    let opening = self.labels.opening(path);
                    self.dispatch(opening);
                }
            }
            Request::Help(topic) => help::ask(ctx, &mut self.help_open, topic),
            Request::About => self.about_open = true,
            Request::Licences => about::open_notices(ctx),
            Request::CopyTheLog => {
                if let Some(log) = crate::log::path() {
                    ctx.copy_text(log.display().to_string());
                }
            }
            Request::ShowNotices => self.bars.notices_open = true,
            Request::Confirm(confirm) => self.confirming = Some(confirm),
            Request::Tags(of) => self.tags.open_on(of),
            Request::WhereTheLogIs => self.dispatch(Action::Post(about::where_the_log_is())),
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
            Request::Theme(choice) => self.bars.choose(ctx, choice),
            Request::SaveDocumentPdf(entry) => {
                // Written only where the person chooses, and nothing is written if they do not.
                if let Some(to) = document::choose_pdf() {
                    self.dispatch(Action::SaveDocumentPdf(entry, to));
                }
            }
            // How the window is arranged and drawn, which `arrange` answers.
            other => self.arrange(ctx, other),
        }
    }

    /// Do what asks how the window is arranged or drawn: the tabs, the columns, full screen, the
    /// size of the interface.
    fn arrange(&mut self, ctx: &egui::Context, request: Request) {
        match request {
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
            Request::ShowOtherFiles => {
                self.rail.other_open = true;
                if !self.screen.list_shown {
                    self.screen.toggle_rail();
                }
                ctx.request_repaint();
            }
            other => tracing::warn!("a request that nothing answers: {other:?}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::commands::{self, Does};

    #[test]
    fn labelling_the_scan_also_shows_the_tools_tab_however_it_was_asked_for() {
        // The Tools tab's button and the search both do what the action in the list does.
        let labelling = commands::find("tools.label").expect("listed");
        assert!(matches!(labelling.does, Does::Ask(Request::LabelScan)));
        assert_eq!(also_shows(&Request::LabelScan), Some(Tab::Tools));
        // And no other request shows a tab of its own accord.
        for command in commands::all() {
            if let Does::Ask(request) = &command.does {
                assert_eq!(
                    also_shows(request) == Some(Tab::Tools),
                    *request == Request::LabelScan,
                    "{}",
                    command.id
                );
            }
        }
    }
}
