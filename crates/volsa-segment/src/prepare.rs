// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `preprocess` in SynthSeg/predict_synthseg.py, with the lab2im functions it
// calls, translated to Rust and changed for Volsa.
//! Turning one of Volsa's volumes into the image `SynthSeg`'s networks see, in `SynthSeg`'s own
//! steps and order, and remembering how to come back:
//!
//! 1. *Resample* to 1 mm along the volume's own axes, when any spacing is more than 0.05 mm
//!    from 1 mm (the module `resample`).
//! 2. *Turn* the axes, without resampling, so that x runs to the patient's right, y to the
//!    front and z to the top (the module `align`).
//! 3. *Cut* to the head, when asked. This is Volsa's own step, in the place where `SynthSeg`
//!    crops to a fixed size (the module `head`).
//! 4. *Scale* the brightness: the 0.5th percentile becomes 0 and the 99.5th becomes 1, and
//!    everything outside them is held at 0 or 1.
//! 5. *Pad* each side with zeros to the next multiple of 32, and at least 128.
//!
//! The steps follow `preprocess` in `SynthSeg`'s `predict_synthseg.py`, number for number.
//!
//! Where that program stops, Volsa refuses. `SynthSeg` takes a last side of ten or fewer for
//! the channels of a flat picture, so it stops on a stack of ten or fewer slices as stored,
//! and on one with ten or fewer voxels from bottom to top once it is at 1 mm and turned.
//! Neither can be given an image that is `SynthSeg`'s, so neither is given one.

use std::borrow::Cow;

use volsa_core::{Affine, Grid, Patient, Volume, Voxel};
use volsa_models::{Cut, Numbers, Tensor};

mod align;
mod head;
#[cfg(test)]
mod head_tests;
mod resample;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod thin_tests;

use align::{Turn, Turned};
pub use head::{Sizes, sizes};

/// A volume needs this many voxels along every side, as given and at 1 mm. This floor is
/// Volsa's own: `SynthSeg` has none for a side that is not the last.
const FEWEST: u32 = 4;
/// A volume needs this many slices as it is stored, and this many voxels from bottom to top
/// once it is at 1 mm and turned. Below it `SynthSeg`'s own program stops.
const FEWEST_SLICES: u32 = 11;
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
    /// grid was taken. A voxel outside the cut is not in the image, so the networks give it
    /// no label: it comes back as 0.
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
    /// The volume is too thin: under eleven slices as stored, under eleven voxels from
    /// bottom to top at 1 mm, or under four along any side, as stored or at 1 mm.
    #[error(
        "This series is too thin to label. It needs at least eleven slices and at least four \
         voxels along every side as stored. Counted at 1 mm, it needs at least eleven voxels \
         from bottom to top and at least four along every side."
    )]
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
/// Too thin and too large are decided from the sizes and the transform, before any value is
/// read. A value that is not finite counts as the smallest one that is.
///
/// `Cut::Head` keeps only the part of the turned grid that holds the head, when that is
/// smaller than the grid: the smallest box round the largest bright piece, with 16 voxels
/// more on every side. `Back::cut_from` says where the box began. Nothing bright, or a head
/// whose box fills the grid, gives the whole grid and a `cut_from` of zeros. The brightness is
/// scaled by the percentiles of the cut and not of the whole grid, as `SynthSeg` scales what it
/// has cropped.
///
/// # Errors
///
/// [`PrepareError::TooFewSlices`] when the volume has under eleven slices as stored, or under
/// eleven voxels from bottom to top at 1 mm, or under four along any side, as stored or at
/// 1 mm.
/// [`PrepareError::TooLarge`] when a side would have over 512 at 1 mm.
/// [`PrepareError::NothingToSee`] when the two percentiles are equal, over the whole grid, or
/// over the cut when it is cut to the head.
pub fn prepare(volume: &Volume, cut: Cut) -> Result<Prepared, PrepareError> {
    let ready = make_ready(volume, cut)?;
    let (image, padding_before) = scaled_and_padded(&ready.cut, ready.low, ready.high);
    Ok(Prepared {
        image,
        back: Back {
            order: ready.turn.order,
            turned_round: ready.turn.turned_round,
            padding_before,
            size: ready.cut.size,
            cut_from: ready.cut_from,
            grid_dims: ready.grid_dims,
            grid_placement: ready.grid_placement,
        },
        spacing_mm: volume.spacing_mm(),
    })
}

/// A volume resampled, turned and cut: everything that comes before the scaling.
struct Ready {
    /// How the grid was turned.
    turn: Turn,
    /// The 1 mm grid's columns, rows and slices.
    grid_dims: [u32; 3],
    /// Where each voxel of the 1 mm grid lies in the patient.
    grid_placement: Affine<Voxel, Patient>,
    /// How many voxels the turned grid has along x, y and z, before any cut.
    whole: [usize; 3],
    /// Where the cut began in the turned grid.
    cut_from: [usize; 3],
    /// The values that remain after the cut.
    cut: Turned,
    /// The 0.5th percentile of the values of the cut, which `high` exceeds.
    low: f64,
    /// The 99.5th percentile of the values of the cut.
    high: f64,
}

/// The steps of [`prepare`] that come before the scaling, and the percentiles of the values
/// that remain, which the scaling needs. [`sizes`] takes the same steps, so it refuses what
/// `prepare` refuses.
fn make_ready(volume: &Volume, cut: Cut) -> Result<Ready, PrepareError> {
    let turn = turn_if_it_fits(volume)?;
    let finite = all_finite(volume).ok_or(PrepareError::NothingToSee)?;
    let grid = resample::one_mm(&finite);
    let turned = turn.turned(&grid);
    let whole = turned.size;
    let (low, high) = spread(&turned.values)?;
    let (cut, cut_from, low, high) = match cut {
        Cut::Whole => (turned, [0; 3], low, high),
        Cut::Head => {
            // The head is found by the whole grid's percentiles; the brightness is scaled by
            // the cut's own.
            let (head, cut_from) = head::cut_to_head(turned, low, high);
            let (low, high) = spread(&head.values)?;
            (head, cut_from, low, high)
        }
    };
    Ok(Ready {
        turn,
        grid_dims: grid.dims(),
        grid_placement: grid.voxel_to_patient(),
        whole,
        cut_from,
        cut,
        low,
        high,
    })
}

/// The 0.5th and 99.5th percentiles of the values, which must differ: the scaling divides by
/// their difference, and holds values between them. Nothing to see when they are equal.
fn spread(values: &[f32]) -> Result<(f64, f64), PrepareError> {
    percentiles(values)
        .filter(|(low, high)| low < high)
        .ok_or(PrepareError::NothingToSee)
}

/// Whether a scan is of a size that can be labelled, from its sizes and its transform alone:
/// no value is read, so it can be asked of a scan as it is held, before the 32-bit copy a run
/// is given is made.
///
/// # Errors
///
/// [`PrepareError::TooFewSlices`] and [`PrepareError::TooLarge`], exactly as [`prepare`] and
/// [`sizes`] give them for a volume of this size and place.
pub fn fits(scan: &(impl Grid + ?Sized)) -> Result<(), PrepareError> {
    turn_if_it_fits(scan).map(|_| ())
}

/// How the volume's 1 mm grid is turned, or why the volume is refused for its size. Decided
/// from the sizes and the transform alone: no value is read.
fn turn_if_it_fits(volume: &(impl Grid + ?Sized)) -> Result<Turn, PrepareError> {
    let [_, _, slices] = volume.dims();
    if slices < FEWEST_SLICES || volume.dims().iter().any(|&side| side < FEWEST) {
        return Err(PrepareError::TooFewSlices);
    }
    let sides = resample::sides(volume);
    if sides.iter().any(|&side| side > LONGEST) {
        return Err(PrepareError::TooLarge);
    }
    if sides.iter().any(|&side| side < FEWEST) {
        return Err(PrepareError::TooFewSlices);
    }
    // `SynthSeg` turns the grid it has at 1 mm, so it is that grid's transform that says
    // which side runs from bottom to top.
    let turn = Turn::of(volume, resample::steps(volume));
    if turn.tall(sides) < FEWEST_SLICES {
        return Err(PrepareError::TooFewSlices);
    }
    Ok(turn)
}

/// The volume at 1 mm along its own axes, resampled as `SynthSeg` resamples it.
///
/// It is the same volume when every spacing is within 0.05 mm of 1 mm: `SynthSeg` leaves such a
/// volume alone. It is also the same volume when its 1 mm grid would have a side under 4
/// voxels or over 512: there is then no grid to give, and [`prepare`] refuses such a volume.
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

/// A side of an image, padded to the next multiple of 32, and at least 128.
fn padded(side: usize) -> usize {
    side.next_multiple_of(MULTIPLE).max(SMALLEST)
}

/// How many voxels an image has when each side of a grid of this size is padded.
fn padded_voxels(size: [usize; 3]) -> u64 {
    let count = |side: usize| u64::try_from(padded(side)).unwrap_or(u64::MAX);
    size.into_iter().map(count).fold(1, u64::saturating_mul)
}

/// The image a network is given, and how many voxels of zeros lie before it along each axis.
/// Each value is held between `low` and `high` and spread from 0 to 1; `low` must be the
/// smaller, which `prepare` has seen to. Each side is padded to the next multiple of 32, and
/// at least 128: half the difference before, rounded down, the rest after.
fn scaled_and_padded(turned: &Turned, low: f64, high: f64) -> (Tensor, [usize; 3]) {
    let [_, size_y, size_z] = turned.size;
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
