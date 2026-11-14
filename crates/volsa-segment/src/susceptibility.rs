//! An approximate map of magnetic susceptibility worked out from the phase of a
//! susceptibility-weighted (SWI) or T2* scan: quantitative susceptibility mapping, or QSM.
//! Tissue that is drawn into a magnetic field (veins, which hold deoxygenated blood, and
//! nuclei rich in iron) bends the field round it a little, which shows in the phase. The phase
//! is unwrapped, the field from outside the brain is taken away, and what is left is turned
//! back into the susceptibility that would make it (see the `steps` module).
//!
//! The map is approximate: it is worked out on a grid of about a millimetre, inside a head
//! found from the phase alone, with the simplest of the ways to turn field into
//! susceptibility, and from one echo. A phase that the scanner has already filtered, as many
//! do for SWI, has lost part of the field, so the numbers come out smaller than they are.
//! Scanners do not agree on which way phase turns: the map is turned so that its strongest
//! values are positive, as veins and iron are.

use rayon::prelude::*;
use volsa_core::Volume;

mod fft;
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid sizes and places are small whole numbers, and values in parts per billion fit an f32"
)]
mod grid;
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "widths in voxels are small whole numbers, and means of f32 values fit an f32"
)]
mod mask;
#[expect(
    clippy::cast_possible_truncation,
    reason = "the factors multiplied in are well within an f32"
)]
mod steps;
#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "made-up test volumes are small"
)]
mod tests;

/// What the map is called.
pub const SUSCEPTIBILITY_TITLE: &str = "Susceptibility map (approximate)";
/// How wide a window shows the map well, in parts per billion either side of 0.
pub const SUSCEPTIBILITY_WINDOW_PPB: f32 = 150.0;

/// The most voxels the working grid may have: about 330 MB of working memory at most.
const MOST_WORK_VOXELS: usize = 16_000_000;
/// The fewest slices the map can be worked out from.
const FEWEST_SLICES: u32 = 8;
/// The gyromagnetic ratio of hydrogen, in radians per second per tesla.
const GAMMA: f64 = 2.0 * std::f64::consts::PI * 42.577_478e6;
/// The share of the strongest values, at each end, that is compared to choose the map's sign.
const SIGN_SHARE: f64 = 0.002;

/// What the map needs to know of the scan, from its header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseFacts {
    /// The echo time, in milliseconds.
    pub echo_ms: Option<f64>,
    /// The strength of the scanner's field, in tesla.
    pub field_t: Option<f64>,
}

/// Why the map could not be worked out. Each says so in a sentence for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SusceptibilityError {
    /// It was stopped.
    #[error("The susceptibility map was stopped part-way.")]
    Stopped,
    /// There was not the memory.
    #[error("There is not the memory to work out a susceptibility map of this scan.")]
    NoMemory,
    /// The scan is too large.
    #[error("This scan is too large to work out a susceptibility map of.")]
    TooLarge,
    /// The scan has too few slices.
    #[error("A susceptibility map needs a 3D scan of at least 8 slices.")]
    TooThin,
    /// The header gives no echo time or field strength.
    #[error("This scan does not say its echo time and field strength, which the map needs.")]
    NoFacts,
    /// The values are not phase in any way Volsa knows.
    #[error("This scan's values do not look like phase. Choose the phase series of an SWI scan.")]
    NotPhase,
    /// No head was found.
    #[error("No head was found in this phase. Choose the phase series of an SWI scan.")]
    NoHead,
}

/// The susceptibility map of `phase`, the phase series of an SWI or T2* scan, on its own
/// grid, in parts per billion; 0 outside the brain. `stop` is asked between steps.
///
/// # Errors
///
/// A [`SusceptibilityError`] saying why not.
pub fn susceptibility(
    phase: &Volume,
    facts: PhaseFacts,
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<Volume, SusceptibilityError> {
    let stopped = || {
        if stop() {
            Err(SusceptibilityError::Stopped)
        } else {
            Ok(())
        }
    };
    let dims = phase.dims();
    if dims[2] < FEWEST_SLICES || dims[0] < FEWEST_SLICES || dims[1] < FEWEST_SLICES {
        return Err(SusceptibilityError::TooThin);
    }
    let (Some(echo_ms), Some(field_t)) = (facts.echo_ms, facts.field_t) else {
        return Err(SusceptibilityError::NoFacts);
    };
    if !(echo_ms > 0.0 && field_t > 0.0) {
        return Err(SusceptibilityError::NoFacts);
    }
    let radians = to_radians(phase.values())?;
    let axes = phase.voxel_to_patient().columns();
    let spacing = axes.map(|axis| axis.iter().map(|part| part * part).sum::<f64>().sqrt());
    let work = grid::Work::for_scan(dims.map(|side| side as usize), spacing);
    if work.count() > MOST_WORK_VOXELS {
        return Err(SusceptibilityError::TooLarge);
    }
    let (angles, blank) = work.take_in(phase.values().iter().map(|value| radians(*value)))?;
    stopped()?;
    let (_, inner) = mask::head(&angles, &blank, &work)?;
    drop(blank);
    stopped()?;
    let unwrapped = steps::unwrapped(&angles, &work)?;
    drop(angles);
    stopped()?;
    let field = steps::brains_own(&unwrapped, &inner, &work)?;
    drop(unwrapped);
    stopped()?;
    let along = main_field_along(axes, spacing);
    let mut map = steps::from_field(&field, &inner, &work, along)?;
    drop(field);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the factor from field to parts per billion is a few hundred"
    )]
    let to_ppb = (1e9 / (GAMMA * field_t * echo_ms / 1000.0)) as f32;
    map.par_iter_mut().for_each(|value| *value *= to_ppb);
    if strongest_are_negative(&map, &inner) {
        map.par_iter_mut().for_each(|value| *value = -*value);
    }
    stopped()?;
    let values = work.give_back(&map)?;
    Volume::new(dims, values, phase.voxel_to_patient()).ok_or(SusceptibilityError::NoMemory)
}

/// How a value of `values` is turned into radians, by how large they are: radians as they
/// are, thousandths of a radian (as GE writes its filtered phase), or 12-bit numbers from 0
/// to 4095 that stand for a whole turn (as Siemens writes phase).
fn to_radians(values: &[f32]) -> Result<impl Fn(f32) -> f32 + use<>, SusceptibilityError> {
    let step = (values.len() / 1_000_000).max(1);
    let mut sizes: Vec<f32> = values
        .iter()
        .step_by(step)
        .map(|value| value.abs())
        .collect();
    sizes.sort_by(f32::total_cmp);
    let near_top = sizes.get(sizes.len() * 999 / 1000).copied().unwrap_or(0.0);
    let least = values.iter().copied().fold(f32::INFINITY, f32::min);
    let most = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let pi = std::f32::consts::PI;
    let (offset, scale) = if least >= 0.0 && most <= 4095.0 && near_top > 3300.0 {
        (2048.0, pi / 2048.0)
    } else if (800.0..=3300.0).contains(&near_top) {
        (0.0, 1.0 / 1000.0)
    } else if (0.2..=3.3).contains(&near_top) {
        (0.0, 1.0)
    } else {
        return Err(SusceptibilityError::NotPhase);
    };
    Ok(move |value: f32| (value - offset) * scale)
}

/// Which way the scanner's main field runs, along the voxels' sides as millimetres: the
/// patient's head-to-foot axis, along which a patient lies in the bore.
fn main_field_along(axes: [[f64; 3]; 3], spacing: [f64; 3]) -> [f64; 3] {
    let along = [0, 1, 2].map(|axis| {
        let (Some(direction), Some(length)) = (axes.get(axis), spacing.get(axis)) else {
            return 0.0;
        };
        direction.get(2).copied().unwrap_or(0.0) / length.max(f64::MIN_POSITIVE)
    });
    let length = along.iter().map(|part| part * part).sum::<f64>().sqrt();
    if length > 0.0 {
        along.map(|part| part / length)
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Whether, inside `inner`, the strongest negative values of `map` outweigh the strongest
/// positive ones.
fn strongest_are_negative(map: &[f32], inner: &[bool]) -> bool {
    let mut inside: Vec<f32> = map
        .iter()
        .zip(inner)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    if inside.is_empty() {
        return false;
    }
    inside.sort_by(f32::total_cmp);
    let last = inside.len() - 1;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a share of the count of values is a place among them"
    )]
    let at = |share: f64| {
        inside
            .get((share * last as f64) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    at(1.0 - SIGN_SHARE) < -at(SIGN_SHARE)
}

/// A list of `len` zeros, or [`SusceptibilityError::NoMemory`] when there is not the room.
fn zeroed<T: Clone + Default>(len: usize) -> Result<Vec<T>, SusceptibilityError> {
    let mut list = Vec::new();
    list.try_reserve_exact(len)
        .map_err(|_| SusceptibilityError::NoMemory)?;
    list.resize(len, T::default());
    Ok(list)
}
