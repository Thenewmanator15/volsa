//! The bar across the top of the window, and what the person asks it, or the window, to open.

use std::path::PathBuf;

use egui::Ui;
use voxa_core::WindowLevel;
use voxa_volume::PRESETS;

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

/// The layouts offered, with the label and hover text of each button.
const LAYOUTS: [(Layout, &str, &str); 3] = [
    (Layout::One, "1", "One viewport"),
    (Layout::TwoAcross, "2", "Two viewports side by side"),
    (Layout::TwoByTwo, "4", "Four viewports"),
];

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
        let list = ui.selectable_label(list_shown, "Series");
        asked.list = list
            .on_hover_text("Show or hide the list of series (B)")
            .clicked();
        ui.separator();
        asked.open = open(ui);
        ui.separator();
        ui.label("Layout");
        for (layout, label, hint) in LAYOUTS {
            let chosen = state.layout == layout;
            if ui
                .selectable_label(chosen, label)
                .on_hover_text(hint)
                .clicked()
                && !chosen
            {
                actions.push(Action::SetLayout(layout));
            }
        }
        ui.separator();
        window(ui, state, actions);
        ui.separator();
        switches(ui, state, actions);
        ui.separator();
        playback(ui, state, actions);
        super::volumes::stepper(ui, state, actions);
        ui.separator();
        let measure = ui.button("Measure");
        asked.measure = measure
            .on_hover_text("Rulers, angles, regions and markers, and the list of them (M)")
            .clicked();
        let tags = ui.button("Tags");
        asked.tags = tags
            .on_hover_text("Every element of the file behind the slice shown")
            .clicked();
        ui.separator();
        asked.size = super::size::menu(ui);
        asked.shortcuts = ui.button("?").on_hover_text("Keys and mouse").clicked();
        asked.about = ui.button("About").clicked();
    });
    asked
}

/// The buttons that open a scan: the kind of thing asked to be opened, if one was pressed.
fn open(ui: &mut Ui) -> Option<Pick> {
    ui.label("Open");
    let folder = ui
        .button("Folder…")
        .on_hover_text("Open a folder of DICOM files");
    let file = ui
        .button("File…")
        .on_hover_text("Open a zip file or a DICOMDIR");
    if folder.clicked() {
        Some(Pick::Folder)
    } else if file.clicked() {
        Some(Pick::File)
    } else {
        None
    }
}

/// What the person has just asked to open, if anything: a file or folder dropped on the
/// window, or one chosen in the system's dialog after pressing an Open button. While
/// something is being dragged over the window, say that it can be dropped.
pub fn chosen(ctx: &egui::Context, asked: Option<Pick>) -> Option<PathBuf> {
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
    dropped.or_else(|| match asked? {
        Pick::Folder => rfd::FileDialog::new()
            .set_title("Open a folder of DICOM files")
            .pick_folder(),
        Pick::File => rfd::FileDialog::new()
            .set_title("Open a zip file or a DICOMDIR")
            .pick_file(),
    })
}

/// The window and level of the active viewport, as numbers that can be typed or dragged,
/// and the presets.
fn window(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
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
    for (preset, named) in PRESETS.iter().enumerate() {
        let hint = format!(
            "From the {} to the {} percentile of the slice shown",
            named.low_pct, named.high_pct
        );
        if ui.button(named.name).on_hover_text(hint).clicked() {
            actions.push(Action::ApplyPreset { viewport, preset });
        }
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

/// The display convention, whether the patient's details are shown, whether the slice views
/// follow the planes' position, and whether the planes are drawn smooth.
fn switches(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let convention = "On: the patient's left is on the right of the screen, as a radiologist reads it.\n\
                      Off: neurological, the patient's left on the left.";
    if ui
        .selectable_label(state.radiological, "Radiological")
        .on_hover_text(convention)
        .clicked()
    {
        actions.push(Action::ToggleRadiological);
    }
    let patient = "Show the patient's name and identifier in the viewports and the tag browser";
    if ui
        .selectable_label(state.show_patient, "Patient details")
        .on_hover_text(patient)
        .clicked()
    {
        actions.push(Action::TogglePatient);
    }
    let linked = state.planes.linked;
    let link = ui.selectable_label(linked, "Link");
    if link.on_hover_text(LINK_SAYS).clicked() {
        actions.push(Action::SetLinked(!linked));
    }
    let smooth = state.planes.smooth;
    let curve = ui.selectable_label(smooth, "Smooth");
    if curve.on_hover_text(SMOOTH_SAYS).clicked() {
        actions.push(Action::SetSmooth(!smooth));
    }
}

/// Film-like playback through the slices of the active viewport.
fn playback(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let cine = state.viewports.get(viewport).and_then(|shown| shown.cine);
    let cine = cine.unwrap_or(Cine::new(Cine::DEFAULT_FPS, true, false));
    let label = if cine.playing { "Pause" } else { "Play" };
    if ui
        .button(label)
        .on_hover_text("Play through the slices (Space)")
        .clicked()
    {
        actions.push(Action::CineToggle { viewport });
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
