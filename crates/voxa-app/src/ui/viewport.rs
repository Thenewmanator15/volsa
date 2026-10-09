//! The grid of viewports: what each one draws, what is written over it, and its mouse.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, wgpu};
use voxa_core::{SeriesId, SeriesInfo, StudyInfo};
use voxa_render::{SlicePipeline, ViewTransform};

use super::overlay;
use super::theme;
use crate::action::Action;
use crate::cache::FrameCache;
use crate::state::{AppState, Layout};

mod annotate;
mod mouse;

pub(super) use mouse::turned as wheel_turned;

/// Space between neighbouring viewports, in points.
pub const GAP: f32 = 4.0;
/// Thickness of the line round the active viewport, in points.
const ACTIVE_BORDER: f32 = 2.0;

/// What one viewport keeps between frames.
pub struct Gpu {
    pipeline: Arc<Mutex<SlicePipeline>>,
    /// The slice that is uploaded now, so it is uploaded once and not every frame.
    uploaded: Option<(SeriesId, usize)>,
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
/// each into actions.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    cache: &Mutex<FrameCache>,
    gpus: &mut [Gpu],
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
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
        let placement = viewport.series.as_ref().and_then(|id| state.series(id));
        let placement = placement.and_then(|series| series.info.voxel_to_patient.as_ref());
        let shown = overlay::display_view(placement, &viewport.view, state.radiological);
        let pane = Pane {
            state,
            at,
            cell,
            shown,
        };
        let mut gpu = gpus.get_mut(at);
        let drawn = match (gpu.as_deref_mut(), render) {
            (Some(gpu), Some(render)) => slice(ui, &pane, cache, gpu, render),
            _ => false,
        };
        if drawn {
            annotate::overlays(ui, &pane);
        }
        if let Some(text) = message(state, at, drawn) {
            let font = egui::TextStyle::Body.resolve(ui.style());
            ui.painter().text(
                cell.center(),
                Align2::CENTER_CENTER,
                text,
                font,
                theme::QUIET_TEXT,
            );
        }
        if at == state.active {
            let line = Stroke::new(ACTIVE_BORDER, theme::ACCENT);
            ui.painter().rect_stroke(cell, 0, line, StrokeKind::Inside);
        }
        let mut unused = 0.0;
        let part_notch = gpu.map_or(&mut unused, |gpu| &mut gpu.part_notch);
        let input = mouse::gather(ui, &response, cell, viewport.wl.width, part_notch);
        actions.extend(overlay::input_to_actions(at, &input));
    }
}

/// Upload the slice a viewport shows if it is not the one already uploaded, set window and
/// view, and queue the drawing. False when the slice is not in the cache or cannot be shown.
fn slice(
    ui: &Ui,
    pane: &Pane<'_>,
    cache: &Mutex<FrameCache>,
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
        let cached = cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series, viewport.slice);
        let Some(frame) = cached else {
            return false;
        };
        let (device, queue) = (&render.device, &render.queue);
        if pipeline
            .upload(device, queue, frame.rows, frame.cols, &frame.values)
            .is_err()
        {
            return false;
        }
        gpu.uploaded = Some(wanted);
    }
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    pipeline.set_params(&render.queue, viewport.wl, &pane.shown, size, aspect);
    let draw = DrawSlice(Arc::clone(&gpu.pipeline));
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(pane.cell, draw));
    true
}

#[cfg(test)]
mod tests;
