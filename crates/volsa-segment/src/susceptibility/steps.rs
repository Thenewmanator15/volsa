//! The three steps from phase to susceptibility, each worked in frequencies:
//!
//! 1. Unwrapping: the phase is only known round a turn, so it jumps where it passes a half
//!    turn. Its Laplacian can be had without those jumps, from its sine and cosine, and the
//!    Laplacian undone (Schofield and Zhu, 2003, "Fast phase unwrapping algorithm for
//!    interferometric applications").
//! 2. Taking away the field from outside the brain: within the brain that field has no
//!    sources, so it equals its own mean over any sphere; taking away the mean over a sphere
//!    leaves what the brain itself makes, which is then undone where it can be (SHARP:
//!    Schweser and others, 2011).
//! 3. From field to susceptibility: a small magnet makes a field shaped like the dipole kernel,
//!    so the field is divided by that kernel, which is cut short where it is near 0 so that
//!    noise is not blown up (thresholded k-space division: Shmueli and others, 2009).

use rayon::prelude::*;
use rustfft::num_complex::Complex32;

use super::SusceptibilityError;
use super::fft::{Way, frequency, transform};
use super::grid::Work;

/// The radius of the sphere the outside field is averaged over, in millimetres.
const SPHERE_MM: f64 = 5.0;
/// Below this, the sphere's undoing is not done.
const SPHERE_CUT: f64 = 0.05;
/// The dipole kernel is held at no less than this, in size, where it is divided by.
const DIPOLE_CUT: f64 = 0.19;

/// The phase, in radians, without the jumps of a whole turn, up to a smooth field that the
/// next step takes away.
///
/// # Errors
///
/// [`SusceptibilityError::NoMemory`] when there is not the room.
pub(super) fn unwrapped(phase: &[f32], work: &Work) -> Result<Vec<f32>, SusceptibilityError> {
    let laplacian = |values: &mut Vec<Complex32>| {
        transform(values, work.sides, Way::Forward)?;
        apply(values, work, |k| {
            -4.0 * std::f64::consts::PI.powi(2) * dot(k, k)
        });
        transform(values, work.sides, Way::Back)
    };
    let mut sines: Vec<Complex32> = phase.par_iter().map(|a| a.sin().into()).collect();
    let mut cosines: Vec<Complex32> = phase.par_iter().map(|a| a.cos().into()).collect();
    laplacian(&mut sines)?;
    laplacian(&mut cosines)?;
    let mut whole: Vec<Complex32> = phase
        .par_iter()
        .zip(&sines)
        .zip(&cosines)
        .map(|((angle, sine), cosine)| (angle.cos() * sine.re - angle.sin() * cosine.re).into())
        .collect();
    drop((sines, cosines));
    transform(&mut whole, work.sides, Way::Forward)?;
    apply(&mut whole, work, |k| {
        let squared = dot(k, k);
        if squared > 0.0 {
            -1.0 / (4.0 * std::f64::consts::PI.powi(2) * squared)
        } else {
            0.0
        }
    });
    transform(&mut whole, work.sides, Way::Back)?;
    Ok(whole.par_iter().map(|value| value.re).collect())
}

/// The part of `phase` that the brain itself makes, inside `inner`.
///
/// # Errors
///
/// [`SusceptibilityError::NoMemory`] when there is not the room.
pub(super) fn brains_own(
    phase: &[f32],
    inner: &[bool],
    work: &Work,
) -> Result<Vec<f32>, SusceptibilityError> {
    let sphere = sphere_mean(work)?;
    let less_mean = |at: usize| 1.0 - f64::from(sphere.get(at).copied().unwrap_or(0.0));
    let mut values: Vec<Complex32> = phase.par_iter().map(|value| (*value).into()).collect();
    transform(&mut values, work.sides, Way::Forward)?;
    multiply_each(&mut values, less_mean);
    transform(&mut values, work.sides, Way::Back)?;
    keep_inside(&mut values, inner);
    transform(&mut values, work.sides, Way::Forward)?;
    multiply_each(&mut values, |at| {
        let factor = less_mean(at);
        if factor.abs() > SPHERE_CUT {
            1.0 / factor
        } else {
            0.0
        }
    });
    transform(&mut values, work.sides, Way::Back)?;
    keep_inside(&mut values, inner);
    Ok(values.par_iter().map(|value| value.re).collect())
}

/// The susceptibility, in the field's own units, inside `inner`, that makes `field`, with
/// the main field along `along` (a direction of length 1 on the working grid's sides).
///
/// # Errors
///
/// [`SusceptibilityError::NoMemory`] when there is not the room.
pub(super) fn from_field(
    field: &[f32],
    inner: &[bool],
    work: &Work,
    along: [f64; 3],
) -> Result<Vec<f32>, SusceptibilityError> {
    let mut values: Vec<Complex32> = field.par_iter().map(|value| (*value).into()).collect();
    transform(&mut values, work.sides, Way::Forward)?;
    apply(&mut values, work, |k| {
        let squared = dot(k, k);
        if squared == 0.0 {
            return 0.0;
        }
        let kernel = 1.0 / 3.0 - dot(k, along).powi(2) / squared;
        if kernel.abs() > DIPOLE_CUT {
            1.0 / kernel
        } else {
            kernel.signum() / DIPOLE_CUT
        }
    });
    transform(&mut values, work.sides, Way::Back)?;
    keep_inside(&mut values, inner);
    Ok(values.par_iter().map(|value| value.re).collect())
}

/// The mean over a sphere of [`SPHERE_MM`], as frequencies: the transform of a sphere,
/// centred on the first voxel and carried round the edges, whose voxels add up to 1.
fn sphere_mean(work: &Work) -> Result<Vec<f32>, SusceptibilityError> {
    let [columns, rows, slices] = work.sides;
    let [x_mm, y_mm, z_mm] = work.step;
    let from_first = |at: usize, side: usize, step: f64| {
        let near = at.min(side - at) as f64;
        near * step
    };
    let mut ball = super::zeroed::<Complex32>(work.count())?;
    ball.par_chunks_mut(columns)
        .enumerate()
        .for_each(|(line, out)| {
            let (row, slice) = (line % rows, line / rows);
            let (y, z) = (from_first(row, rows, y_mm), from_first(slice, slices, z_mm));
            for (column, value) in out.iter_mut().enumerate() {
                let x = from_first(column, columns, x_mm);
                if x * x + y * y + z * z <= SPHERE_MM * SPHERE_MM {
                    *value = 1.0.into();
                }
            }
        });
    let total: f32 = ball.par_iter().map(|value| value.re).sum();
    ball.par_iter_mut()
        .for_each(|value| *value /= total.max(1.0));
    transform(&mut ball, work.sides, Way::Forward)?;
    Ok(ball.par_iter().map(|value| value.re).collect())
}

/// Multiply each value of `values`, frequencies on `work`, by what `by` gives for its
/// frequency in cycles per millimetre along the working grid's sides.
fn apply(values: &mut [Complex32], work: &Work, by: impl Fn([f64; 3]) -> f64 + Sync) {
    let [columns, rows, slices] = work.sides;
    let [x_mm, y_mm, z_mm] = work.step;
    values
        .par_chunks_mut(columns)
        .enumerate()
        .for_each(|(line, out)| {
            let (row, slice) = (line % rows, line / rows);
            let (ky, kz) = (frequency(row, rows, y_mm), frequency(slice, slices, z_mm));
            for (column, value) in out.iter_mut().enumerate() {
                let kx = frequency(column, columns, x_mm);
                *value *= by([kx, ky, kz]) as f32;
            }
        });
}

/// Multiply each value of `values` by what `by` gives for its place.
fn multiply_each(values: &mut [Complex32], by: impl Fn(usize) -> f64 + Sync) {
    values
        .par_iter_mut()
        .enumerate()
        .for_each(|(at, value)| *value *= by(at) as f32);
}

/// Clear every value of `values` outside `inside`.
fn keep_inside(values: &mut [Complex32], inside: &[bool]) {
    values.par_iter_mut().zip(inside).for_each(|(value, kept)| {
        if !kept {
            *value = Complex32::default();
        }
    });
}

fn dot(one: [f64; 3], other: [f64; 3]) -> f64 {
    one.iter().zip(&other).map(|(a, b)| a * b).sum()
}
