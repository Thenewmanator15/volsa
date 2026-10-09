//! The grid of viewports and what each one draws.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, wgpu};
use voxa_core::SeriesId;
use voxa_render::SlicePipeline;

use super::theme;
use crate::action::Action;
use crate::cache::FrameCache;
use crate::state::{AppState, Layout};

/// Space between neighbouring viewports, in points.
pub const GAP: f32 = 4.0;
/// Thickness of the line round the active viewport, in points.
const ACTIVE_BORDER: f32 = 2.0;

/// What one viewport keeps on the graphics device.
pub struct Gpu {
    pipeline: Arc<Mutex<SlicePipeline>>,
    /// The slice that is uploaded now, so it is uploaded once and not every frame.
    uploaded: Option<(SeriesId, usize)>,
}

impl Gpu {
    /// A pipeline drawing into the window's own format, with nothing uploaded yet.
    pub fn new(render: &RenderState) -> Self {
        let pipeline = SlicePipeline::new(&render.device, render.target_format);
        Self {
            pipeline: Arc::new(Mutex::new(pipeline)),
            uploaded: None,
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
    reason = "a ratio of two pixel spacings is a small number"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// Draw the viewports of the current layout into what is left of `ui`. Clicking one makes it
/// the active viewport.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    cache: &Mutex<FrameCache>,
    gpus: &mut [Gpu],
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    let area = ui.available_rect_before_wrap();
    for (at, cell) in grid(area, state.layout).into_iter().enumerate() {
        let response = ui.interact(cell, ui.id().with(("viewport", at)), Sense::click());
        if response.clicked() {
            actions.push(Action::SetActive(at));
        }
        ui.painter().rect_filled(cell, 0, Color32::BLACK);
        let drawn = match (gpus.get_mut(at), render) {
            (Some(gpu), Some(render)) => slice(ui, state, cache, gpu, render, at, cell),
            _ => false,
        };
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
    }
    ui.allocate_rect(area, Sense::hover());
}

/// Upload the slice a viewport shows if it is not the one already uploaded, set window and
/// view, and queue the drawing. False when the slice is not in the cache or cannot be shown.
fn slice(
    ui: &Ui,
    state: &AppState,
    cache: &Mutex<FrameCache>,
    gpu: &mut Gpu,
    render: &RenderState,
    at: usize,
    cell: Rect,
) -> bool {
    let Some(viewport) = state.viewports.get(at) else {
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
    let size = [cell.width(), cell.height()];
    pipeline.set_params(
        &render.queue,
        viewport.wl,
        &viewport.view,
        size,
        pixel_aspect(state, at),
    );
    let draw = DrawSlice(Arc::clone(&gpu.pipeline));
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(cell, draw));
    true
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};

    use super::*;
    use crate::action::Action;
    use crate::reducer::reduce;
    use crate::state::{AppState, Layout};
    use crate::testing::{THICK_SCAN, id, three_series};

    fn area() -> Rect {
        Rect::from_min_max(pos2(240.0, 40.0), pos2(1040.0, 640.0))
    }

    #[test]
    fn one_viewport_takes_the_whole_area() {
        assert_eq!(grid(area(), Layout::One), vec![area()]);
    }

    #[test]
    fn two_viewports_sit_side_by_side_with_a_gap_between() {
        let cells = grid(area(), Layout::TwoAcross);
        assert_eq!(cells.len(), 2);
        assert_eq!((cells[0].min, cells[1].max), (area().min, area().max));
        assert_eq!(cells[0].width(), cells[1].width());
        assert_eq!(cells[1].min.x - cells[0].max.x, GAP);
        assert_eq!(cells[0].height(), area().height());
    }

    #[test]
    fn four_viewports_make_two_rows_of_two() {
        let cells = grid(area(), Layout::TwoByTwo);
        assert_eq!(cells.len(), 4);
        assert_eq!((cells[0].min, cells[3].max), (area().min, area().max));
        assert_eq!(cells[0].size(), cells[3].size());
        assert_eq!(cells[2].min.y - cells[0].max.y, GAP);
        assert_eq!(
            (cells[1].min.y, cells[2].min.x),
            (cells[0].min.y, cells[0].min.x)
        );
    }

    fn showing() -> AppState {
        let mut state = AppState::default();
        reduce(&mut state, Action::IndexProgress(three_series()));
        reduce(
            &mut state,
            Action::SelectSeries {
                viewport: 0,
                series: id(THICK_SCAN),
            },
        );
        state
    }

    #[test]
    fn a_viewport_says_what_it_is_waiting_for() {
        let state = showing();
        assert_eq!(message(&state, 0, false).as_deref(), Some("Loading…"));
        assert_eq!(
            message(&state, 0, true),
            None,
            "the slice is there to be drawn"
        );
        assert_eq!(
            message(&state, 1, false).as_deref(),
            Some("No series is shown here")
        );
        assert_eq!(message(&state, 9, false), None, "there is no such viewport");
    }

    #[test]
    fn a_slice_that_failed_says_so_with_the_reason() {
        let mut state = showing();
        let failed = Action::FrameFailed {
            series: id(THICK_SCAN),
            slice: 15,
            reason: "The image could not be decoded: the file is damaged or cut short.".to_owned(),
        };
        reduce(&mut state, failed);
        assert_eq!(
            message(&state, 0, false).as_deref(),
            Some(
                "This slice could not be decoded\n\
                 The image could not be decoded: the file is damaged or cut short."
            )
        );
    }

    #[test]
    fn the_pixels_of_a_series_are_as_tall_as_its_rows_are_apart() {
        let mut state = showing();
        assert_eq!(pixel_aspect(&state, 0), 1.0);
        let index = state.index.as_mut().expect("indexed");
        index.series[0].info.spacing_mm = [0.5, 1.5, 5.0];
        assert_eq!(pixel_aspect(&state, 0), 3.0);
        index_spacing(&mut state, [0.0, 1.0, 1.0]);
        assert_eq!(
            pixel_aspect(&state, 0),
            1.0,
            "a spacing of nothing is not believed"
        );
        assert_eq!(pixel_aspect(&state, 3), 1.0, "nothing shown");
    }

    fn index_spacing(state: &mut AppState, spacing: [f64; 3]) {
        let index = state.index.as_mut().expect("indexed");
        index.series[0].info.spacing_mm = spacing;
    }
}
