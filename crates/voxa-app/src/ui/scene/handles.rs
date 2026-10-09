//! The things in the scene a person can take hold of: each cut plane, and the slice shown in
//! 2D. Where they are and what a drag on them means is worked out in `voxa-render`; here they
//! are drawn and the drags are turned into actions.

use egui::{Pos2, Rect, Sense, Shape, Stroke, Ui, vec2};
use voxa_render::{
    MAX_CUTS, SlicePlane, cut_handle, drag_to_mm, on_screen, slice_drag_to_scroll, slice_outline,
};

use super::super::theme;
use crate::action::Action;
use crate::state::AppState;

/// The side of the square at a plane's own point, in points.
const GRIP: f32 = 12.0;
/// The radius of the knob at the end of a cut's normal, in points.
const KNOB: f32 = 7.0;
const LINE: f32 = 1.5;

/// Draw the handles of the scene laid out in `area`, and turn drags on them into actions: the
/// square at a cut's point moves the cut, the knob at the end of its normal turns it, and
/// the square in the middle of the slice's outline moves on to another slice. `carried`
/// keeps how far the slice has been dragged short of a whole slice.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    carried: &mut f64,
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
        painter.rect_filled(grip, 2, theme::ACCENT);
        if let Some(by) = dragged(&moved.on_hover_text("Drag to move this cut")) {
            // The cut goes with the pointer along its normal; to remove more is to go
            // against it.
            let by_mm = -drag_to_mm(&scene.camera, plane.normal, by);
            actions.push(Action::CutMove { cut, by_mm });
        }
        let end = Rect::from_center_size(knob, vec2(2.0 * KNOB, 2.0 * KNOB));
        let turned = ui.interact(end, ui.id().with(("cut knob", cut)), Sense::drag());
        painter.circle_filled(knob, KNOB, theme::ACCENT);
        if let Some(by) = dragged(&turned.on_hover_text("Drag to turn this cut")) {
            actions.push(Action::CutTurn { cut, by });
        }
    }
    if scene.slice_shown {
        slice(ui, area, state, carried, actions);
    }
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

/// A place in the scene's picture as a place in the window.
fn placed(area: Rect, [x, y]: [f32; 2]) -> Pos2 {
    area.min + vec2(x, y)
}

/// How far a handle was dragged this frame, if it was.
fn dragged(handle: &egui::Response) -> Option<[f32; 2]> {
    let by = handle.drag_delta();
    (handle.dragged() && by.length_sq() > 0.0).then_some([by.x, by.y])
}
