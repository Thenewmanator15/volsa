//! The three-dimensional Fourier transform of a block of complex numbers, done one side at a
//! time on every core, and the frequencies each place of a transformed block stands for.

use rayon::prelude::*;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use super::SusceptibilityError;

/// Which way a transform goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Way {
    /// From places to frequencies.
    Forward,
    /// From frequencies back to places, divided by the count so that it undoes the other.
    Back,
}

/// Transform `data`, a block of `sides` (columns, rows, slices) numbers, columns fastest, in
/// place.
///
/// # Errors
///
/// [`SusceptibilityError::NoMemory`] when there is not the room for the copy that the slices
/// are transformed in.
pub(super) fn transform(
    data: &mut [Complex32],
    sides: [usize; 3],
    way: Way,
) -> Result<(), SusceptibilityError> {
    let [columns, rows, slices] = sides;
    let mut planner = FftPlanner::<f32>::new();
    let mut plan = |length| match way {
        Way::Forward => planner.plan_fft_forward(length),
        Way::Back => planner.plan_fft_inverse(length),
    };
    let (along_columns, along_rows, along_slices) = (plan(columns), plan(rows), plan(slices));
    data.par_chunks_mut(columns)
        .for_each(|line| along_columns.process(line));
    data.par_chunks_mut(columns * rows)
        .for_each(|plane| along_strided(plane, columns, &*along_rows));
    across_slices(data, sides, &*along_slices)?;
    if way == Way::Back {
        let scale = 1.0 / data.len() as f32;
        data.par_iter_mut().for_each(|value| *value *= scale);
    }
    Ok(())
}

/// Transform each of the `stride` interleaved lines of `block`: line `n` is every
/// `stride`th number from the `n`th.
fn along_strided(block: &mut [Complex32], stride: usize, plan: &dyn Fft<f32>) {
    let mut line = vec![Complex32::default(); block.len() / stride.max(1)];
    for first in 0..stride {
        let numbers = block.iter().skip(first).step_by(stride);
        line.iter_mut()
            .zip(numbers)
            .for_each(|(into, from)| *into = *from);
        plan.process(&mut line);
        let numbers = block.iter_mut().skip(first).step_by(stride);
        numbers.zip(&line).for_each(|(into, from)| *into = *from);
    }
}

/// Transform along the slices: each column of slices is gathered into a copy where it lies
/// in a row, transformed there, and put back.
fn across_slices(
    data: &mut [Complex32],
    sides: [usize; 3],
    plan: &dyn Fft<f32>,
) -> Result<(), SusceptibilityError> {
    let [columns, rows, slices] = sides;
    let plane = columns * rows;
    let mut gathered = super::zeroed::<Complex32>(data.len())?;
    let source: &[Complex32] = data;
    gathered
        .par_chunks_mut(slices)
        .enumerate()
        .for_each(|(place, line)| {
            let numbers = source.iter().skip(place).step_by(plane);
            line.iter_mut()
                .zip(numbers)
                .for_each(|(into, from)| *into = *from);
            plan.process(line);
        });
    data.par_chunks_mut(plane)
        .enumerate()
        .for_each(|(slice, out)| {
            let numbers = gathered.iter().skip(slice).step_by(slices);
            out.iter_mut()
                .zip(numbers)
                .for_each(|(into, from)| *into = *from);
        });
    Ok(())
}

/// The frequency, in cycles per millimetre, that place `at` of a transformed side `length`
/// places long, `step` millimetres apart, stands for.
pub(super) fn frequency(at: usize, length: usize, step: f64) -> f64 {
    let (at, length) = (at as f64, length as f64);
    let signed = if at < length / 2.0 { at } else { at - length };
    signed / (length * step)
}

/// The first length at or past `length` whose only prime factors are 2, 3 and 5, which the
/// transform takes fastest.
pub(super) fn smooth_length(length: usize) -> usize {
    (length.max(1)..=length.max(1) * 2)
        .find(|&candidate| {
            let mut left = candidate;
            for prime in [2, 3, 5] {
                while left % prime == 0 {
                    left /= prime;
                }
            }
            left == 1
        })
        .unwrap_or(length)
}
