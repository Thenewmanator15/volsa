//! The things in the scene a person can take hold of: each cut plane, and the slice shown in
//! 2D. Where they are and what a drag on them means is worked out in `volsa-render`; here they
//! are drawn and the drags are turned into actions.

use egui::{Pos2, Rect, Response, Sense, Shape, Stroke, Ui, WidgetInfo, WidgetType, vec2};
use volsa_render::{
    MAX_CUTS, SlicePlane, box_section, cut_handle, drag_to_mm, on_screen, position_cross,
    slice_drag_to_scroll, slice_outline,
};
use volsa_volume::{PlaneKind, pitch_along, rises_along};

use super::super::planes::plane_colour;
use super::super::theme;
use crate::action::Action;
use crate::planes::{grid, in_sight, plane};
use crate::state::AppState;

/// The side of the square at a plane's own point, in points.
const GRIP: f32 = 12.0;
/// The radius of the knob at the end of a cut's normal, in points.
const KNOB: f32 = 7.0;
const LINE: f32 = 1.5;
/// How far from the position each arm of its cross reaches, in points, and the least an arm
/// must be seen to reach for its end to be taken hold of: an arm that points at the viewer
/// cannot be dragged along.
const REACH: f32 = 28.0;
const LEAST_REACH: f32 = 10.0;

/// What is left of drags that have not yet come to a whole step, in millimetres: of the
/// slice's handle, and of the end of each arm of the position's cross.
#[derive(Debug, Default)]
pub struct Carried {
    slice: f64,
    planes: [f64; 3],
}

/// Draw the handles of the scene laid out in `area`, and turn drags on them into actions: the
/// square at a cut's point moves the cut, the knob at the end of its normal turns it, and
/// the square in the middle of the slice's outline moves on to another slice. With the plane
/// views in sight, each plane is outlined where it cuts the volume, and the ends of the cross
/// at the position move the planes. `carried` keeps what has been dragged short of a whole
/// step.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    carried: &mut Carried,
    actions: &mut Vec<Action>,
) {
    let scene = &state.scene;
    let viewport = [area.width(), area.height()];
    let painter = ui.painter_at(area);
    for (cut, plane) in scene.cuts.iter().enumerate().take(MAX_CUTS) {
        let handle = cut_handle(&scene.camera, viewport, plane);
        let (centre, knob) = (placed(area, handle.centre), placed(area, handle.knob));
        if !area.contains(centre) {
            continue;
        }
        painter.line_segment([centre, knob], Stroke::new(LINE, theme::ACCENT));
        let grip = Rect::from_center_size(centre, vec2(GRIP, GRIP));
        let moved = ui.interact(grip, ui.id().with(("cut", cut)), Sense::drag());
        named(&moved, &format!("Move cut {}", cut + 1));
        painter.rect_filled(grip, 2, theme::ACCENT);
        if let Some(by) = dragged(&moved.on_hover_text("Drag to move this cut")) {
            // The cut goes with the pointer along its normal; to remove more is to go
            // against it.
            let by_mm = -drag_to_mm(&scene.camera, plane.normal, by);
            actions.push(Action::CutMove { cut, by_mm });
        }
        let end = Rect::from_center_size(knob, vec2(2.0 * KNOB, 2.0 * KNOB));
        let turned = ui.interact(end, ui.id().with(("cut knob", cut)), Sense::drag());
        named(&turned, &format!("Turn cut {}", cut + 1));
        painter.circle_filled(knob, KNOB, theme::ACCENT);
        if let Some(by) = dragged(&turned.on_hover_text("Drag to turn this cut")) {
            actions.push(Action::CutTurn { cut, by });
        }
    }
    if scene.slice_shown {
        slice(ui, area, state, &mut carried.slice, actions);
    }
    if in_sight(state) {
        planes(ui, area, state, &mut carried.planes, actions);
    }
}

/// The plane views as they lie in the scene: each plane outlined in its colour where it cuts
/// the volume, and at the position they share a cross with an arm along each plane's normal,
/// in that plane's colour. The end of an arm is dragged to move its plane through its
/// layers. Nothing without a volume ready.
fn planes(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    carried: &mut [f64; 3],
    actions: &mut Vec<Action>,
) -> Option<()> {
    let camera = &state.scene.camera;
    let (corners, position) = (state.scene.corners?, state.planes.position?);
    let (placement, _) = grid(state)?;
    let viewport = [area.width(), area.height()];
    let painter = ui.painter_at(area);
    let seen = |at| placed(area, on_screen(camera, viewport, at));
    let shown = PlaneKind::ALL.map(|kind| plane(state, kind));
    for (kind, plane) in PlaneKind::ALL.into_iter().zip(shown) {
        let cut = plane.map(|plane| box_section(&corners, plane.centre, plane.normal()));
        let outline: Vec<Pos2> = cut.unwrap_or_default().into_iter().map(seen).collect();
        let line = Stroke::new(LINE, plane_colour(kind));
        painter.add(Shape::closed_line(outline, line));
    }
    let normals = shown.map(|plane| plane.map_or([0.0; 3], |plane| plane.normal()));
    let arm_mm = f64::from(REACH) * camera.mm_per_px;
    let arms = position_cross(camera, viewport, position, normals, arm_mm);
    let centre = seen(position);
    let each = PlaneKind::ALL
        .into_iter()
        .zip(arms)
        .zip(normals)
        .zip(carried);
    for (((kind, [back, front]), normal), carried) in each {
        let (back, front) = (placed(area, back), placed(area, front));
        painter.line_segment([back, front], Stroke::new(LINE, plane_colour(kind)));
        if (front - centre).length() < LEAST_REACH {
            continue;
        }
        let grip = Rect::from_center_size(front, vec2(GRIP, GRIP));
        let moved = ui.interact(grip, ui.id().with(("plane grip", kind)), Sense::drag());
        painter.circle_filled(front, GRIP / 2.0, plane_colour(kind));
        let name = kind.name().to_lowercase();
        named(&moved, &format!("Move the {name} plane through its layers"));
        let hover = format!("Drag to move the {name} plane through its layers");
        let dragged = dragged(&moved.on_hover_text(hover));
        let layers = dragged
            .zip(pitch_along(&placement, normal))
            .map(|(by, pitch)| slice_drag_to_scroll(camera, normal, pitch, by, carried));
        // Layers are counted the way the voxel numbers rise, whichever way the plane faces.
        let rising = rises_along(&placement, normal).unwrap_or(true);
        match layers {
            Some(along) if along != 0 => {
                let steps = if rising { along } else { -along };
                actions.push(Action::PlaneScroll { kind, steps });
            }
            _ => {}
        }
    }
    Some(())
}

/// The outline of the slice the active viewport shows, with a square in its middle that is
/// dragged to move through the slices. Nothing when that series has no place in the patient.
fn slice(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    carried: &mut f64,
    actions: &mut Vec<Action>,
) -> Option<()> {
    let camera = &state.scene.camera;
    let viewport = [area.width(), area.height()];
    let shown = state.viewports.get(state.active)?;
    let info = &state.series(shown.series.as_ref()?)?.info;
    let (placement, size) = (info.voxel_to_patient.as_ref()?, [info.cols, info.rows]);
    let outline = slice_outline(camera, viewport, placement, size, shown.slice);
    let [first, second, ..] = outline.map(|corner| placed(area, corner));
    let outline = outline.map(|corner| placed(area, corner)).to_vec();
    let painter = ui.painter_at(area);
    painter.add(Shape::closed_line(
        outline,
        Stroke::new(LINE, theme::WARNING),
    ));
    let plane = SlicePlane::of_slice(placement, size, shown.slice)?;
    // Taken hold of half way along its first edge, clear of a cut's handle in the middle of
    // the scene; or in its own middle when that edge is out of sight.
    let on_the_edge = first + (second - first) / 2.0;
    let in_the_middle = placed(area, on_screen(camera, viewport, plane.point));
    let centre = [on_the_edge, in_the_middle]
        .into_iter()
        .find(|at| area.contains(*at))?;
    let grip = Rect::from_center_size(centre, vec2(GRIP, GRIP));
    let moved = ui.interact(grip, ui.id().with("slice"), Sense::drag());
    named(&moved, "Move through the slices");
    painter.rect_filled(grip, 2, theme::WARNING);
    let by = dragged(&moved.on_hover_text("Drag to move through the slices"))?;
    let [_, _, between_slices] = info.spacing_mm;
    let delta = slice_drag_to_scroll(camera, plane.normal, between_slices, by, carried);
    if delta != 0 {
        actions.push(Action::Scroll {
            viewport: state.active,
            delta,
        });
    }
    Some(())
}

/// Tell a screen reader what a handle is: a grip to drag, with this name.
fn named(handle: &Response, name: &str) {
    handle.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, name));
}

/// A place in the scene's picture as a place in the window.
fn placed(area: Rect, [x, y]: [f32; 2]) -> Pos2 {
    area.min + vec2(x, y)
}

/// How far a handle was dragged this frame, if it was.
fn dragged(handle: &egui::Response) -> Option<[f32; 2]> {
    let by = handle.drag_delta();
    (handle.dragged() && by.length_sq() > 0.0).then_some([by.x, by.y])
}
