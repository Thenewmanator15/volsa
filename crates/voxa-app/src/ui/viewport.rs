//! The grid of viewports: what each one draws, what is written over it, what is measured on
//! it, and its mouse.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, wgpu};
use voxa_core::{SeriesId, SeriesInfo, StudyInfo};
use voxa_dicom::Frame;
use voxa_render::{SlicePipeline, ViewTransform};

use super::overlay;
use super::theme;
use crate::action::Action;
use crate::measuring::SliceValues;
use crate::state::{AppState, Layout};

mod annotate;
mod colour;
mod mouse;
mod tools;
mod value;

pub(super) use mouse::turned as wheel_turned;
pub use tools::Drafts;

/// Where a viewport gets a decoded slice from, by series and place: `None` until it is there.
pub type Frames<'a> = dyn Fn(&SeriesId, usize) -> Option<Arc<Frame>> + 'a;

/// Where a viewport reads a single value of a slice from, by series, place and the pixel as its
/// column and row: for the value under the pointer, which is asked for as the pointer moves.
/// `None` when the value is not there to be read, or the slice is a picture in colour.
pub type ValueAt<'a> = dyn Fn(&SeriesId, usize, (usize, usize)) -> Option<f32> + 'a;

/// Where a viewport reads the values of a slice from, where they lie, by series and place: the
/// values are handed to the reader given, if they are there to be read and the slice is not a
/// picture in colour. For what a region drawn on a slice holds, read once as it is finished.
pub type ReadSlice<'a> = dyn Fn(&SeriesId, usize, &mut dyn FnMut(SliceValues<'_>)) + 'a;

/// What a viewport reads slices from: the whole decoded slice, to draw it, a single value of
/// it, to say what is under the pointer, and its values in place, to measure on it.
pub struct Slices<'a> {
    /// A whole decoded slice.
    pub frame: &'a Frames<'a>,
    /// One value of a slice.
    pub value: &'a ValueAt<'a>,
    /// A slice's values, where they lie.
    pub read: &'a ReadSlice<'a>,
}

/// Space between neighbouring viewports, in points.
pub const GAP: f32 = 4.0;
/// Thickness of the line round the active viewport, in points.
const ACTIVE_BORDER: f32 = 2.0;

/// What one viewport keeps between frames.
pub struct Gpu {
    pipeline: Arc<Mutex<SlicePipeline>>,
    /// The slice that is uploaded now, so it is uploaded once and not every frame.
    uploaded: Option<(SeriesId, usize)>,
    /// That slice as a picture in its own colours, when it is stored in colour.
    coloured: Option<egui::TextureHandle>,
    /// Part of a wheel notch scrolled on a touchpad and not yet acted on.
    part_notch: f32,
}

impl Gpu {
    /// A pipeline drawing into the window's own format, with nothing uploaded yet.
    pub fn new(render: &RenderState) -> Self {
        let pipeline = SlicePipeline::new(&render.device, render.target_format);
        Self {
            pipeline: Arc::new(Mutex::new(pipeline)),
            uploaded: None,
            coloured: None,
            part_notch: 0.0,
        }
    }

    /// Whether a slice has been uploaded, so there is something to see.
    pub fn has_picture(&self) -> bool {
        self.uploaded.is_some()
    }
}

/// Draws a viewport's slice inside egui's own render pass.
struct DrawSlice(Arc<Mutex<SlicePipeline>>);

impl CallbackTrait for DrawSlice {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &CallbackResources,
    ) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .draw(pass);
    }
}

/// One viewport being drawn this frame.
struct Pane<'a> {
    state: &'a AppState,
    at: usize,
    cell: Rect,
    /// The view the slice is drawn with: the person's own, mirrored if the convention asks.
    shown: ViewTransform,
}

impl Pane<'_> {
    /// The series this viewport shows and the study it belongs to.
    fn series(&self) -> Option<(&SeriesInfo, &StudyInfo)> {
        let id = self.state.viewports.get(self.at)?.series.as_ref()?;
        let index = self.state.index.as_ref()?;
        let study = index
            .studies
            .iter()
            .find(|study| study.series.iter().any(|s| s.id == *id))?;
        Some((study.series.iter().find(|series| series.id == *id)?, study))
    }
}

/// The areas the viewports of a layout take: one, two side by side, or two rows of two.
pub fn grid(area: Rect, layout: Layout) -> Vec<Rect> {
    let (across, down) = match layout {
        Layout::One => (1_u8, 1_u8),
        Layout::TwoAcross => (2, 1),
        Layout::TwoByTwo => (2, 2),
    };
    let cell = vec2(
        (area.width() - GAP * f32::from(across - 1)) / f32::from(across),
        (area.height() - GAP * f32::from(down - 1)) / f32::from(down),
    );
    let place = |row: u8, col: u8| {
        let (row, col) = (f32::from(row), f32::from(col));
        let min = area.min + vec2(col * (cell.x + GAP), row * (cell.y + GAP));
        Rect::from_min_size(min, cell)
    };
    (0..down)
        .flat_map(|row| (0..across).map(move |col| place(row, col)))
        .collect()
}

/// What to write over a viewport that has no picture to show: why not. `None` when the slice
/// is there to be drawn, or there is no such viewport.
pub fn message(state: &AppState, viewport: usize, has_frame: bool) -> Option<String> {
    let viewport = state.viewports.get(viewport)?;
    let Some(series) = &viewport.series else {
        return Some("No series is shown here".to_owned());
    };
    if let Some(reason) = state.failed.get(&(series.clone(), viewport.slice)) {
        return Some(format!("This slice could not be decoded\n{reason}"));
    }
    (!has_frame).then(|| "Loading…".to_owned())
}

/// How many times taller than wide one pixel of the series shown is: the distance between
/// rows over the distance between columns. 1 when nothing is shown or the files do not say.
pub fn pixel_aspect(state: &AppState, viewport: usize) -> f32 {
    let shown = state
        .viewports
        .get(viewport)
        .and_then(|viewport| viewport.series.as_ref());
    let spacing = shown
        .and_then(|id| state.series(id))
        .map(|series| series.info.spacing_mm);
    match spacing {
        Some([between_cols, between_rows, _]) if between_cols > 0.0 && between_rows > 0.0 => {
            narrowed(between_rows / between_cols)
        }
        _ => 1.0,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "ratios and screen lengths are small numbers"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// Draw the viewports of the current layout into `area`, and turn what the mouse does over
/// each into actions: what it asks of the viewport, and what it measures on the slice the
/// active one shows. `held` is what each viewport keeps between frames: what is on the
/// graphics device, and what is being drawn on its slice. With no `area`, the arrangement
/// shown has no viewports: nothing is drawn, and what was being drawn on a slice is given up,
/// not kept for when the slices are shown again.
pub fn show(
    ui: &mut Ui,
    area: Option<Rect>,
    state: &AppState,
    slices: &Slices<'_>,
    (gpus, drafts): (&mut [Gpu], &mut Drafts),
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    let Some(area) = area else {
        drafts.give_up();
        return;
    };
    let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    drafts.keep_up(state, cancel);
    for (at, cell) in grid(area, state.layout).into_iter().enumerate() {
        let Some(viewport) = state.viewports.get(at) else {
            continue;
        };
        let response = ui.interact(
            cell,
            ui.id().with(("viewport", at)),
            Sense::click_and_drag(),
        );
        ui.painter().rect_filled(cell, 0, Color32::BLACK);
        let pane = pane(state, at, cell);
        let mut gpu = gpus.get_mut(at);
        let drawn = match (gpu.as_deref_mut(), render) {
            (Some(gpu), Some(render)) => slice(ui, &pane, slices.frame, gpu, render),
            _ => false,
        };
        if drawn {
            let value = value::under_pointer(&pane, slices.value, response.hover_pos());
            let taken = annotate::overlays(ui, &pane, value);
            let held = (&mut *drafts, slices.read);
            actions.extend(tools::measure(ui, &pane, &response, held, &taken));
        }
        written_over(ui, &pane, drawn);
        let mut unused = 0.0;
        let part_notch = gpu.map_or(&mut unused, |gpu| &mut gpu.part_notch);
        let input = mouse::gather(ui, &response, cell, viewport.wl.width, part_notch);
        let input = mouse::for_tool(input, state.tool);
        actions.extend(overlay::input_to_actions(at, &input));
    }
}

/// Viewport `at` of `state` being drawn in `cell` this frame, with the view its slice is drawn
/// with: the person's own, mirrored if the convention asks.
fn pane(state: &AppState, at: usize, cell: Rect) -> Pane<'_> {
    let viewport = state.viewports.get(at);
    let shown = viewport.and_then(|viewport| state.series(viewport.series.as_ref()?));
    let placement = shown.and_then(|series| series.info.voxel_to_patient.as_ref());
    let view = viewport.map_or_else(ViewTransform::fit, |viewport| viewport.view);
    Pane {
        state,
        at,
        cell,
        shown: overlay::display_view(placement, &view, state.radiological),
    }
}

/// Write over a viewport why it has no picture, if it has none, and mark it if it is the
/// active one.
fn written_over(ui: &Ui, pane: &Pane<'_>, drawn: bool) {
    if let Some(text) = message(pane.state, pane.at, drawn) {
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter().text(
            pane.cell.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            theme::QUIET_TEXT,
        );
    }
    if pane.at == pane.state.active {
        let line = Stroke::new(ACTIVE_BORDER, theme::ACCENT);
        ui.painter()
            .rect_stroke(pane.cell, 0, line, StrokeKind::Inside);
    }
}

/// Upload the slice a viewport shows if it is not the one already uploaded, set window and
/// view, and queue the drawing. False when the slice is not there yet or cannot be shown.
fn slice(
    ui: &Ui,
    pane: &Pane<'_>,
    frames: &Frames<'_>,
    gpu: &mut Gpu,
    render: &RenderState,
) -> bool {
    let Some(viewport) = pane.state.viewports.get(pane.at) else {
        return false;
    };
    let Some(series) = &viewport.series else {
        gpu.uploaded = None;
        return false;
    };
    let wanted = (series.clone(), viewport.slice);
    let mut pipeline = gpu.pipeline.lock().unwrap_or_else(PoisonError::into_inner);
    if gpu.uploaded.as_ref() != Some(&wanted) {
        let cached = frames(series, viewport.slice);
        let Some(frame) = cached else {
            return false;
        };
        let (device, queue) = (&render.device, &render.queue);
        gpu.coloured = colour::texture(ui.ctx(), &frame);
        let grey = gpu.coloured.is_none();
        if grey
            && pipeline
                .upload(device, queue, frame.rows, frame.cols, &frame.values)
                .is_err()
        {
            return false;
        }
        gpu.uploaded = Some(wanted);
    }
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    if let Some(picture) = &gpu.coloured {
        colour::draw(ui, pane.cell, &pane.shown, picture, aspect);
        return true;
    }
    pipeline.set_params(&render.queue, viewport.wl, &pane.shown, size, aspect);
    let draw = DrawSlice(Arc::clone(&gpu.pipeline));
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(pane.cell, draw));
    true
}

#[cfg(test)]
mod in_hand_tests;
#[cfg(test)]
mod labels_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tools_tests;
