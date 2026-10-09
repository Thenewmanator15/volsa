//! A smooth curve through the values of a volume: cubic B-spline interpolation, after
//! Thévenaz, Blu and Unser, "Interpolation revisited", IEEE Transactions on Medical Imaging
//! 19 (2000), equations 4 to 6. The values are turned, once, into the spline's coefficients
//! by a filter run forwards and backwards along each axis; the value anywhere is then a
//! weighted sum of the 64 coefficients round it.
//!
//! The curve passes through every measured value. Between them it is smooth, which is not
//! the same as showing more detail than was measured.

use voxa_core::Volume;

/// The pole of the cubic B-spline's filter: the square root of 3, less 2.
const POLE: f32 = -0.267_949_2;
/// What the values are multiplied by before the filter, so that what comes out of it are the
/// coefficients: (1 - pole)(1 - 1 / pole).
const GAIN: f32 = 6.0;
/// How many terms of the sum that starts the filter are kept. The pole to this power is far
/// below what an `f32` can tell from nothing beside a value.
const TERMS: i64 = 16;

/// The coefficients of the cubic B-spline through a volume's values, in the order of the
/// values. The ends are mirrored about the volume's faces: the value one voxel beyond a face
/// is taken to be the value one voxel inside it. A value that is not a number is read as the
/// smallest that is, so that it does not spoil the rest.
pub fn coefficients(volume: &Volume) -> Vec<f32> {
    let [cols, rows, _] = volume
        .dims()
        .map(|count| usize::try_from(count).unwrap_or(0));
    let finite = volume.values().iter().filter(|value| value.is_finite());
    let smallest = finite.copied().fold(f32::INFINITY, f32::min);
    let floor = if smallest.is_finite() { smallest } else { 0.0 };
    let read = |value: &f32| if value.is_finite() { *value } else { floor };
    let mut values: Vec<f32> = volume.values().iter().map(read).collect();
    let (row, slice) = (cols, cols * rows);
    if slice == 0 {
        return values;
    }
    // Along the columns of each row, then the rows of each slice, then the slices.
    for line in values.chunks_exact_mut(row) {
        along(line, 1);
    }
    for layer in values.chunks_exact_mut(slice) {
        along(layer, row);
    }
    along(&mut values, slice);
    values
}

/// Run the filter over a block of layers, each of `step` values: every value with the ones
/// at its own place in the layers before and after it.
fn along(block: &mut [f32], step: usize) {
    let count = block.len() / step.max(1);
    if count == 0 {
        return;
    }
    for value in block.iter_mut() {
        *value *= GAIN;
    }
    // Forwards. The first layer is the sum of everything before it, mirrored, each layer
    // further back counting for less by the pole.
    let mut first = vec![0.0_f32; step];
    let mut weight = 1.0_f32;
    for back in 0..TERMS {
        let layer = block.chunks_exact(step).nth(mirrored(-back, count));
        for (sum, value) in first.iter_mut().zip(layer.unwrap_or_default()) {
            *sum += weight * value;
        }
        weight *= POLE;
    }
    let mut layers = block.chunks_exact_mut(step);
    let Some(mut before) = layers.next() else {
        return;
    };
    before.copy_from_slice(&first);
    for layer in layers {
        for (value, before) in layer.iter_mut().zip(before.iter()) {
            *value += POLE * before;
        }
        before = layer;
    }
    // Backwards. The layer beyond the last is the last, mirrored, which gives the last.
    let mut layers = block.chunks_exact_mut(step).rev();
    let Some(mut after) = layers.next() else {
        return;
    };
    for value in after.iter_mut() {
        *value *= POLE / (POLE - 1.0);
    }
    for layer in layers {
        for (value, after) in layer.iter_mut().zip(after.iter()) {
            *value = POLE * (after - *value);
        }
        after = layer;
    }
}

/// Which of a line of `count` voxels a place before its first or after its last stands for,
/// the line being mirrored about each of its ends: one before the first is the first, two
/// before it the second.
fn mirrored(index: i64, count: usize) -> usize {
    let count = i64::try_from(count).unwrap_or(i64::MAX).max(1);
    let within = index.rem_euclid(2 * count);
    let folded = if within < count {
        within
    } else {
        2 * count - 1 - within
    };
    usize::try_from(folded).unwrap_or(0)
}

/// The value of the curve at a place in a volume, given in voxels along its columns, rows
/// and slices, counted from voxel centres. `coefficients` are those [`coefficients`] made of
/// a volume of `dims`. `None` outside the volume, more than half a voxel beyond the centres
/// of its edge voxels, and when the coefficients are not those of such a volume.
pub fn sample_cubic(dims: [u32; 3], coefficients: &[f32], voxel: [f64; 3]) -> Option<f32> {
    let [cols, rows, slices] = dims;
    let [x, y, z] = voxel;
    let (across, down, through) = (taps(x, cols)?, taps(y, rows)?, taps(z, slices)?);
    let [cols, rows, slices] = dims.map(|count| usize::try_from(count).unwrap_or(0));
    if coefficients.len() != cols * rows * slices {
        return None;
    }
    let mut sum = 0.0_f64;
    for (slice, deep) in through {
        for (row, high) in down {
            for (col, wide) in across {
                let coefficient = coefficients.get((slice * rows + row) * cols + col)?;
                sum += f64::from(*coefficient) * deep * high * wide;
            }
        }
    }
    Some(narrowed(sum))
}

/// The four voxels along one axis whose coefficients a place draws on, and how much of each.
/// `None` when the place is outside a line of `count` voxels, or is not a number.
fn taps(at: f64, count: u32) -> Option<[(usize, f64); 4]> {
    if count == 0 || !(-0.5..=f64::from(count) - 0.5).contains(&at) {
        return None;
    }
    let whole = at.floor();
    let part = at - whole;
    let rest = 1.0 - part;
    let shares = [
        rest.powi(3) / 6.0,
        2.0 / 3.0 - part * part + part.powi(3) / 2.0,
        2.0 / 3.0 - rest * rest + rest.powi(3) / 2.0,
        part.powi(3) / 6.0,
    ];
    let first = index(whole) - 1;
    let count = usize::try_from(count).unwrap_or(0);
    let mut taken = [(0, 0.0); 4];
    for ((tap, offset), share) in taken.iter_mut().zip(0..).zip(shares) {
        *tap = (mirrored(first + offset, count), share);
    }
    Some(taken)
}

/// A whole number of voxels, which is small, as an integer.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a place within a volume, already checked"
)]
fn index(whole: f64) -> i64 {
    whole as i64
}

/// A sum worked out in double precision, as the precision the values are kept in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a blend of f32 values is within their range"
)]
fn narrowed(sum: f64) -> f32 {
    sum as f32
}

#[cfg(test)]
mod tests {
    use voxa_core::{Affine, Point};

    use super::*;
    use crate::sample::sample;

    /// A volume whose voxels are 1 mm apart from the origin, each value given by its column,
    /// row and slice.
    fn made(dims: [u32; 3], value: impl Fn(u32, u32, u32) -> f32) -> Volume {
        let [cols, rows, slices] = dims;
        let mut values = Vec::new();
        for slice in 0..slices {
            for row in 0..rows {
                for col in 0..cols {
                    values.push(value(col, row, slice));
                }
            }
        }
        Volume::new(dims, values, Affine::identity()).expect("adds up")
    }

    /// A number from 0 to 100 that looks like chance and is the same every time.
    fn scattered(seed: u32) -> f32 {
        let mixed = seed.wrapping_mul(2_654_435_761).rotate_left(13) % 10_000;
        f32::from(u16::try_from(mixed).expect("below 10,000")) / 100.0
    }

    fn real(part: u32) -> f32 {
        f32::from(u16::try_from(part).expect("a small number"))
    }

    #[test]
    fn the_curve_passes_through_every_measured_value() {
        let dims = [6, 6, 6];
        let volume = made(dims, |col, row, slice| {
            scattered(col + 6 * row + 36 * slice)
        });
        let curve = coefficients(&volume);
        assert_eq!(curve.len(), 216);
        for slice in 0..6 {
            for row in 0..6 {
                for col in 0..6 {
                    let at = [col, row, slice].map(f64::from);
                    let here = sample_cubic(dims, &curve, at).expect("inside");
                    let measured = scattered(col + 6 * row + 36 * slice);
                    assert!(
                        (here - measured).abs() < 1.0e-3,
                        "{here} for {measured} at {at:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn between_slices_it_follows_the_bend_where_the_blend_cuts_across_it() {
        let dims = [4, 4, 16];
        let squares = made(dims, |_, _, slice| real(slice * slice));
        let curve = coefficients(&squares);
        let between = sample_cubic(dims, &curve, [1.5, 1.5, 4.5]).expect("inside");
        assert!(
            (between - 20.25).abs() < 0.05,
            "four and a half, squared: {between}"
        );
        let blend = sample(&squares, Point::new(1.5, 1.5, 4.5)).expect("inside");
        assert_eq!(blend, 20.5, "half way from 16 to 25");
    }

    #[test]
    fn on_a_slope_the_curve_and_the_blend_agree() {
        let dims = [20, 20, 20];
        let slope = made(dims, |col, row, slice| real(2 * col + 3 * row + 20 - slice));
        let curve = coefficients(&slope);
        let (x, y, z) = (9.3, 10.7, 9.5);
        let expected = 2.0 * 9.3 + 3.0 * 10.7 + 20.0 - 9.5;
        let smooth = sample_cubic(dims, &curve, [x, y, z]).expect("inside");
        let blend = sample(&slope, Point::new(x, y, z)).expect("inside");
        assert!(
            (smooth - expected).abs() < 1.0e-3,
            "{smooth} for {expected}"
        );
        assert!((blend - expected).abs() < 1.0e-3, "{blend} for {expected}");
    }

    #[test]
    fn outside_the_volume_there_is_no_value() {
        let dims = [4, 5, 6];
        let volume = made(dims, |col, _, _| real(col));
        let curve = coefficients(&volume);
        let at = |voxel| sample_cubic(dims, &curve, voxel);
        assert!(at([-0.5, 0.0, 0.0]).is_some() && at([3.5, 4.5, 5.5]).is_some());
        for outside in [
            [-0.6, 0.0, 0.0],
            [3.6, 0.0, 0.0],
            [0.0, 4.6, 0.0],
            [0.0, 0.0, 5.6],
        ] {
            assert_eq!(at(outside), None, "{outside:?}");
        }
        assert_eq!(at([f64::NAN, 0.0, 0.0]), None);
        assert_eq!(at([0.0, f64::INFINITY, 0.0]), None);
        assert_eq!(
            sample_cubic(dims, &curve[1..], [1.0; 3]),
            None,
            "not its coefficients"
        );
        assert_eq!(
            sample_cubic([0, 5, 6], &[], [0.0; 3]),
            None,
            "a volume of nothing"
        );
    }

    #[test]
    fn a_volume_of_one_voxel_is_that_value_all_through() {
        let one = made([1, 1, 1], |_, _, _| 7.0);
        let curve = coefficients(&one);
        assert!((curve[0] - 7.0).abs() < 1.0e-4, "{curve:?}");
        for voxel in [[0.0; 3], [0.4, -0.4, 0.2]] {
            let here = sample_cubic([1, 1, 1], &curve, voxel).expect("inside");
            assert!((here - 7.0).abs() < 1.0e-4, "{here} at {voxel:?}");
        }
    }

    #[test]
    fn a_value_that_is_not_a_number_does_not_spoil_the_rest() {
        let dims = [5, 5, 5];
        let holed = made(dims, |col, row, slice| {
            if [col, row, slice] == [2, 2, 2] {
                f32::NAN
            } else {
                10.0 + real(col)
            }
        });
        let curve = coefficients(&holed);
        assert!(curve.iter().all(|value| value.is_finite()));
        let far = sample_cubic(dims, &curve, [4.0, 0.0, 0.0]).expect("inside");
        assert!((far - 14.0).abs() < 1.0e-3, "{far}");
        let hole = sample_cubic(dims, &curve, [2.0, 2.0, 2.0]).expect("inside");
        assert!(
            (hole - 10.0).abs() < 1.0e-3,
            "read as the smallest value: {hole}"
        );
    }

    #[test]
    fn the_ends_are_mirrored_about_the_faces() {
        let folded = [-5, -2, -1, 0, 3, 4, 5, 8].map(|index| mirrored(index, 4));
        assert_eq!(folded, [3, 1, 0, 0, 3, 3, 2, 0]);
        assert_eq!([-3, 0, 7].map(|index| mirrored(index, 1)), [0, 0, 0]);
    }
}
