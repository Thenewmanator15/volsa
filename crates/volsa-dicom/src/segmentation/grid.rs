//! Putting the frames of a segmentation into one grid: which slice each frame is, how many
//! slices there are, and where the grid lies in the patient.
//!
//! The frames are ordered along the slice normal. The step between slices is the smallest gap
//! between two frames at different places; every frame must then lie a whole number of steps
//! from the first, give or take a tenth of a step, as Volsa asks of the slices of any scan. A
//! segmentation stores frames only where it marks something, so whole steps with no frame are
//! slices with nothing in them; a frame between two steps makes the slices uneven, and the
//! segmentation is refused.

use volsa_core::{Affine, Patient, Voxel};

use super::frames::{Frames, NOT_ON_A_GRID};
use super::{MAX_SEGMENTATION_VOXELS, SegmentationError, refused};
use crate::geometry::{SAME_POSITION_MM, cross, dot};

/// How far from a whole number of steps a frame may lie, as a fraction of a step.
const UNEVEN_FRACTION: f64 = 0.1;
/// What is said when the frames are not evenly spaced.
const UNEVEN: &str =
    "The slices of this segmentation are not evenly spaced, so Volsa does not open it as labels.";
/// What is said of a segmentation whose grid would be too large.
pub(super) const TOO_LARGE: &str = "This segmentation is larger than Volsa will read.";

/// Where the frames go in one grid.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Placed {
    /// How many slices the grid has.
    pub slices: u32,
    /// The slice each frame is, in the order the frames are stored.
    pub slice_of: Vec<u32>,
    /// Where the voxel at (column, row, slice) lies in the patient.
    pub placement: Affine<Voxel, Patient>,
}

/// Put `frames` of `rows` × `cols` pixels into one grid.
///
/// # Errors
///
/// When the frames are not evenly spaced, do not lie on one grid, or make a grid of more than
/// [`MAX_SEGMENTATION_VOXELS`] voxels.
pub(super) fn place(frames: &Frames, rows: u32, cols: u32) -> Result<Placed, SegmentationError> {
    let [ax, ay, az, dx, dy, dz] = frames.orientation;
    let (across, down) = ([ax, ay, az], [dx, dy, dz]);
    let normal = cross(across, down);
    let along = |at: &[f64; 3]| dot(*at, normal);
    let by_place = |a: &&[f64; 3], b: &&[f64; 3]| along(a).total_cmp(&along(b));
    let positions = &frames.positions;
    let first = *positions
        .iter()
        .min_by(by_place)
        .ok_or_else(|| refused(NOT_ON_A_GRID))?;
    let last = *positions
        .iter()
        .max_by(by_place)
        .ok_or_else(|| refused(NOT_ON_A_GRID))?;
    let (step, axis, slices) = match smallest_gap(positions, normal) {
        None => {
            let step = frames.stated_step.unwrap_or(1.0);
            (step, normal.map(|v| v * step), 1)
        }
        Some(step) => {
            let gaps = whole((along(&last) - along(&first)) / step).ok_or_else(too_large)?;
            let spread = sub(last, first).map(|v| v / f64::from(gaps));
            (step, spread, gaps.checked_add(1).ok_or_else(too_large)?)
        }
    };
    let voxels = u64::from(rows) * u64::from(cols) * u64::from(slices);
    if voxels > MAX_SEGMENTATION_VOXELS {
        return Err(too_large());
    }
    let slice_of = positions
        .iter()
        .map(|at| {
            let steps = (along(at) - along(&first)) / step;
            let slice = whole(steps).ok_or_else(too_large)?;
            let even = (steps - f64::from(slice)).abs() <= UNEVEN_FRACTION;
            even.then_some(slice).ok_or_else(|| refused(UNEVEN))
        })
        .collect::<Result<Vec<u32>, _>>()?;
    // Across the plane, each frame must lie where its slice of the grid does, give or take a
    // tenth of a pixel.
    let [between_rows, between_cols] = frames.spacing;
    let slack = UNEVEN_FRACTION * between_rows.min(between_cols);
    for (at, slice) in positions.iter().zip(&slice_of) {
        let off = sub(*at, add(first, axis.map(|v| v * f64::from(*slice))));
        let across_the_plane = sub(off, normal.map(|v| v * dot(off, normal)));
        if dot(across_the_plane, across_the_plane).sqrt() > slack {
            return Err(refused(NOT_ON_A_GRID));
        }
    }
    Ok(Placed {
        slices,
        slice_of,
        placement: Affine::from_columns(
            across.map(|v| v * between_cols),
            down.map(|v| v * between_rows),
            axis,
            first,
        ),
    })
}

/// The smallest distance along `normal` between two of `positions` at different places;
/// `None` when they are all at one place.
fn smallest_gap(positions: &[[f64; 3]], normal: [f64; 3]) -> Option<f64> {
    let mut places: Vec<f64> = positions.iter().map(|at| dot(*at, normal)).collect();
    places.sort_by(f64::total_cmp);
    let gaps = places.windows(2).filter_map(|pair| match pair {
        [here, next] => Some(next - here),
        _ => None,
    });
    gaps.filter(|gap| *gap >= SAME_POSITION_MM)
        .min_by(f64::total_cmp)
}

/// The whole number nearest a count of steps, when it is one a grid can have.
fn whole(steps: f64) -> Option<u32> {
    let nearest = steps.round();
    // A grid has no more slices than voxels, so any count above that is refused before the
    // cast, which then neither wraps nor loses a sign.
    let fits = nearest.is_finite() && (0.0..=MAX_SEGMENTATION_VOXELS as f64).contains(&nearest);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to lie within 0..=MAX_SEGMENTATION_VOXELS first"
    )]
    fits.then_some(nearest as u32)
}

fn too_large() -> SegmentationError {
    refused(TOO_LARGE)
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
