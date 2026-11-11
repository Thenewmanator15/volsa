//! Blocks of a fixed size laid over the plugin's grid, as nnU-Net lays them: along each axis
//! from the start, `step` apart, the last laid against the far end; one block when the grid is
//! no longer than a block. Each voxel is given to the block whose centre is nearest, the
//! earlier on a tie, so that what a block's answer gives is written once and nothing is added
//! up: memory is one block and the grid of labels, whatever the number of parts.

use crate::Region;

#[cfg(test)]
mod tests;

/// Blocks over a grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPlan {
    /// The grid's size along each axis.
    pub grid: [usize; 3],
    /// A block's size along each axis.
    pub size: [usize; 3],
    /// How far apart blocks start along each axis: from 1 to the block's size.
    pub step: [usize; 3],
}

/// Where blocks start along one axis.
fn starts_along(grid: usize, size: usize, step: usize) -> Vec<usize> {
    if grid <= size {
        return vec![0];
    }
    let mut starts = Vec::new();
    let mut start = 0;
    while start + size < grid {
        starts.push(start);
        start += step.max(1);
    }
    starts.push(grid - size);
    starts
}

/// The centre of a block along one axis.
fn centre(start: usize, size: usize) -> f64 {
    start as f64 + (size as f64 - 1.0) / 2.0
}

/// The voxels along one axis the block starting at `starts[place]` owns: from just past the
/// midpoint with the centre before it to the midpoint with the centre after it, a voxel on a
/// midpoint going to the earlier block.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "midpoints lie within the grid"
)]
fn owned_along(starts: &[usize], place: usize, size: usize, grid: usize) -> (usize, usize) {
    let centre_of = |place: usize| starts.get(place).map(|start| centre(*start, size));
    let Some(own) = centre_of(place) else {
        return (0, 0);
    };
    let low = match place.checked_sub(1).and_then(centre_of) {
        Some(before) => f64::midpoint(before, own).floor() as usize + 1,
        None => 0,
    };
    let high = match centre_of(place + 1) {
        Some(after) => f64::midpoint(own, after).floor() as usize + 1,
        None => grid,
    };
    (low.min(grid), high.min(grid))
}

impl BlockPlan {
    /// The starts along each axis.
    fn along(&self) -> [Vec<usize>; 3] {
        let [g0, g1, g2] = self.grid;
        let [s0, s1, s2] = self.size;
        let [t0, t1, t2] = self.step;
        [
            starts_along(g0, s0, t0),
            starts_along(g1, s1, t1),
            starts_along(g2, s2, t2),
        ]
    }

    /// Where each block starts, the first axis changing slowest.
    pub fn starts(&self) -> Vec<[usize; 3]> {
        let [first, second, third] = self.along();
        let mut starts = Vec::with_capacity(first.len() * second.len() * third.len());
        for a in &first {
            for b in &second {
                for c in &third {
                    starts.push([*a, *b, *c]);
                }
            }
        }
        starts
    }

    /// How many blocks there are.
    pub fn count(&self) -> usize {
        self.along().iter().map(Vec::len).product()
    }

    /// The voxels of the grid the block starting at `start` owns.
    pub fn owned(&self, start: [usize; 3]) -> Region {
        let along = self.along();
        let mut region = Region {
            from: [0; 3],
            size: [0; 3],
        };
        for axis in 0..3 {
            let starts = along.get(axis).map(Vec::as_slice).unwrap_or_default();
            let at = start.get(axis).copied().unwrap_or(0);
            let place = starts.iter().position(|start| *start == at).unwrap_or(0);
            let size = self.size.get(axis).copied().unwrap_or(1);
            let grid = self.grid.get(axis).copied().unwrap_or(1);
            let (low, high) = owned_along(starts, place, size, grid);
            if let (Some(from), Some(count)) =
                (region.from.get_mut(axis), region.size.get_mut(axis))
            {
                *from = isize::try_from(low).unwrap_or(isize::MAX);
                *count = high.saturating_sub(low);
            }
        }
        region
    }
}
