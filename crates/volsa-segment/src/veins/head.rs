//! Where the head is: the voxels whose surroundings, a few millimetres round, are bright. A
//! dark tube inside the head has bright tissue round it; the dark air outside does not, and
//! nothing there is looked at. The surroundings are measured on a coarse copy of the scan,
//! about two millimetres a voxel, so that this costs little.

use super::{VeinsError, smooth, whole, zeroed};

/// The size of a voxel of the coarse copy, in millimetres.
const COARSE_MM: f64 = 2.0;
/// How far round a voxel its surroundings are measured, in millimetres.
const ROUND_MM: f64 = 4.0;
/// The share of the bright level that the surroundings of a voxel inside must reach.
const INSIDE_SHARE: f32 = 0.3;

/// Whether each voxel of `values` (`sides` voxels, columns fastest, `spacing` millimetres
/// apart) lies inside the head, as said above. `bright` is the scan's bright level.
///
/// # Errors
///
/// [`VeinsError::NoMemory`] when there is not the room for the coarse copy.
pub(super) fn inside(
    values: &[f32],
    sides: [usize; 3],
    spacing: [f64; 3],
    bright: f32,
) -> Result<Vec<bool>, VeinsError> {
    let block = spacing.map(|step| whole((COARSE_MM / step.max(f64::MIN_POSITIVE)).round()).max(1));
    let coarse_sides = [0, 1, 2].map(|axis| side(sides, axis).div_ceil(side(block, axis)));
    let [columns, rows, _] = sides;
    let [coarse_columns, coarse_rows, coarse_slices] = coarse_sides;
    let count = coarse_columns * coarse_rows * coarse_slices;
    let mut sums = zeroed::<f32>(count)?;
    let mut counts = zeroed::<u32>(count)?;
    let coarse_of = |place: usize| {
        let (slice, row, column) = (
            place / (columns * rows),
            place / columns % rows,
            place % columns,
        );
        let [bx, by, bz] = block;
        (slice / bz * coarse_rows + row / by) * coarse_columns + column / bx
    };
    for (place, value) in values.iter().enumerate() {
        let at = coarse_of(place);
        if let (Some(sum), Some(count)) = (sums.get_mut(at), counts.get_mut(at)) {
            *sum += if value.is_finite() { *value } else { 0.0 };
            *count += 1;
        }
    }
    for (sum, count) in sums.iter_mut().zip(&counts) {
        *sum /= (*count).max(1) as f32;
    }
    drop(counts);
    let coarse_spacing = [0, 1, 2].map(|axis| side(spacing, axis) * side(block, axis) as f64);
    let smoothed = smooth::smoothed(&sums, coarse_sides, coarse_spacing, ROUND_MM)?;
    let level = INSIDE_SHARE * bright;
    let mut inside = zeroed::<bool>(values.len())?;
    for (place, slot) in inside.iter_mut().enumerate() {
        *slot = smoothed
            .get(coarse_of(place))
            .is_some_and(|value| *value > level);
    }
    Ok(inside)
}

/// One of three.
fn side<T: Copy>(three: [T; 3], axis: usize) -> T {
    let [x, y, z] = three;
    match axis {
        0 => x,
        1 => y,
        _ => z,
    }
}
