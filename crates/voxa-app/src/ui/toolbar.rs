//! The bar across the top of the window, and what the person asks it, or the window, to open.
//! Its buttons are actions of the list of actions, which gives their words and hints.

use std::path::PathBuf;

use egui::Ui;
use voxa_core::WindowLevel;

use super::commands::{self, Request, Tab};
use crate::action::Action;
use crate::state::{AppState, Cine, Layout};

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
    /// To open or close the measurements window.
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

/// The layouts offered, each by the action that chooses it.
const LAYOUTS: [(Layout, &str); 3] = [
    (Layout::One, "layout.one"),
    (Layout::TwoAcross, "layout.two"),
    (Layout::TwoByTwo, "layout.four"),
];

/// The brightness presets offered, by their actions, in the order of the presets.
const PRESET_IDS: [&str; 3] = ["window.auto", "window.soft", "window.hard"];

/// Draw the toolbar: the series list shown or hidden (`list_shown` says which it is), open,
/// the layouts, the window of the active viewport, the display switches, playback, the
/// measurements, the tag browser, the size of the interface, the list of keys and About. In a
/// narrow window the bar wraps onto further lines.
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
        ui.label("Layout");
        for (layout, id) in LAYOUTS {
            let chosen = state.layout == layout;
            let pressed = commands::button(ui, id, state, Some(chosen));
            // The layout in use is not asked for again.
            if let Some(does) = pressed.filter(|_| !chosen) {
                asked.take(does.run(state, actions));
            }
        }
        ui.separator();
        window(ui, state, &mut asked, actions);
        ui.separator();
        // The display convention, whether the patient's details are shown, whether the slice
        // views follow the planes' position, and whether the planes are drawn smooth.
        let switches = [
            ("view.radiological", state.radiological),
            ("view.patient", state.show_patient),
            ("planes.link", state.planes.linked),
            ("planes.smooth", state.planes.smooth),
        ];
        for (id, on) in switches {
            offer(ui, id, state, Some(on), &mut asked, actions);
        }
        ui.separator();
        playback(ui, state, actions);
        super::volumes::stepper(ui, state, actions);
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

/// The window and level of the active viewport, as numbers that can be typed or dragged,
/// and the presets.
fn window(ui: &mut Ui, state: &AppState, asked: &mut Asked, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let Some(shown) = state.viewports.get(viewport) else {
        return;
    };
    ui.label("Window");
    let series = shown.series.as_ref().and_then(|id| state.series(id));
    if series.is_some_and(|listed| listed.info.colour) {
        ui.weak("none: in its own colours");
        return;
    }
    let (mut width, mut level) = (shown.wl.width, shown.wl.center);
    let step = f64::from((shown.wl.width / 100.0).max(0.1));
    let width_field = egui::DragValue::new(&mut width)
        .speed(step)
        .prefix("W ")
        .range(1.0..=f32::MAX);
    let level_field = egui::DragValue::new(&mut level).speed(step).prefix("L ");
    let width_changed = ui
        .add(width_field)
        .on_hover_text("Width: the span of values from black to white")
        .changed();
    let level_changed = ui
        .add(level_field)
        .on_hover_text("Level: the value shown as mid-grey")
        .changed();
    if width_changed || level_changed {
        actions.push(Action::SetWindow {
            viewport,
            wl: WindowLevel::new(level, width),
        });
    }
    for id in PRESET_IDS {
        offer(ui, id, state, None, asked, actions);
    }
}

/// What the Link switch says when pointed at.
pub const LINK_SAYS: &str = "Keep the slices of this study on the planes' position, and the position on \
                             the slice scrolled to. Linked views take it that the patient did not \
                             move between series.";

/// What the Smooth switch says when pointed at.
pub const SMOOTH_SAYS: &str = "Draw the three planes from a curve through the scan's values, not a blend \
                               of the nearest. Beside a sharp edge the curve overshoots a little, \
                               which shows as a faint rim that is not in the scan. Smoothed: a curve \
                               through the measured values, not more detail.";

/// Film-like playback through the slices of the active viewport.
fn playback(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let cine = state.viewports.get(viewport).and_then(|shown| shown.cine);
    let cine = cine.unwrap_or(Cine::new(Cine::DEFAULT_FPS, true, false));
    let label = if cine.playing { "Pause" } else { "Play" };
    if let Some(does) = commands::button_labelled(ui, "slice.play", label, state, None) {
        let _ = does.run(state, actions);
    }
    let (mut fps, mut looping) = (cine.fps, cine.looping);
    let speed = egui::DragValue::new(&mut fps)
        .speed(0.5)
        .range(Cine::MIN_FPS..=Cine::MAX_FPS)
        .suffix(" /s");
    let speed_changed = ui
        .add(speed)
        .on_hover_text("Slices shown each second")
        .changed();
    let loop_hint = "Start again from the first slice after the last";
    let loop_changed = ui
        .checkbox(&mut looping, "Loop")
        .on_hover_text(loop_hint)
        .changed();
    if speed_changed || loop_changed {
        actions.push(Action::CineSet {
            viewport,
            fps,
            looping,
        });
    }
}

#[cfg(test)]
mod tests;
