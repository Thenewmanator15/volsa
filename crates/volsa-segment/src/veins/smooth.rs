//! Smoothing a volume with a Gaussian of a width in millimetres, one side at a time. Past its
//! edges the volume is taken to go on as its edge voxels are.

use rayon::prelude::*;

use super::{VeinsError, whole, zeroed};

/// The narrowest a Gaussian is made along a side, in voxels: narrower than this it would hardly
/// change the volume, and the side is left as it is.
const NARROWEST: f64 = 0.25;

/// `values`, a volume of `sides` voxels (columns fastest), smoothed by a Gaussian `width_mm`
/// wide along each of its three sides, whose voxels are `spacing` millimetres apart.
///
/// # Errors
///
/// [`VeinsError::NoMemory`] when there is not the room for the smoothed volume.
pub(super) fn smoothed(
    values: &[f32],
    sides: [usize; 3],
    spacing: [f64; 3],
    width_mm: f64,
) -> Result<Vec<f32>, VeinsError> {
    let mut now = zeroed::<f32>(values.len())?;
    now.copy_from_slice(values);
    let mut next = zeroed::<f32>(values.len())?;
    for (axis, step) in spacing.iter().enumerate() {
        let weights = kernel(width_mm / step.max(f64::MIN_POSITIVE));
        if weights.len() > 1 {
            along(&now, &mut next, sides, axis, &weights);
            std::mem::swap(&mut now, &mut next);
        }
    }
    Ok(now)
}

/// The weights of a Gaussian `sigma` voxels wide, from the middle out, summing to 1 over both
/// sides; a single 1 when it is too narrow to smooth.
pub(super) fn kernel(sigma: f64) -> Vec<f32> {
    if sigma < NARROWEST || !sigma.is_finite() {
        return vec![1.0];
    }
    let reach = whole((3.0 * sigma).ceil()).max(1);
    let weights: Vec<f64> = (0..=reach)
        .map(|at| (-((at * at) as f64) / (2.0 * sigma * sigma)).exp())
        .collect();
    let first = weights.first().copied().unwrap_or(1.0);
    let total = first + 2.0 * weights.iter().skip(1).sum::<f64>();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "weights lie between 0 and 1, where an f32 holds them well enough"
    )]
    weights
        .iter()
        .map(|weight| (weight / total) as f32)
        .collect()
}

/// Smooth `from` along one side into `into`, a slice of the volume at a time.
fn along(from: &[f32], into: &mut [f32], sides: [usize; 3], axis: usize, weights: &[f32]) {
    let [columns, rows, slices] = sides;
    let plane = columns * rows;
    let (stride, length) = match axis {
        0 => (1, columns),
        1 => (columns, rows),
        _ => (plane, slices),
    };
    into.par_chunks_mut(plane)
        .enumerate()
        .for_each(|(slice, out)| {
            for (at, slot) in out.iter_mut().enumerate() {
                let along_at = match axis {
                    0 => at % columns,
                    1 => at / columns,
                    _ => slice,
                };
                let start = slice * plane + at - along_at * stride;
                let value = |step: usize| from.get(start + step * stride).copied().unwrap_or(0.0);
                let mut sum = 0.0;
                for (reach, weight) in weights.iter().enumerate() {
                    let before = along_at.saturating_sub(reach);
                    let after = (along_at + reach).min(length - 1);
                    sum += if reach == 0 {
                        weight * value(along_at)
                    } else {
                        weight * (value(before) + value(after))
                    };
                }
                *slot = sum;
            }
        });
}
