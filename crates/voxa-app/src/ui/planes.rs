//! The axial, coronal and sagittal panels: the scene's volume in three planes through one
//! position, each lettered, captioned and crossed by the other two.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui};
use egui_wgpu::RenderState;
use voxa_core::WindowLevel;
use voxa_render::{PlaneView, VolumePipeline};
use voxa_volume::{PlaneKind, pitch_along};

use super::pen::{self, Pen};
use super::scene::status_text;
use super::{overlay as slice_overlay, theme};
use crate::action::Action;
use crate::planes::{grid, plane};
use crate::state::AppState;

mod gpu;
mod mouse;
mod overlay;

pub use mouse::{PlaneInput, plane_input_to_actions};
pub use overlay::{
    Room, crosshair, crossing, interpolated_note, plane_caption, plane_colour, plane_letters,
};

/// How thick the lines of the cross and the bar along the top of a panel are, in points.
const LINE: f32 = 1.0;
const BAR: f32 = 2.0;
/// The room the cross leaves at the edges of a panel: for the letters at the sides and the
/// top, and for the scale bar and a line of small type at the bottom.
const ROOM: Room = Room {
    sides: 26.0,
    top: 26.0,
    bottom: 34.0,
};
/// How much of a panel's width a note at its bottom left may take, leaving the corner beside
/// it to the scale bar.
const NOTE_SHARE: f32 = 0.62;

/// What the plane panels keep between frames.
pub struct PlanesView {
    /// One for each plane, in the order of [`PlaneKind::ALL`]; none when the window has no
    /// graphics device.
    gpus: Option<[gpu::PlaneGpu; 3]>,
    /// Part of a wheel notch scrolled on a touchpad over each panel and not yet acted on.
    part_notch: [f32; 3],
}

impl PlanesView {
    /// Panels with nothing drawn yet.
    pub fn new(render: Option<&RenderState>) -> Self {
        Self {
            gpus: render.map(|render| PlaneKind::ALL.map(|_| gpu::PlaneGpu::new(render))),
            part_notch: [0.0; 3],
        }
    }
}

/// Draw the three planes into `cells`, in the order of [`PlaneKind::ALL`], from the volume
/// the scene's pipeline holds, and turn what the mouse does over each into actions.
pub fn show(
    ui: &mut Ui,
    cells: [Rect; 3],
    state: &AppState,
    volume: Option<&VolumePipeline>,
    view: &mut PlanesView,
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    let gpus = view.gpus.as_ref().map(<[gpu::PlaneGpu; 3]>::each_ref);
    let [first, second, third] = gpus.map_or([None; 3], |gpus| gpus.map(Some));
    let [a, b, c] = &mut view.part_notch;
    let panels = [(first, a), (second, b), (third, c)];
    for ((kind, cell), (gpu, part_notch)) in PlaneKind::ALL.into_iter().zip(cells).zip(panels) {
        let on_device = gpu.zip(render).zip(volume);
        panel(ui, (kind, cell), state, on_device, part_notch, actions);
    }
}

/// What a panel is drawn with on the graphics device: its own pipeline, the window's device,
/// and the scene's volume.
type OnDevice<'a> = ((&'a gpu::PlaneGpu, &'a RenderState), &'a VolumePipeline);

/// Draw one plane's panel and gather what the mouse does over it.
fn panel(
    ui: &mut Ui,
    (kind, cell): (PlaneKind, Rect),
    state: &AppState,
    on_device: Option<OnDevice<'_>>,
    part_notch: &mut f32,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(cell, ui.id().with(("plane", kind)), Sense::click_and_drag());
    ui.painter().rect_filled(cell, 0, Color32::BLACK);
    let size = [cell.width(), cell.height()];
    let shown = plane(state, kind)
        .zip(state.scene.corners)
        .map(|(plane, corners)| {
            let panel = state.planes.panel(kind);
            PlaneView::showing(plane, &corners, size, panel.zoom, panel.offset_mm)
        });
    let active = state.viewports.get(state.active);
    let window = active.map_or(WindowLevel::new(0.5, 1.0), |shown| shown.wl);
    let drawn = shown.as_ref().zip(on_device).is_some_and(|(view, device)| {
        let ((gpu, render), volume) = device;
        gpu.draw(ui, cell, render, volume, view, window)
    });
    match &shown {
        Some(view) if drawn => written_over(ui, cell, state, kind, view),
        _ => {
            let why =
                status_text(&state.scene).unwrap_or_else(|| super::scene::CANNOT_DRAW.to_owned());
            let font = egui::TextStyle::Body.resolve(ui.style());
            let middle = Align2::CENTER_CENTER;
            ui.painter()
                .text(cell.center(), middle, why, font, theme::QUIET_TEXT);
        }
    }
    let top = Rect::from_min_max(cell.min, egui::pos2(cell.right(), cell.top() + BAR));
    ui.painter().rect_filled(top, 0, plane_colour(kind));
    if let Some(view) = &shown {
        let input = mouse::gather(ui, &response, cell, view, part_notch);
        let input = PlaneInput {
            window_width: window.width,
            ..input
        };
        actions.extend(plane_input_to_actions(kind, state.active, &input));
    }
}

/// Write over a plane's picture: the cross through the position, a letter at each edge, a
/// scale bar, the plane's name and place, and what the picture cannot show.
fn written_over(ui: &Ui, cell: Rect, state: &AppState, kind: PlaneKind, view: &PlaneView) {
    let pen = Pen::over(ui, cell);
    let size = [cell.width(), cell.height()];
    let on_screen = |[x, y]: [f32; 2]| egui::pos2(cell.left() + x, cell.top() + y);
    if let Some(position) = state.planes.position {
        let cross = crosshair(view, size, position, ROOM);
        let (level, upright) = crossing(kind);
        let lines = [(cross.level, level), (cross.upright, upright)];
        for (lines, other) in lines {
            let stroke = Stroke::new(LINE, plane_colour(other));
            for [from, to] in lines {
                pen.painter
                    .line_segment([on_screen(from), on_screen(to)], stroke);
            }
        }
    }
    let inside = cell.shrink(pen::INSET);
    pen::letters(&pen, inside, &plane_letters(&view.plane));
    let mm_per_point = narrowed(view.mm_per_px);
    let longest = cell.width() * (1.0 - NOTE_SHARE) - pen::INSET;
    if let Some((length, label)) = slice_overlay::scale_bar(mm_per_point, longest) {
        // In the corner: the middle of the bottom edge belongs to the cross and its letter.
        pen::bar_in_corner(&pen, inside, length, &label);
    }
    // In the top left corner, kept clear of the letter at the top.
    let room = inside.width() / 2.0 - pen::LETTER_ROOM;
    let position = state.planes.position.unwrap_or(view.plane.centre);
    let caption = plane_caption(kind, &view.plane, position);
    pen.write_in(inside.left_top(), &caption, plane_colour(kind), room);
    let note = grid(state).and_then(|(placement, _)| {
        let pitch = |side| pitch_along(&placement, side);
        let spacing = state.series(state.scene.series.as_ref()?)?.info.spacing_mm;
        interpolated_note(spacing, pitch(view.plane.across)?, pitch(view.plane.down)?)
    });
    if let Some(note) = note {
        pen.write_wrapped_up(inside.left_bottom(), &note, inside.width() * NOTE_SHARE);
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a scale in millimetres to the point"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests;
