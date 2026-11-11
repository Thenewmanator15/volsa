//! What the scene's camera is fitted to. That is the scan's own box, except in the Parts
//! look when the labels reach beyond the scan, as a whole head's labels do over a thin slab:
//! then it is the labels' box, so that the solids are seen whole.

use voxa_core::{Patient, Point};
use voxa_render::Look;

use crate::labels::look_drawn;
use crate::state::AppState;

/// How far past the scan's box, as a share of the box's longest side, labels must reach to
/// count as reaching beyond it. Labels on the scan's own grid, or a voxel or two larger, do
/// not.
const BEYOND: f64 = 0.05;

/// Keep the scene's frame on what its look draws, after any action. When that changes, the
/// camera is fitted to the new frame from the way it already faces, whoever placed it: it
/// was placed for something else.
pub(super) fn keep(state: &mut AppState) {
    let scan = state.scene.corners;
    let labels = state
        .labels
        .corners
        .filter(|_| look_drawn(state) == Look::Parts);
    let beyond =
        |labels: &[Point<Patient>; 8]| scan.is_some_and(|scan| reaches_beyond(labels, &scan));
    let wanted = labels.filter(beyond).or(scan);
    let scene = &mut state.scene;
    if scene.frame == wanted {
        return;
    }
    let was = std::mem::replace(&mut scene.frame, wanted);
    // A volume that has just arrived was fitted as it arrived, if the frame is its own box.
    let just_fitted = was.is_none() && wanted == scan;
    if let Some(frame) = wanted.filter(|_| !just_fitted) {
        scene.camera = scene.camera.refitted(&frame, scene.viewport);
    }
}

/// Whether a box has a corner more than [`BEYOND`] of another's longest side outside it,
/// both taken as they lie along the patient's own three ways.
fn reaches_beyond(labels: &[Point<Patient>; 8], scan: &[Point<Patient>; 8]) -> bool {
    let axes: [fn(&Point<Patient>) -> f64; 3] = [|at| at.x, |at| at.y, |at| at.z];
    let reach = |corners: &[Point<Patient>; 8], axis: fn(&Point<Patient>) -> f64| {
        let along = corners.iter().map(axis);
        let low = along.clone().fold(f64::INFINITY, f64::min);
        (low, along.fold(f64::NEG_INFINITY, f64::max))
    };
    let sides = axes.map(|axis| reach(scan, axis));
    let longest = sides
        .iter()
        .fold(0.0_f64, |longest, (low, high)| longest.max(high - low));
    let margin = BEYOND * longest;
    axes.into_iter().zip(sides).any(|(axis, (low, high))| {
        let (least, most) = reach(labels, axis);
        least < low - margin || most > high + margin
    })
}
