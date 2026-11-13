//! Dropping pieces of a mask too small to be a stretch of tube. Voxels are joined when they
//! touch at all, by a face, an edge or a corner, since a thin tube that runs aslant the grid
//! goes from voxel to voxel by their corners.

use super::VeinsError;

/// A voxel set and not yet met.
const SET: u8 = 1;
/// A voxel met, in the piece being gathered.
const MET: u8 = 2;
/// A voxel of a piece kept.
const KEPT: u8 = 3;

/// Clear the pieces of `mask` (1 for set, 0 for not, columns fastest over `sides` voxels) that
/// have fewer than `smallest` voxels. What is left is 1 where it was set and is kept.
///
/// # Errors
///
/// [`VeinsError::NoMemory`] when there is not the room to gather a piece.
pub(super) fn drop_small(
    mask: &mut [u8],
    sides: [usize; 3],
    smallest: f64,
) -> Result<(), VeinsError> {
    let [columns, rows, slices] = sides;
    let plane = columns * rows;
    let mut piece: Vec<usize> = Vec::new();
    let mut queue: Vec<usize> = Vec::new();
    for start in 0..mask.len() {
        if mask.get(start) != Some(&SET) {
            continue;
        }
        piece.clear();
        queue.clear();
        queue.push(start);
        if let Some(voxel) = mask.get_mut(start) {
            *voxel = MET;
        }
        while let Some(here) = queue.pop() {
            piece.try_reserve(1).map_err(|_| VeinsError::NoMemory)?;
            piece.push(here);
            let (slice, row, column) = (here / plane, here / columns % rows, here % columns);
            for dz in near(slice, slices) {
                for dy in near(row, rows) {
                    for dx in near(column, columns) {
                        let there = dz * plane + dy * columns + dx;
                        if let Some(voxel) = mask.get_mut(there).filter(|voxel| **voxel == SET) {
                            *voxel = MET;
                            queue.try_reserve(1).map_err(|_| VeinsError::NoMemory)?;
                            queue.push(there);
                        }
                    }
                }
            }
        }
        let left = if (piece.len() as f64) < smallest {
            0
        } else {
            KEPT
        };
        for voxel in &piece {
            if let Some(voxel) = mask.get_mut(*voxel) {
                *voxel = left;
            }
        }
    }
    for voxel in mask.iter_mut() {
        *voxel = u8::from(*voxel == KEPT);
    }
    Ok(())
}

/// The places next to `place` along a side `length` long, and `place` itself.
fn near(place: usize, length: usize) -> std::ops::RangeInclusive<usize> {
    place.saturating_sub(1)..=(place + 1).min(length - 1)
}
