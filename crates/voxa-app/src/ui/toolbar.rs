//! The bar across the top of the window, and what the person asks it, or the window, to open.
//! Its buttons are actions of the list of actions, which gives their words and hints.

use std::path::PathBuf;

use egui::Ui;

use super::commands::{self, Request, Tab};
use crate::action::Action;
use crate::state::AppState;

/// The kinds of thing a scan can be opened from. The system's own dialogs choose either a
/// folder or a file, never both, so there is a button for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A folder of DICOM files.
    Folder,
    /// A zip file, a DICOMDIR or a single DICOM file.
    File,
}

/// What the person asked for that is not a change of state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one for each window or part of one a button of the bar opens"
)]
pub struct Asked {
    /// To choose a scan to open, and which kind.
    pub open: Option<Pick>,
    /// To see the About window.
    pub about: bool,
    /// To see the list of keys and what the mouse does.
    pub shortcuts: bool,
    /// To open or close the tag browser.
    pub tags: bool,
    /// To show the Measure tab.
    pub measure: bool,
    /// To show or hide the series list.
    pub list: bool,
    /// To draw the interface at this size.
    pub size: Option<f32>,
}

impl Asked {
    /// Note what a button of the bar asked of the window.
    fn take(&mut self, request: Option<Request>) {
        match request {
            Some(Request::Open(pick)) => self.open = Some(pick),
            Some(Request::About) => self.about = true,
            Some(Request::Help(_)) => self.shortcuts = true,
            Some(Request::Tags) => self.tags = true,
            Some(Request::ShowTab(Tab::Measure)) => self.measure = true,
            Some(Request::FoldRail) => self.list = true,
            Some(Request::Size(size)) => self.size = Some(size),
            // The bar has no button that asks for anything else.
            _ => {}
        }
    }

    /// What was asked of the window, in the order the bar has its buttons.
    pub fn requests(self) -> Vec<Request> {
        let flags = [
            (self.list, Request::FoldRail),
            (self.measure, Request::ShowTab(Tab::Measure)),
            (self.tags, Request::Tags),
            (self.shortcuts, Request::Help(None)),
            (self.about, Request::About),
        ];
        let opened = self.open.map(Request::Open);
        let sized = self.size.map(Request::Size);
        let flagged = flags
            .into_iter()
            .filter_map(|(asked, request)| asked.then_some(request));
        opened.into_iter().chain(flagged).chain(sized).collect()
    }
}

/// Draw the button of the action `id`, and take what it does: a change of state onto
/// `actions`, anything else into `asked`.
fn offer(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    selected: Option<bool>,
    asked: &mut Asked,
    actions: &mut Vec<Action>,
) {
    if let Some(does) = commands::button(ui, id, state, selected) {
        asked.take(does.run(state, actions));
    }
}

/// Draw the toolbar: the series list shown or hidden (`list_shown` says which it is), open,
/// the measurements, the tag browser, the size of the interface, the list of keys
/// and About. In a narrow window the bar wraps onto further lines. The brightness, the
/// switches and playback are in the View tab of the panel at the right, and the number of
/// viewports in the strip above the pictures.
pub fn show(ui: &mut Ui, state: &AppState, list_shown: bool, actions: &mut Vec<Action>) -> Asked {
    let mut asked = Asked::default();
    ui.horizontal_wrapped(|ui| {
        // In a narrow window what does not fit goes onto the next line, whole.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        super::theme::closer_in_a_short_window(ui);
        offer(
            ui,
            "rail.fold",
            state,
            Some(list_shown),
            &mut asked,
            actions,
        );
        ui.separator();
        ui.label("Open");
        offer(ui, "open.folder", state, None, &mut asked, actions);
        offer(ui, "open.file", state, None, &mut asked, actions);
        ui.separator();
        offer(ui, "measure.show", state, None, &mut asked, actions);
        offer(ui, "tags.show", state, None, &mut asked, actions);
        ui.separator();
        asked.size = super::size::menu(ui, state);
        offer(ui, "help.keys", state, None, &mut asked, actions);
        offer(ui, "about", state, None, &mut asked, actions);
    });
    asked
}

/// What has just been dropped on the window to open, if anything. While something is being
/// dragged over the window, say that it can be dropped.
pub fn dropped(ctx: &egui::Context) -> Option<PathBuf> {
    let (hovering, dropped) = ctx.input(|input| {
        let dropped = input
            .raw
            .dropped_files
            .first()
            .map(|file| file.path().to_owned());
        (!input.raw.hovered_files.is_empty(), dropped)
    });
    if hovering {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop to open"));
        let painter = ctx.layer_painter(layer);
        let screen = ctx.content_rect();
        painter.rect_filled(screen, 0, egui::Color32::from_black_alpha(160));
        let font = egui::FontId::proportional(24.0);
        let middle = egui::Align2::CENTER_CENTER;
        painter.text(
            screen.center(),
            middle,
            "Drop to open",
            font,
            egui::Color32::WHITE,
        );
    }
    dropped
}

/// What the person chose in the system's dialog for this kind of thing to open, if anything.
pub fn picked(pick: Pick) -> Option<PathBuf> {
    match pick {
        Pick::Folder => rfd::FileDialog::new()
            .set_title("Open a folder of DICOM files")
            .pick_folder(),
        Pick::File => rfd::FileDialog::new()
            .set_title("Open a zip file or a DICOMDIR")
            .pick_file(),
    }
}

#[cfg(test)]
mod tests;
