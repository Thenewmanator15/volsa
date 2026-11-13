//! How much each voxel lies in a thin dark tube, from how the smoothed brightness curves there.

use rayon::prelude::*;

/// How much a tube must be rounder than a sheet to count, in Frangi's measure.
const SHEET: f32 = 0.5;
/// How much a tube must be longer than a blob to count, in Frangi's measure.
const BLOB: f32 = 0.5;

/// Score every voxel of `smoothed`, a volume of `sides` voxels `spacing` millimetres apart
/// that was smoothed `width_mm` wide, for how much it lies in a dark tube about that wide, out
/// of 255, and keep the larger of that and what `score` holds already. `curve` is how strongly
/// a tube must curve, in the scan's own values, to count fully. Voxels at the edges are left.
pub(super) fn dark_tubes(
    smoothed: &[f32],
    sides: [usize; 3],
    spacing: [f64; 3],
    width_mm: f64,
    curve: f32,
    score: &mut [u8],
) {
    let [columns, rows, slices] = sides;
    let plane = columns * rows;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "spacings and widths are a few millimetres, which an f32 holds well"
    )]
    let ([hx, hy, hz], width) = (spacing.map(|step| step as f32), width_mm as f32);
    let scale = width * width;
    score
        .par_chunks_mut(plane)
        .enumerate()
        .filter(|(slice, _)| *slice > 0 && *slice + 1 < slices)
        .for_each(|(slice, out)| {
            let at = |place: usize| smoothed.get(place).copied().unwrap_or(0.0);
            for row in 1..rows - 1 {
                for column in 1..columns - 1 {
                    let here = slice * plane + row * columns + column;
                    let middle = at(here);
                    let second = |step: usize, h: f32| {
                        (at(here + step) - 2.0 * middle + at(here - step)) / (h * h)
                    };
                    let mixed = |one: usize, two: usize, h: f32| {
                        let (up, down) = (here + one, here - one);
                        (at(up + two) - at(up - two) - at(down + two) + at(down - two)) / (4.0 * h)
                    };
                    let hessian = [
                        second(1, hx),
                        second(columns, hy),
                        second(plane, hz),
                        mixed(1, columns, hx * hy),
                        mixed(1, plane, hx * hz),
                        mixed(columns, plane, hy * hz),
                    ]
                    .map(|value| value * scale);
                    let tube = dark_tube(eigenvalues(hessian), curve);
                    if let Some(slot) = out.get_mut(row * columns + column) {
                        *slot = (*slot).max(tube);
                    }
                }
            }
        });
}

/// How much a point whose brightness curves as `eigen` lies in a dark tube, out of 255: it
/// curves up strongly across two directions and hardly along the third (Frangi's measure,
/// for dark tubes).
pub(super) fn dark_tube(eigen: [f32; 3], curve: f32) -> u8 {
    let mut sorted = eigen;
    sorted.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    let [along, across, most] = sorted;
    if across <= 0.0 || most <= 0.0 {
        return 0;
    }
    let sheet = across / most;
    let blob = along.abs() / (across * most).sqrt();
    let strength = (along * along + across * across + most * most).sqrt();
    let measure = (1.0 - (-sheet * sheet / (2.0 * SHEET * SHEET)).exp())
        * (-blob * blob / (2.0 * BLOB * BLOB)).exp()
        * (1.0 - (-strength * strength / (2.0 * curve * curve)).exp());
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the measure lies between 0 and 1, so its share of 255 fits a byte"
    )]
    let out = (measure.clamp(0.0, 1.0) * 255.0).round() as u8;
    out
}

/// The eigenvalues of the symmetric matrix whose diagonal is the first three of `matrix` and
/// whose (1,2), (1,3) and (2,3) entries are the last three, largest first.
pub(super) fn eigenvalues(matrix: [f32; 6]) -> [f32; 3] {
    let [xx, yy, zz, xy, xz, yz] = matrix.map(f64::from);
    let off = xy * xy + xz * xz + yz * yz;
    let mean = (xx + yy + zz) / 3.0;
    let spread = (xx - mean).powi(2) + (yy - mean).powi(2) + (zz - mean).powi(2) + 2.0 * off;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "eigenvalues of a scan's curvature are well within an f32"
    )]
    let narrow = |values: [f64; 3]| values.map(|value| value as f32);
    if spread <= f64::EPSILON * (mean * mean).max(f64::MIN_POSITIVE) {
        return narrow([mean; 3]);
    }
    let size = (spread / 6.0).sqrt();
    let [xx, yy, zz] = [xx - mean, yy - mean, zz - mean];
    let det = xx * (yy * zz - yz * yz) - xy * (xy * zz - yz * xz) + xz * (xy * yz - yy * xz);
    let turn = (det / (2.0 * size * size * size)).clamp(-1.0, 1.0);
    let angle = turn.acos() / 3.0;
    let largest = mean + 2.0 * size * angle.cos();
    let smallest = mean + 2.0 * size * (angle + 2.0 * std::f64::consts::FRAC_PI_3).cos();
    narrow([largest, 3.0 * mean - largest - smallest, smallest])
}
