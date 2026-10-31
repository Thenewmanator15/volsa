//! The axial, coronal and sagittal panels: the scene's volume in three planes through one
//! position, each lettered, captioned and crossed by the other two.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui};
use std::sync::Arc;

use egui_wgpu::RenderState;
use voxa_core::{Volume, WindowLevel};
use voxa_labels::LabelSet;
use voxa_render::{LabelsOnDevice, Overlay, PlaneView, SmoothedTexture, VolumePipeline};
use voxa_volume::{Plane, PlaneKind, ellipse_stats, pitch_along};

use super::pen::{self, Pen};
use super::scene::status_text;
use super::{overlay as slice_overlay, theme};
use crate::action::Action;
use crate::annotations::{Shape, Tool, measured};
use crate::measuring::Scale;
use crate::planes::{grid, plane};
use crate::state::{AppState, Notice, NoticeKind};

mod curve;
mod gpu;
mod mouse;
mod overlay;
mod parts;
mod shapes;
mod tools;

pub use curve::Smoothing;
pub use mouse::{PlaneInput, plane_input_to_actions};
pub use overlay::{
    Room, crosshair, crossing, interpolated_note, panel_note, plane_caption, plane_colour,
    plane_letters,
};
pub use shapes::{Drawn, beside, drawn_through, paint, shown_on};
pub use tools::{Draft, Pointer, drafted, preview};

/// How thick the lines of the cross and the bar along the top of a panel are, in points.
const LINE: f32 = 1.0;
const BAR: f32 = 2.0;
/// The room the cross leaves at the edges of a panel: for the letters at the sides and the
/// top, and for the scale bar and two lines of small type at the bottom.
const ROOM: Room = Room {
    sides: 26.0,
    top: 26.0,
    bottom: 48.0,
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
    /// The annotation being drawn, if one has been begun.
    drawing: tools::Drawing,
    /// The curve the planes are drawn from when they are drawn smooth, on the graphics
    /// device, with the volume it runs through.
    curve: Option<(Arc<Volume>, SmoothedTexture)>,
    /// The volume the graphics device had no room to work the curve out for: a worker
    /// was asked instead, and the device is not asked again for the same volume.
    refused: Option<Arc<Volume>>,
}

impl PlanesView {
    /// Panels with nothing drawn yet.
    pub fn new(render: Option<&RenderState>) -> Self {
        Self {
            gpus: render.map(|render| PlaneKind::ALL.map(|_| gpu::PlaneGpu::new(render))),
            part_notch: [0.0; 3],
            drawing: tools::Drawing::default(),
            curve: None,
            refused: None,
        }
    }

    /// Draw these labels over every panel in this way, or none with `None`.
    pub fn label(&mut self, with: Option<&LabelsOnDevice>, overlay: Overlay) {
        for gpu in self.gpus.iter().flatten() {
            gpu.labels(with, overlay);
        }
    }
}

/// What is said when the graphics device has no room for the curve the planes would be drawn
/// smooth from.
pub fn no_room_to_smooth() -> Notice {
    let text = "The graphics device has no room to smooth the planes: they are drawn as before.";
    Notice {
        kind: NoticeKind::Warning,
        text: text.to_owned(),
        details: Vec::new(),
        key: Some("smoothing"),
    }
}

/// What the panels are drawn from: the scene's volume on the graphics device, with the
/// window's device; the same volume on the processor, for measuring; and the labels read,
/// for telling which part lies under the pointer.
pub type Sources<'a> = (
    Option<(&'a VolumePipeline, &'a RenderState)>,
    Option<&'a Volume>,
    Option<&'a LabelSet>,
);

/// Draw the three planes into `cells`, in the order of [`PlaneKind::ALL`], and turn what the
/// mouse does over each into actions: moving the position and the panel, or drawing with the
/// tool in hand.
pub fn show(
    ui: &mut Ui,
    cells: [Rect; 3],
    state: &AppState,
    (on_device, scan, labels): Sources<'_>,
    view: &mut PlanesView,
    actions: &mut Vec<Action>,
) {
    let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let series = state.scene.series.as_ref();
    view.drawing.keep_up(state.tool, series, cancel);
    let gpus = view.gpus.as_ref().map(<[gpu::PlaneGpu; 3]>::each_ref);
    let [first, second, third] = gpus.map_or([None; 3], |gpus| gpus.map(Some));
    let [a, b, c] = &mut view.part_notch;
    let panels = [(first, a), (second, b), (third, c)];
    for ((kind, cell), (gpu, part_notch)) in PlaneKind::ALL.into_iter().zip(cells).zip(panels) {
        let on_device = gpu.zip(on_device);
        let shown = panel(ui, (kind, cell), state, on_device, &view.drawing);
        if let Some((response, shown)) = shown {
            let input = mouse::gather(ui, &response, cell, &shown, part_notch);
            let drawn = handled(state, kind, &input, (&shown.plane, scan), &mut view.drawing);
            actions.extend(drawn);
            write_value(ui, cell, overlay::value_under(state, scan, input.hover_at));
            // The press that moves the position also selects the part pressed on.
            actions.extend(parts::selected_by(state, labels, &input));
            if let Some(name) = parts::named_under(state, labels, &input) {
                response.on_hover_text_at_pointer(name);
            }
        }
    }
}

/// Write the value under the pointer in the top right corner of a panel, which nothing else
/// of the panel's writing takes.
fn write_value(ui: &Ui, cell: Rect, line: Option<String>) {
    let Some(line) = line else {
        return;
    };
    let pen = Pen::over(ui, cell);
    let inside = cell.shrink(pen::INSET);
    let room = inside.width() / 2.0 - pen::LETTER_ROOM;
    pen.write_lines(inside.right_top(), Align2::RIGHT_TOP, &[line], room);
}

/// What a panel is drawn with on the graphics device: its own pipeline, and the scene's
/// volume with the window's device.
type OnDevice<'a> = (&'a gpu::PlaneGpu, (&'a VolumePipeline, &'a RenderState));

/// Draw one plane's panel. Gives what the mouse does over it and what it shows, when it has
/// a plane to show.
fn panel(
    ui: &mut Ui,
    (kind, cell): (PlaneKind, Rect),
    state: &AppState,
    on_device: Option<OnDevice<'_>>,
    drawing: &tools::Drawing,
) -> Option<(egui::Response, PlaneView)> {
    let response = ui.interact(cell, ui.id().with(("plane", kind)), Sense::click_and_drag());
    ui.painter().rect_filled(cell, 0, Color32::BLACK);
    let size = [cell.width(), cell.height()];
    let shown = plane(state, kind)
        .zip(state.scene.corners)
        .map(|(plane, corners)| {
            let panel = state.planes.panel(kind);
            PlaneView::showing(plane, &corners, size, panel.zoom, panel.offset_mm)
        });
    let drawn = shown.as_ref().zip(on_device).is_some_and(|(view, device)| {
        let (gpu, (volume, render)) = device;
        gpu.draw(ui, cell, render, volume, view, window(state))
    });
    let smooth = on_device.is_some_and(|(gpu, _)| gpu.is_smooth());
    match &shown {
        Some(view) if drawn => written_over(ui, cell, state, (kind, view, smooth), drawing),
        _ => {
            let none = || super::scene::CANNOT_DRAW.to_owned();
            let why = status_text(&state.scene).unwrap_or_else(none);
            let font = egui::TextStyle::Body.resolve(ui.style());
            let middle = Align2::CENTER_CENTER;
            ui.painter()
                .text(cell.center(), middle, why, font, theme::QUIET_TEXT);
        }
    }
    let top = Rect::from_min_max(cell.min, egui::pos2(cell.right(), cell.top() + BAR));
    ui.painter().rect_filled(top, 0, plane_colour(kind));
    shown.map(|shown| (response, shown))
}

/// The window the planes are shown through: the active viewport's.
fn window(state: &AppState) -> WindowLevel {
    let active = state.viewports.get(state.active);
    active.map_or(WindowLevel::new(0.5, 1.0), |shown| shown.wl)
}

/// Turn what the mouse did over a panel into actions. With the navigating tool the main
/// button moves the position; with any other it draws, and a finished annotation is added,
/// a region with what the scan holds inside it.
fn handled(
    state: &AppState,
    kind: PlaneKind,
    input: &PlaneInput,
    (plane, scan): (&Plane, Option<&Volume>),
    drawing: &mut tools::Drawing,
) -> Vec<Action> {
    let navigating = state.tool == Tool::Navigate;
    let made = if navigating {
        None
    } else {
        drawing.step(kind, input, plane)
    };
    let input = PlaneInput {
        window_width: window(state).width,
        held_at: input.held_at.filter(|_| navigating),
        ..input.clone()
    };
    let mut actions = plane_input_to_actions(kind, state.active, &input);
    actions.extend(made.map(|shape| Action::AnnotationAdd(measured_on(shape, plane, scan))));
    actions
}

/// A region with what the scan holds inside it worked out, if there is a scan to read; any
/// other annotation as it is.
fn measured_on(shape: Shape, plane: &Plane, scan: Option<&Volume>) -> Shape {
    match shape {
        Shape::Region {
            centre,
            across,
            down,
            radii_mm,
            stats: None,
        } => {
            let [x, y, z] = centre;
            let (on_plane, _) = plane.of(voxa_core::Point::new(x, y, z));
            let stats = scan.and_then(|scan| ellipse_stats(scan, plane, on_plane, radii_mm));
            Shape::Region {
                centre,
                across,
                down,
                radii_mm,
                stats,
            }
        }
        other => other,
    }
}

/// Write over a plane's picture: the cross through the position, a letter at each edge, a
/// scale bar, the plane's name and place, and what the picture cannot show. `smooth` is
/// whether the picture was drawn from the curve through the values.
fn written_over(
    ui: &Ui,
    cell: Rect,
    state: &AppState,
    (kind, view, smooth): (PlaneKind, &PlaneView, bool),
    drawing: &tools::Drawing,
) {
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
    let of_labels = parts::drawn(state).then(|| parts::labels_note(state).to_owned());
    let notes: Vec<String> = panel_note(note, smooth)
        .into_iter()
        .chain(of_labels)
        .collect();
    if let Some(note) = Some(notes.join("\n")).filter(|note| !note.is_empty()) {
        // Kept to its own side of the letter in the middle of the bottom edge.
        pen.write_wrapped_up(inside.left_bottom(), &note, room);
    }
    annotations(&pen, cell, state, view, drawing.so_far(kind, &view.plane));
}

/// Draw the annotations that lie in the layer a panel shows, each with its name and what it
/// measures, and then the one being drawn, with what it would measure if finished now.
fn annotations(pen: &Pen, cell: Rect, state: &AppState, view: &PlaneView, so_far: Option<Shape>) {
    let size = [cell.width(), cell.height()];
    let placement = grid(state).map(|(placement, _)| placement);
    let pitch = placement.and_then(|placement| pitch_along(&placement, view.plane.normal()));
    // Half a layer either side, and a hair more for what lies exactly between two.
    let half_layer = pitch.map_or(0.5, |pitch| pitch / 2.0) + 1.0e-6;
    let value = |shape: &Shape, scale| measured(shape, placement.as_ref(), scale).shown();
    for item in &state.annotations.items {
        if shapes::shown_on(&item.shape, &view.plane, half_layer) {
            let words = shapes::beside(item, &value(&item.shape, item.scale));
            let drawn = shapes::drawn(&item.shape, view, size);
            shapes::paint(pen, (cell, ROOM.top), &drawn, theme::ACCENT, &words);
        }
    }
    if let Some(shape) = so_far {
        let drawn = shapes::drawn(&shape, view, size);
        shapes::paint(
            pen,
            (cell, ROOM.top),
            &drawn,
            theme::WARNING,
            // The planes are of a volume, which is placed in the patient.
            &value(&shape, Scale::Patient),
        );
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
#[cfg(test)]
mod tools_tests;
