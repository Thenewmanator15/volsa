//! Bringing a volume to 1 mm along its own axes, as `SynthSeg`'s `resample_volume` does.
//!
//! With `f` the spacing in millimetres along an axis of `n` voxels:
//!
//! 1. The volume is blurred along that axis with a Gaussian `0.25 / f` voxels wide where `f`
//!    is 1 or less, and not at all where it is more: a fine axis is blurred before it is
//!    thinned out. An axis at exactly 1 mm is blurred too, when another axis has the volume
//!    resampled.
//! 2. Sample `k` of the new grid is read at `(k - (f - 1) / 2) / f` voxels along the axis, held
//!    within the volume, by straight-line blending of the voxels around it.
//! 3. The new transform's columns are the old divided by `f`, and its origin moves back by
//!    half of `f - 1` new steps along each axis.
//!
//! `SynthSeg` does this with `SciPy` and `NumPy`, and three of their habits decide the numbers,
//! so they are kept here:
//!
//! - `SciPy`'s `gaussian_filter` cuts its kernel off `int(4 x sigma + 0.5)` voxels out and
//!   carries a line on past its ends by reflection, the end voxel coming twice.
//! - `NumPy`'s `arange(start, stop, step)` decides how many samples there are, as
//!   `ceil((stop - start) / step)` in floating point. That is usually `ceil(n x f)` and
//!   sometimes one more; the extra sample falls past the end and reads the last voxel again.
//! - `SciPy`'s `RegularGridInterpolator` adds the eight neighbours up in a fixed order.
//!
//! Everything is worked out in double precision, as `SynthSeg` works, and the result is kept
//! as the single precision a `Volume` holds.

use std::borrow::Cow;

use voxa_core::{Affine, Patient, Volume, Voxel};

use super::{FEWEST, LONGEST, narrowed, whole};

/// `SynthSeg` leaves a volume alone when every spacing is within this of 1 mm.
const NEAR_ENOUGH_MM: f64 = 0.05;
/// How wide the blur is, in voxels of the 1 mm grid.
const BLUR: f64 = 0.25;
/// `SciPy` cuts a Gaussian kernel off this many of its widths out.
const KERNEL_REACH: f64 = 4.0;
/// `SciPy` does not blur at all with a Gaussian narrower than this.
const NO_BLUR: f64 = 1e-15;

/// Whether `SynthSeg` resamples a volume with this spacing: when any axis is more than 0.05 mm
/// from 1 mm. Then every axis is resampled, the near ones too.
fn is_needed(spacing: [f64; 3]) -> bool {
    spacing
        .iter()
        .any(|&step| step > 1.0 + NEAR_ENOUGH_MM || step < 1.0 - NEAR_ENOUGH_MM)
}

/// How many voxels of the 1 mm grid there are to one voxel of the volume, along its columns,
/// rows and slices: its spacing in millimetres when it is resampled, 1 when it is left as it
/// is.
pub(super) fn steps(volume: &Volume) -> [f64; 3] {
    let spacing = volume.spacing_mm();
    if is_needed(spacing) {
        spacing
    } else {
        [1.0; 3]
    }
}

/// How many voxels the volume's 1 mm grid has along its columns, rows and slices: the
/// volume's own counts when it needs no resampling. Worked out from the sizes and the
/// spacing alone, without reading a value. A spacing far past any scan's gives no voxels, or
/// the most a count can hold.
pub(super) fn sides(volume: &Volume) -> [u32; 3] {
    let spacing = volume.spacing_mm();
    if !is_needed(spacing) {
        return volume.dims();
    }
    let [cols, rows, slices] = volume.dims();
    let [across, down, through] = spacing;
    [
        Sampling::of(cols, across).side(),
        Sampling::of(rows, down).side(),
        Sampling::of(slices, through).side(),
    ]
}

/// The volume at 1 mm along its own axes. The volume itself when it needs no resampling, and
/// also when its 1 mm grid would have a side under `FEWEST` voxels or over `LONGEST`: such
/// a volume is refused before it comes here.
pub(super) fn one_mm(volume: &Volume) -> Cow<'_, Volume> {
    let spacing = volume.spacing_mm();
    if !is_needed(spacing) {
        return Cow::Borrowed(volume);
    }
    resample(volume, spacing).map_or(Cow::Borrowed(volume), Cow::Owned)
}

fn resample(volume: &Volume, spacing: [f64; 3]) -> Option<Volume> {
    let sides = sides(volume);
    // A side of four or more also keeps every kernel shorter than the line it blurs.
    if sides.iter().any(|side| !(FEWEST..=LONGEST).contains(side)) {
        return None;
    }
    let dims = volume.dims().map(|count| count as usize);
    let mut work: Vec<f64> = volume
        .values()
        .iter()
        .map(|&value| f64::from(value))
        .collect();
    for (axis, &step) in spacing.iter().enumerate() {
        let sigma = if step > 1.0 { 0.0 } else { BLUR / step };
        if sigma > NO_BLUR {
            blur(&mut work, dims, axis, sigma);
        }
    }
    let [cols, rows, slices] = volume.dims();
    let [across, down, through] = spacing;
    let readings = [
        Sampling::of(cols, across).readings(cols),
        Sampling::of(rows, down).readings(rows),
        Sampling::of(slices, through).readings(slices),
    ];
    Volume::new(
        sides,
        blend(&work, dims, &readings),
        placement(volume, spacing),
    )
}

/// The samples the 1 mm grid takes along one axis of a volume, as `SynthSeg` lays them out with
/// `NumPy`'s `arange`.
#[derive(Debug, Clone, Copy)]
struct Sampling {
    /// Where the first sample is read, in voxels along the axis.
    start: f64,
    /// Where the second is read. `NumPy` steps on by the difference of these two.
    next: f64,
    /// How many samples there are, as `NumPy` counts them.
    samples: f64,
}

impl Sampling {
    /// For an axis of `count` voxels `spacing` millimetres apart.
    fn of(count: u32, spacing: f64) -> Self {
        let start = -(spacing - 1.0) / (2.0 * spacing);
        let step = 1.0 / spacing;
        let end = start + step * (f64::from(count) * spacing).ceil();
        Self {
            start,
            next: start + step,
            samples: ((end - start) / step).ceil(),
        }
    }

    /// How many voxels the 1 mm grid has along this axis: none when the count is no number
    /// or under one, and the most a count can hold when it is past that.
    fn side(&self) -> u32 {
        if self.samples >= 1.0 {
            u32::try_from(whole(self.samples)).unwrap_or(u32::MAX)
        } else {
            0
        }
    }

    /// Where each sample reads an axis of `count` voxels.
    fn readings(&self, count: u32) -> Vec<Reading> {
        let last = count.saturating_sub(1);
        let step = self.next - self.start;
        (0..self.side())
            .map(|sample| {
                let at = match sample {
                    0 => self.start,
                    1 => self.next,
                    _ => self.start + f64::from(sample) * step,
                };
                let at = at.clamp(0.0, f64::from(last));
                // SciPy looks for the pair of voxels a place lies between; the last voxel is
                // the far end of the last pair.
                let below = u32::try_from(whole(at.floor())).unwrap_or(last);
                let below = below.min(last.saturating_sub(1));
                Reading {
                    below: below as usize,
                    above: (below + 1).min(last) as usize,
                    along: at - f64::from(below),
                }
            })
            .collect()
    }
}

/// Where one sample reads an axis: the voxel below it, the voxel above, and how far along
/// from the one to the other it lies, from 0 to 1.
#[derive(Debug, Clone, Copy)]
struct Reading {
    below: usize,
    above: usize,
    along: f64,
}

/// One axis of a block held columns fastest: how many voxels lie along it, and how far apart
/// two neighbours along it are in the list of values.
#[derive(Debug, Clone, Copy)]
struct Axis {
    length: usize,
    stride: usize,
}

/// Blurs every line of the block along one axis (0 columns, 1 rows, 2 slices) with a Gaussian
/// `sigma` voxels wide, as `SciPy`'s `gaussian_filter1d` does.
fn blur(values: &mut [f64], dims: [usize; 3], axis: usize, sigma: f64) {
    let [cols, rows, slices] = dims;
    let across = Axis {
        length: cols,
        stride: 1,
    };
    let down = Axis {
        length: rows,
        stride: cols,
    };
    let through = Axis {
        length: slices,
        stride: cols * rows,
    };
    let (along, first, second) = match axis {
        0 => (across, down, through),
        1 => (down, across, through),
        _ => (through, across, down),
    };
    let weights = kernel(sigma);
    let radius = weights.len() / 2;
    let reach = isize::try_from(radius).unwrap_or(isize::MAX);
    // One line at a time, carried on past both its ends.
    let mut line = vec![0.0; along.length + 2 * radius];
    for (a, b) in (0..second.length).flat_map(|b| (0..first.length).map(move |a| (a, b))) {
        let start = a * first.stride + b * second.stride;
        for (held, place) in line.iter_mut().zip(-reach..) {
            let from = start + reflected(place, along.length) * along.stride;
            *held = values.get(from).copied().unwrap_or(0.0);
        }
        for (voxel, window) in line.windows(weights.len()).enumerate() {
            if let Some(value) = values.get_mut(start + voxel * along.stride) {
                *value = blurred(window, &weights);
            }
        }
    }
}

/// `SciPy`'s Gaussian kernel for a blur `sigma` voxels wide: an odd count of weights that add
/// up to 1, the middle one the largest.
fn kernel(sigma: f64) -> Vec<f64> {
    let radius = whole(KERNEL_REACH * sigma + 0.5);
    let weights: Vec<f64> = (0..=2 * radius)
        .map(|place| {
            let away = place.abs_diff(radius) as f64;
            (-0.5 / (sigma * sigma) * (away * away)).exp()
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    weights.into_iter().map(|weight| weight / sum).collect()
}

/// Which voxel of a line of `len` stands at `place`, a place that may lie before the line or
/// past it: `SciPy`'s "reflect", which carries `a b c d` on as `d c b a | a b c d | d c b a`.
pub(super) fn reflected(place: isize, len: usize) -> usize {
    let Ok(period) = isize::try_from(2 * len) else {
        return 0;
    };
    if period == 0 {
        return 0;
    }
    let within = usize::try_from(place.rem_euclid(period)).unwrap_or(0);
    if within < len {
        within
    } else {
        2 * len - 1 - within
    }
}

/// One blurred value, from as many neighbours as there are weights: the middle one first,
/// then each pair of neighbours from the farthest in, as `SciPy` adds them up for a kernel that
/// reads the same both ways.
fn blurred(window: &[f64], weights: &[f64]) -> f64 {
    let radius = weights.len() / 2;
    let middle = |numbers: &[f64]| numbers.get(radius).copied().unwrap_or(0.0);
    let pairs = window
        .iter()
        .zip(window.iter().rev())
        .zip(weights)
        .take(radius);
    pairs.fold(
        middle(window) * middle(weights),
        |sum, ((before, after), weight)| sum + (before + after) * weight,
    )
}

/// The values of the 1 mm grid, columns fastest: each blended from the eight voxels around
/// where it is read, in the order `SciPy`'s `RegularGridInterpolator` adds them up.
fn blend(work: &[f64], dims: [usize; 3], readings: &[Vec<Reading>; 3]) -> Vec<f32> {
    let [cols, rows, _] = dims;
    let [across, down, through] = readings;
    let at = |col: usize, row: usize, slice: usize| {
        let place = (slice * rows + row) * cols + col;
        work.get(place).copied().unwrap_or(0.0)
    };
    let ends = |reading: &Reading| {
        [
            (reading.below, 1.0 - reading.along),
            (reading.above, reading.along),
        ]
    };
    let mut values = Vec::with_capacity(across.len() * down.len() * through.len());
    for z in through {
        for y in down {
            for x in across {
                let mut value = 0.0;
                for (col, col_share) in ends(x) {
                    for (row, row_share) in ends(y) {
                        for (slice, slice_share) in ends(z) {
                            value += at(col, row, slice) * (col_share * row_share * slice_share);
                        }
                    }
                }
                values.push(narrowed(value));
            }
        }
    }
    values
}

/// Where the 1 mm grid lies in the patient: each column of the volume's transform divided by
/// its spacing, and the origin moved back by half of `f - 1` of the new steps along each
/// axis. The same sum in Voxa's left-posterior-superior as in `SynthSeg`'s
/// right-anterior-superior, since the two differ only in the sign of two rows.
fn placement(volume: &Volume, spacing: [f64; 3]) -> Affine<Voxel, Patient> {
    let old = volume.voxel_to_patient();
    let [across, down, through] = spacing;
    let [x, y, z] = old.columns();
    let x = x.map(|part| part / across);
    let y = y.map(|part| part / down);
    let z = z.map(|part| part / through);
    let back = [across, down, through].map(|step| 0.5 * (step - 1.0));
    let mut origin = old.origin();
    for (((part, x), y), z) in origin.iter_mut().zip(x).zip(y).zip(z) {
        let [by_x, by_y, by_z] = back;
        *part -= x * by_x + y * by_y + z * by_z;
    }
    Affine::from_columns(x, y, z, origin)
}
