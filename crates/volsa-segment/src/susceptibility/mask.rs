//! Where the head's tissue is, from the phase alone. Where there is tissue, the phase changes
//! smoothly from voxel to voxel, so the phases round a voxel, taken as turns, point much the
//! same way; outside, the phase is noise and they point every way, or the scanner has
//! blanked it to exactly 0. The largest piece of what is smooth and not blank, its holes
//! filled slice by slice, is taken as the head.

use rayon::prelude::*;

use super::SusceptibilityError;
use super::grid::Work;

/// How far round a voxel its neighbours are looked at, in millimetres.
const ROUND_MM: f64 = 2.0;
/// The most the phases round a voxel of tissue may disagree: 1 less the length of the mean
/// of their turns.
const MOST_DISAGREEMENT: f32 = 0.06;
/// The largest share of the voxels round one of tissue that may be blank.
const MOST_BLANK: f32 = 0.2;
/// How many times the mask is shrunk and grown again, to cut thin bridges to the scalp.
const OPENING: usize = 2;
/// The least a head may hold, in millilitres.
const LEAST_ML: f64 = 200.0;

/// The head's tissue on `work`, from the phase (`phase`, radians) and the share of blank
/// voxels in each working voxel (`blank`); and the same shrunk by [`ROUND_MM`], inside of
/// which the map is worked out.
///
/// # Errors
///
/// [`SusceptibilityError::NoHead`] when what is found is too small to be a head, and
/// [`SusceptibilityError::NoMemory`] when there is not the room.
pub(super) fn head(
    phase: &[f32],
    blank: &[f32],
    work: &Work,
) -> Result<(Vec<bool>, Vec<bool>), SusceptibilityError> {
    let half = work
        .step
        .map(|step| (ROUND_MM / step).round().max(1.0) as usize);
    let mut along = phase
        .par_iter()
        .map(|angle| angle.cos())
        .collect::<Vec<f32>>();
    let mut across = phase
        .par_iter()
        .map(|angle| angle.sin())
        .collect::<Vec<f32>>();
    box_mean(&mut along, work.sides, half)?;
    box_mean(&mut across, work.sides, half)?;
    let mut blank = blank.to_vec();
    box_mean(&mut blank, work.sides, half)?;
    let mut tissue: Vec<bool> = along
        .par_iter()
        .zip(&across)
        .zip(&blank)
        .map(|((x, y), empty)| 1.0 - x.hypot(*y) < MOST_DISAGREEMENT && *empty < MOST_BLANK)
        .collect();
    drop((along, across, blank));
    for _ in 0..OPENING {
        tissue = grown(&tissue, work.sides, false)?;
    }
    for _ in 0..OPENING {
        tissue = grown(&tissue, work.sides, true)?;
    }
    let [columns, rows, slices] = work.sides;
    let mut tissue = crate::largest_piece(&tissue, [slices, rows, columns]);
    fill_holes(&mut tissue, work.sides);
    let voxel_ml = work.step.iter().product::<f64>() / 1000.0;
    let ml = tissue.iter().filter(|set| **set).count() as f64 * voxel_ml;
    if ml < LEAST_ML {
        return Err(SusceptibilityError::NoHead);
    }
    let inner = shrunk(&tissue, work.sides, half)?;
    Ok((tissue, inner))
}

/// `mask` grown by a voxel every way, or with `grow` false shrunk by one.
fn grown(mask: &[bool], sides: [usize; 3], grow: bool) -> Result<Vec<bool>, SusceptibilityError> {
    let mut share: Vec<f32> = mask
        .par_iter()
        .map(|set| f32::from(u8::from(*set)))
        .collect();
    box_mean(&mut share, sides, [1; 3])?;
    Ok(share
        .par_iter()
        .map(|part| if grow { *part > 1e-3 } else { *part > 0.999 })
        .collect())
}

/// `mask` shrunk by `half` voxels along each side.
fn shrunk(
    mask: &[bool],
    sides: [usize; 3],
    half: [usize; 3],
) -> Result<Vec<bool>, SusceptibilityError> {
    let mut share: Vec<f32> = mask
        .par_iter()
        .map(|set| f32::from(u8::from(*set)))
        .collect();
    box_mean(&mut share, sides, half)?;
    Ok(share.par_iter().map(|part| *part > 0.999).collect())
}

/// Set every voxel of a slice of `mask` that cannot be reached from the slice's edge without
/// crossing a set voxel.
fn fill_holes(mask: &mut [bool], sides: [usize; 3]) {
    let [columns, rows, _] = sides;
    mask.par_chunks_mut(columns * rows).for_each(|plane| {
        let mut outside = vec![false; plane.len()];
        let mut queue: Vec<usize> = (0..plane.len())
            .filter(|&at| {
                let (column, row) = (at % columns, at / columns);
                column == 0 || row == 0 || column + 1 == columns || row + 1 == rows
            })
            .collect();
        while let Some(at) = queue.pop() {
            let free = plane.get(at) == Some(&false);
            match outside.get_mut(at) {
                Some(seen) if free && !*seen => *seen = true,
                _ => continue,
            }
            let (column, row) = (at % columns, at / columns);
            if column > 0 {
                queue.push(at - 1);
            }
            if column + 1 < columns {
                queue.push(at + 1);
            }
            if row > 0 {
                queue.push(at - columns);
            }
            if row + 1 < rows {
                queue.push(at + columns);
            }
        }
        for (set, out) in plane.iter_mut().zip(&outside) {
            *set = !out;
        }
    });
}

/// Replace each value of `values`, a block of `sides` voxels, columns fastest, by the mean of
/// those within `half` voxels of it along each side. Near an edge only the voxels inside are
/// counted.
///
/// # Errors
///
/// [`SusceptibilityError::NoMemory`] when there is not the room for a copy of the block.
pub(super) fn box_mean(
    values: &mut [f32],
    sides: [usize; 3],
    half: [usize; 3],
) -> Result<(), SusceptibilityError> {
    let [columns, rows, slices] = sides;
    let [along_columns, along_rows, along_slices] = half;
    values
        .par_chunks_mut(columns)
        .for_each(|line| mean_line(line, along_columns));
    values.par_chunks_mut(columns * rows).for_each(|plane| {
        let mut line = vec![0.0; rows];
        for first in 0..columns {
            let column = plane.iter().skip(first).step_by(columns);
            line.iter_mut()
                .zip(column)
                .for_each(|(into, from)| *into = *from);
            mean_line(&mut line, along_rows);
            let column = plane.iter_mut().skip(first).step_by(columns);
            column.zip(&line).for_each(|(into, from)| *into = *from);
        }
    });
    let plane = columns * rows;
    let mut gathered = super::zeroed::<f32>(values.len())?;
    let source: &[f32] = values;
    gathered
        .par_chunks_mut(slices)
        .enumerate()
        .for_each(|(place, line)| {
            let numbers = source.iter().skip(place).step_by(plane);
            line.iter_mut()
                .zip(numbers)
                .for_each(|(into, from)| *into = *from);
            mean_line(line, along_slices);
        });
    values
        .par_chunks_mut(plane)
        .enumerate()
        .for_each(|(slice, out)| {
            let numbers = gathered.iter().skip(slice).step_by(slices);
            out.iter_mut()
                .zip(numbers)
                .for_each(|(into, from)| *into = *from);
        });
    Ok(())
}

/// Replace each value of `line` by the mean of those within `half` places of it.
fn mean_line(line: &mut [f32], half: usize) {
    let mut sums = Vec::with_capacity(line.len() + 1);
    sums.push(0.0_f64);
    let mut running = 0.0;
    for value in line.iter() {
        running += f64::from(*value);
        sums.push(running);
    }
    let length = line.len();
    for (at, value) in line.iter_mut().enumerate() {
        let (from, to) = (at.saturating_sub(half), (at + half + 1).min(length));
        let total = sums.get(to).copied().unwrap_or(0.0) - sums.get(from).copied().unwrap_or(0.0);
        *value = (total / (to - from).max(1) as f64) as f32;
    }
}
