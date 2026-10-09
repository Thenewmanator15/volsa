//! Actions about the plane views, and keeping their position inside the scene's volume.

use voxa_core::{Patient, Point};
use voxa_render::{MAX_PLANE_ZOOM, MIN_PLANE_ZOOM};
use voxa_volume::{PlaneKind, central_voxel, clamped_into, scrolled};

use crate::action::Action;
use crate::planes::{PlanePanel, grid, plane};
use crate::state::AppState;

/// The farthest the middle of a panel can be moved from the middle of its plane, in
/// millimetres: far past any scan, and still a number that can be worked with.
const FARTHEST_MM: f64 = 1.0e4;

/// Whether an action is about the plane views.
pub(super) fn concerns(action: &Action) -> bool {
    matches!(
        action,
        Action::SetPosition(_)
            | Action::PlaneScroll { .. }
            | Action::PlaneZoom { .. }
            | Action::PlanePan { .. }
            | Action::PlaneFit(_)
            | Action::SetColumn(_)
    )
}

pub(super) fn reduce(state: &mut AppState, action: &Action) {
    match *action {
        Action::SetPosition(point) => set_position(state, point),
        Action::PlaneScroll { kind, steps } => scroll(state, kind, steps),
        Action::PlaneZoom {
            kind,
            factor,
            about_mm,
        } => zoom(state.planes.panel_mut(kind), factor, about_mm),
        Action::PlanePan { kind, by_mm } => pan(state.planes.panel_mut(kind), by_mm),
        Action::PlaneFit(kind) => *state.planes.panel_mut(kind) = PlanePanel::default(),
        Action::SetColumn(column) => state.planes.column = column,
        _ => {}
    }
}

/// Keep the position where there is a volume to have one in: on the voxel in the middle of a
/// volume that has just become ready, and nowhere, with the panels as they start, while
/// there is none.
pub(super) fn settle(state: &mut AppState) {
    if let Some((placement, dims)) = grid(state) {
        let middle = central_voxel(&placement, dims);
        state.planes.position.get_or_insert(middle);
    } else {
        state.planes.position = None;
        state.planes.panels = [PlanePanel::default(); 3];
    }
}

/// Move the position, holding it inside the volume. Nothing happens with no volume ready.
fn set_position(state: &mut AppState, point: Point<Patient>) {
    if let Some((placement, dims)) = grid(state) {
        state.planes.position = Some(clamped_into(&placement, dims, point));
    }
}

/// Move the position whole layers along one plane's normal.
fn scroll(state: &mut AppState, kind: PlaneKind, steps: i32) {
    let moved = || {
        let (placement, dims) = grid(state)?;
        let normal = plane(state, kind)?.normal();
        scrolled(&placement, dims, state.planes.position?, normal, steps)
    };
    if let Some(moved) = moved() {
        state.planes.position = Some(moved);
    }
}

/// Magnify a panel, keeping still the point of the plane `about_mm` from its centre.
fn zoom(panel: &mut PlanePanel, factor: f64, about_mm: [f64; 2]) {
    let usable = factor.is_finite() && factor > 0.0 && about_mm.into_iter().all(f64::is_finite);
    if !usable {
        return;
    }
    let zoomed = (panel.zoom * factor).clamp(MIN_PLANE_ZOOM, MAX_PLANE_ZOOM);
    let by = zoomed / panel.zoom;
    let [about_across, about_down] = about_mm;
    let [across, down] = panel.offset_mm;
    panel.offset_mm = [
        about_across - (about_across - across) / by,
        about_down - (about_down - down) / by,
    ];
    panel.zoom = zoomed;
}

/// Move what is in the middle of a panel by this much of its plane.
fn pan(panel: &mut PlanePanel, by_mm: [f64; 2]) {
    let [by_across, by_down] = by_mm;
    if !(by_across.is_finite() && by_down.is_finite()) {
        return;
    }
    let [across, down] = panel.offset_mm;
    let held = |mm: f64| mm.clamp(-FARTHEST_MM, FARTHEST_MM);
    panel.offset_mm = [held(across + by_across), held(down + by_down)];
}
