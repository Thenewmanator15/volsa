//! Turning one of Voxa's volumes into the image `SynthSeg`'s networks see, in `SynthSeg`'s own
//! steps and order, and remembering how to come back:
//!
//! 1. *Resample* to 1 mm along the volume's own axes, when any spacing is more than 0.05 mm
//!    from 1 mm (the module `resample`).
//! 2. *Turn* the axes, without resampling, so that x runs to the patient's right, y to the
//!    front and z to the top (the module `align`).
//! 3. *Cut* to the head, when asked.
//! 4. *Scale* the brightness: the 0.5th percentile becomes 0 and the 99.5th becomes 1, and
//!    everything outside them is held at 0 or 1.
//! 5. *Pad* each side with zeros to the next multiple of 32, and at least 128.
//!
//! The steps follow `preprocess` in `SynthSeg`'s `predict_synthseg.py`, number for number.

use std::borrow::Cow;

use voxa_core::{Affine, Patient, Volume, Voxel};
use voxa_models::{Cut, Numbers, Tensor};

mod align;
mod resample;
#[cfg(test)]
mod tests;

use align::{Turn, Turned};

/// A volume needs this many voxels along every side, as given and at 1 mm.
const FEWEST: u32 = 4;
/// The 1 mm grid may be no longer than this along any side.
const LONGEST: u32 = 512;
/// The percentile that becomes 0.
const LOW_PERCENT: f64 = 0.5;
/// The percentile that becomes 1.
const HIGH_PERCENT: f64 = 99.5;
/// A network halves its image five times, so each side is padded to a multiple of this.
const MULTIPLE: usize = 32;
/// No side is padded to less than this.
const SMALLEST: usize = 128;

/// How to come back from the image a network saw to the volume it was made from.
///
/// To find the voxel of the 1 mm grid that the image's voxel `[x, y, z]` shows: take
/// `padding_before` from the three numbers and add `cut_from`. That is a place in the turned
/// grid, whose side along its axis `a` is `grid_dims[order[a]]` voxels long. The number along
/// axis `a` is then the place along the grid's own axis `order[a]`, counted from the far end
/// of that axis when `turned_round[a]` is set.
#[derive(Debug, Clone, PartialEq)]
pub struct Back {
    /// For each of the image's axes (x to the patient's right, y to the front, z to the
    /// top), the axis of the 1 mm grid it runs along: 0 for columns, 1 for rows, 2 for
    /// slices.
    pub order: [usize; 3],
    /// For each of the image's axes, whether it runs the other way from the grid's axis it
    /// was taken from.
    pub turned_round: [bool; 3],
    /// How many voxels of zeros lie before the image along x, y and z.
    pub padding_before: [usize; 3],
    /// The image's size along x, y and z before it was padded: the turned grid's, or the
    /// cut's when it was cut.
    pub size: [usize; 3],
    /// Where the cut began in the turned grid, along x, y and z. All zero when the whole
    /// grid was taken.
    pub cut_from: [usize; 3],
    /// The 1 mm grid's columns, rows and slices, in the volume's own axes: the volume's own
    /// when it needed no resampling.
    pub grid_dims: [u32; 3],
    /// Where each voxel of the 1 mm grid lies in the patient.
    pub grid_placement: Affine<Voxel, Patient>,
}

/// A volume made ready for the networks.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    /// What a network is given: `[1, x, y, z, 1]`, z fastest, every value from 0 to 1.
    pub image: Tensor,
    /// How to come back from the image to the volume.
    pub back: Back,
    /// The distance in millimetres between neighbouring voxels of the volume as it was
    /// given, along its columns, rows and slices. The image itself is on the 1 mm grid.
    pub spacing_mm: [f64; 3],
}

/// Why a volume cannot be prepared. Each says so in words a person can be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PrepareError {
    /// A side is under four voxels, as given or at 1 mm.
    #[error("This series has too few slices to label: it needs at least four along every side.")]
    TooFewSlices,
    /// The two percentiles are equal: all but a few values are the same, or none is a number.
    #[error("This series holds no values that differ, so there is nothing in it to label.")]
    NothingToSee,
    /// At 1 mm a side would be over 512 voxels.
    #[error("This series is too large to label: at 1 mm it would be over 512 voxels along a side.")]
    TooLarge,
}

/// Makes a volume ready for `SynthSeg`'s networks, step for step as `SynthSeg`'s own code does.
///
/// Too few slices and too large are decided from the sizes, before any value is read. A
/// value that is not finite counts as the smallest one that is.
///
/// `Cut::Head` is not made yet: for now it gives the whole grid, as `Cut::Whole` does, and
/// `Back::cut_from` is all zeros.
///
/// # Errors
///
/// [`PrepareError::TooFewSlices`] when a side has under four voxels, as given or at 1 mm.
/// [`PrepareError::TooLarge`] when a side would have over 512 at 1 mm.
/// [`PrepareError::NothingToSee`] when the two percentiles are equal.
pub fn prepare(volume: &Volume, cut: Cut) -> Result<Prepared, PrepareError> {
    if volume.dims().iter().any(|&side| side < FEWEST) {
        return Err(PrepareError::TooFewSlices);
    }
    let sides = resample::sides(volume);
    if sides.iter().any(|&side| side > LONGEST) {
        return Err(PrepareError::TooLarge);
    }
    if sides.iter().any(|&side| side < FEWEST) {
        return Err(PrepareError::TooFewSlices);
    }

    let finite = all_finite(volume).ok_or(PrepareError::NothingToSee)?;
    let grid = resample::one_mm(&finite);
    let turn = Turn::of(&grid);
    let turned = turn.turned(&grid);
    // The cut to the head goes here: after the turn, before the scaling.
    let cut_from = match cut {
        Cut::Whole | Cut::Head => [0; 3],
    };
    let (low, high) = percentiles(&turned.values)
        .filter(|(low, high)| low < high)
        .ok_or(PrepareError::NothingToSee)?;
    let (image, padding_before) = scaled_and_padded(&turned, low, high);
    Ok(Prepared {
        image,
        back: Back {
            order: turn.order,
            turned_round: turn.turned_round,
            padding_before,
            size: turned.size,
            cut_from,
            grid_dims: grid.dims(),
            grid_placement: grid.voxel_to_patient(),
        },
        spacing_mm: volume.spacing_mm(),
    })
}

/// The volume at 1 mm along its own axes, resampled as `SynthSeg` resamples it.
///
/// It is the same volume when every spacing is within 0.05 mm of 1 mm: `SynthSeg` leaves such a
/// volume alone. It is also the same volume when [`prepare`] would refuse it for its size (a
/// side of its 1 mm grid under 4 voxels or over 512), since there is then no grid to give.
/// Values that are not finite are not mended here; `prepare` does that first.
pub fn resampled(volume: &Volume) -> Volume {
    resample::one_mm(volume).into_owned()
}

/// The volume with every value that is not finite put at the smallest that is. `None` when no
/// value is finite.
fn all_finite(volume: &Volume) -> Option<Cow<'_, Volume>> {
    let values = volume.values();
    if values.iter().all(|value| value.is_finite()) {
        return Some(Cow::Borrowed(volume));
    }
    let finite = values.iter().copied().filter(|value| value.is_finite());
    let smallest = finite.min_by(f32::total_cmp)?;
    let mended = values
        .iter()
        .map(|&value| if value.is_finite() { value } else { smallest });
    Volume::new(volume.dims(), mended.collect(), volume.voxel_to_patient()).map(Cow::Owned)
}

/// The 0.5th and 99.5th percentiles of the values; `None` when there are none.
fn percentiles(values: &[f32]) -> Option<(f64, f64)> {
    let mut sorted = values.to_vec();
    let low = percentile(&mut sorted, LOW_PERCENT)?;
    let high = percentile(&mut sorted, HIGH_PERCENT)?;
    Some((low, high))
}

/// One percentile, as `NumPy`'s `percentile` works it out. With the values in order, it lies
/// `(count - 1) x percent / 100` places along them, and is blended from the value below that
/// place and the one above. Puts `values` partly in order; `None` when there are none.
fn percentile(values: &mut [f32], percent: f64) -> Option<f64> {
    let last = values.len().checked_sub(1)?;
    let place = last as f64 * (percent / 100.0);
    let below = whole(place.floor()).min(last);
    let (_, &mut under, above) = values.select_nth_unstable_by(below, f32::total_cmp);
    let over = above
        .iter()
        .copied()
        .min_by(f32::total_cmp)
        .unwrap_or(under);
    let (under, over) = (f64::from(under), f64::from(over));
    let along = place - below as f64;
    // NumPy blends from whichever end is nearer.
    let rise = over - under;
    Some(if along < 0.5 {
        under + rise * along
    } else {
        over - rise * (1.0 - along)
    })
}

/// The image a network is given, and how many voxels of zeros lie before it along each axis.
/// Each value is held between `low` and `high` and spread from 0 to 1; `low` must be the
/// smaller, which `prepare` has seen to. Each side is padded to the next multiple of 32, and
/// at least 128: half the difference before, rounded down, the rest after.
fn scaled_and_padded(turned: &Turned, low: f64, high: f64) -> (Tensor, [usize; 3]) {
    let [_, size_y, size_z] = turned.size;
    let padded = |side: usize| side.next_multiple_of(MULTIPLE).max(SMALLEST);
    let [full_x, full_y, full_z] = turned.size.map(padded);
    let before = turned.size.map(|side| (padded(side) - side) / 2);
    let [before_x, before_y, before_z] = before;
    let span = high - low;
    let mut numbers = vec![0.0_f32; full_x * full_y * full_z];
    // One line along z at a time: the lines come in order of x, then y.
    for (line, values) in turned.values.chunks_exact(size_z.max(1)).enumerate() {
        let (x, y) = (line / size_y.max(1), line % size_y.max(1));
        let start = ((x + before_x) * full_y + y + before_y) * full_z + before_z;
        let Some(places) = numbers.get_mut(start..start + size_z) else {
            continue;
        };
        for (place, &value) in places.iter_mut().zip(values) {
            *place = narrowed((f64::from(value).clamp(low, high) - low) / span);
        }
    }
    let image = Tensor {
        shape: vec![1, full_x, full_y, full_z, 1],
        numbers: Numbers::F32(numbers),
    };
    (image, before)
}

/// A number worked out in double precision, as the single precision values are kept in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a blend of f32 values, or a part of 1"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// A count worked out as a real number, as a whole one: nothing for a count below zero or
/// one that is no number, and the largest there is for one too large to hold.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the cast keeps a number within what a count can hold"
)]
fn whole(count: f64) -> usize {
    count as usize
}
