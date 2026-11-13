//! Highlighting thin dark tubes in a scan: on a susceptibility-weighted (SWI) or T2* scan these
//! are mostly veins. No trained model is used. The scan is smoothed at a few widths, and at
//! each voxel the way its brightness curves is measured (the eigenvalues of its Hessian): a
//! voxel darker than its neighbours across two directions and not along the third lies in a
//! dark tube (Frangi and others, 1998, "Multiscale vessel enhancement filtering", with the
//! signs turned for dark tubes). The voxels that score highly inside the head are kept, and
//! pieces too small to be a stretch of tube are dropped.
//!
//! What it finds is shown as labels with one part, said to be approximate: it is not every
//! vein, and not everything it finds is one.

use volsa_core::Volume;
use volsa_labels::{Entry, LabelGrid, MAX_LABEL_VOXELS, Side, Table, Trust};

mod head;
mod onto;
mod pieces;
mod smooth;
#[cfg(test)]
mod tests;
mod tube;

pub use onto::veins_onto;

/// What the labels are of.
pub const VEINS_TITLE: &str = "Dark tube-like structures";
/// The name of the one part.
pub const VEINS_PART: &str = "Dark tubes (mostly veins)";
/// How the part was made, as the Labels tab says it.
pub const VEINS_HOW: &str = "Found by a filter that looks for thin dark tubes, not by a trained \
                             model. On an SWI or T2* scan most of them are veins; some are not, \
                             and not every vein is found.";
/// The group the part is listed under.
pub const VEINS_GROUP: &str = "Highlighted";
/// The colour the part is drawn in: a clear blue, as veins are often drawn.
pub const VEINS_COLOUR: [u8; 3] = [64, 156, 255];

/// The widths the scan is smoothed at before its curvature is measured, in millimetres: the
/// tubes found are about two to four times as wide.
const WIDTHS_MM: [f64; 2] = [0.7, 1.4];
/// How strongly a tube must curve to count fully, as a share of the scan's bright level.
const CURVE_SHARE: f32 = 0.08;
/// The least score, out of 255, of a voxel kept.
const LEAST_SCORE: u8 = 56;
/// The smallest piece kept, in cubic millimetres.
const SMALLEST_PIECE_MM3: f64 = 12.0;
/// The share of the scan's values below its bright level.
const BRIGHT_SHARE: f64 = 0.99;

/// Why no tubes could be looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VeinsError {
    /// It was stopped.
    #[error("Highlighting stopped part-way.")]
    Stopped,
    /// There was not the memory for it.
    #[error("There is not the memory on this computer to highlight this scan's dark tubes now.")]
    NoMemory,
    /// The scan is larger than Volsa holds as labels.
    #[error("This scan has more voxels than Volsa will hold as labels.")]
    TooLarge,
    /// The scan has too few slices, or too few voxels in a slice, to find a tube in.
    #[error("This scan has too few slices to look for tubes in: it needs a 3D series.")]
    TooThin,
    /// Nothing in it is bright, so nothing in it can be told apart from the dark.
    #[error("This scan holds nothing bright, so nothing in it can be told from the dark.")]
    Blank,
    /// No thin dark tubes were found.
    #[error("No thin dark tubes were found on this scan.")]
    NoneFound,
}

/// The voxels of `volume` that lie in thin dark tubes, as a label volume on its grid: 1 for a
/// tube, 0 for anything else. `stop` is asked between steps; once it says yes, the work ends.
///
/// # Errors
///
/// A [`VeinsError`] when it is stopped, there is not the memory, or the scan cannot be looked
/// at for tubes.
pub fn veins(volume: &Volume, stop: &(dyn Fn() -> bool + Sync)) -> Result<LabelGrid, VeinsError> {
    let dims = volume.dims();
    let sides = dims.map(|side| usize::try_from(side).unwrap_or(usize::MAX));
    if sides.iter().any(|side| *side < 3) {
        return Err(VeinsError::TooThin);
    }
    let voxels = sides
        .iter()
        .try_fold(1_usize, |all, side| all.checked_mul(*side));
    if voxels.is_none_or(|voxels| voxels as u64 > MAX_LABEL_VOXELS) {
        return Err(VeinsError::TooLarge);
    }
    let values = volume.values();
    let spacing = volume.spacing_mm();
    let bright = bright_level(values).ok_or(VeinsError::Blank)?;
    let inside = head::inside(values, sides, spacing, bright)?;
    let stopped = || {
        if stop() {
            Err(VeinsError::Stopped)
        } else {
            Ok(())
        }
    };
    stopped()?;
    let mut score = zeroed::<u8>(values.len())?;
    for width in WIDTHS_MM {
        let smoothed = smooth::smoothed(values, sides, spacing, width)?;
        stopped()?;
        let curve = CURVE_SHARE * bright;
        tube::dark_tubes(&smoothed, sides, spacing, width, curve, &mut score);
        stopped()?;
    }
    let mut kept: Vec<u8> = score
        .iter()
        .zip(&inside)
        .map(|(score, inside)| u8::from(*inside && *score >= LEAST_SCORE))
        .collect();
    drop(score);
    let voxel_mm3 = spacing.iter().product::<f64>();
    let smallest = SMALLEST_PIECE_MM3 / voxel_mm3.max(f64::MIN_POSITIVE);
    pieces::drop_small(&mut kept, sides, smallest)?;
    stopped()?;
    if !kept.contains(&1) {
        return Err(VeinsError::NoneFound);
    }
    Ok(LabelGrid {
        dims,
        ids: kept.into_iter().map(u32::from).collect(),
        placement: volume.voxel_to_patient(),
    })
}

/// The table that names what [`veins`] finds: one part, approximate.
pub fn veins_table() -> Table {
    Table {
        title: VEINS_TITLE.to_owned(),
        entries: vec![Entry {
            name: VEINS_PART.to_owned(),
            group: VEINS_GROUP.to_owned(),
            side: Side::Midline,
            ids: vec![1],
            colour: VEINS_COLOUR,
            trust: Trust::Approximate(VEINS_HOW.to_owned()),
        }],
        verdict: None,
        made: None,
    }
}

/// The scan's bright level: the value that all but the brightest hundredth lie below, from at
/// most a million values spread through it. `None` when nothing is above zero.
fn bright_level(values: &[f32]) -> Option<f32> {
    let step = (values.len() >> 20).max(1);
    let mut sample: Vec<f32> = values
        .iter()
        .step_by(step)
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect();
    let last = sample.len().checked_sub(1)?;
    let at = whole(last as f64 * BRIGHT_SHARE).min(last);
    let (_, level, _) = sample.select_nth_unstable_by(at, f32::total_cmp);
    (*level > 0.0).then_some(*level)
}

/// A count from a number that is never below zero, rounded down.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the number is a count of voxels, from zero up and far below the largest usize"
)]
pub(crate) fn whole(number: f64) -> usize {
    number.max(0.0) as usize
}

/// A list of `len` zeros, or [`VeinsError::NoMemory`] when there is not the room for it.
pub(crate) fn zeroed<T: Clone + Default>(len: usize) -> Result<Vec<T>, VeinsError> {
    let mut list = Vec::new();
    list.try_reserve_exact(len)
        .map_err(|_| VeinsError::NoMemory)?;
    list.resize(len, T::default());
    Ok(list)
}
