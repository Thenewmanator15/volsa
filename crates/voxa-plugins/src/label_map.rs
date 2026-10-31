//! The labels of a whole run, put together from its blocks' answers, each voxel written by the
//! block that owns it; then, when the description asks, only the largest connected piece kept;
//! then turned back onto the scan as a label set with the names its description gives.

use voxa_core::Volume;
use voxa_labels::{Entry, LabelGrid, Table, Trust, colour_for};
use voxa_segment::largest_piece;

use crate::finish::{FinishError, room};
use crate::{Grid, LabelOutput, Region, label_grid};

#[cfg(test)]
mod tests;

/// The labels of a region of the grid, filled in block by block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelMap {
    region: Region,
    /// A label for each voxel of the region, the last axis fastest.
    pub(crate) ids: Vec<u16>,
}

/// A place along one axis, counted from `from`, if it lies within `size` of it.
fn within(place: isize, from: isize, size: usize) -> Option<usize> {
    let inside = usize::try_from(place - from).ok()?;
    (inside < size).then_some(inside)
}

impl LabelMap {
    /// A map of the region, every voxel unlabelled.
    ///
    /// # Errors
    ///
    /// [`FinishError::NoRoom`] when there is not the memory for it.
    pub fn new(region: Region) -> Result<LabelMap, FinishError> {
        let count = region
            .size
            .iter()
            .try_fold(1_usize, |count, side| count.checked_mul(*side))
            .ok_or(FinishError::NoRoom)?;
        let mut ids = room(count)?;
        ids.resize(count, 0);
        Ok(LabelMap { region, ids })
    }

    /// Writes the labels of the block that starts at `start` (a place in the grid) and is `size`
    /// voxels, `labels` in the block's own order, at the voxels it `owns` that lie in the map.
    pub fn put(&mut self, start: [isize; 3], size: [usize; 3], owns: Region, labels: &[u16]) {
        let [m0, m1, m2] = self.region.size;
        let [b0, b1, b2] = size;
        let [f0, f1, f2] = self.region.from;
        let [s0, s1, s2] = start;
        let [o0, o1, o2] = owns.from;
        let place = |from: isize, step: usize| from + isize::try_from(step).unwrap_or(isize::MAX);
        for i in 0..owns.size[0] {
            let x = place(o0, i);
            let (Some(mx), Some(bx)) = (within(x, f0, m0), within(x, s0, b0)) else {
                continue;
            };
            for j in 0..owns.size[1] {
                let y = place(o1, j);
                let (Some(my), Some(by)) = (within(y, f1, m1), within(y, s1, b1)) else {
                    continue;
                };
                for k in 0..owns.size[2] {
                    let z = place(o2, k);
                    let (Some(mz), Some(bz)) = (within(z, f2, m2), within(z, s2, b2)) else {
                        continue;
                    };
                    let label = labels.get((bx * b1 + by) * b2 + bz).copied().unwrap_or(0);
                    if let Some(slot) = self.ids.get_mut((mx * m1 + my) * m2 + mz) {
                        *slot = label;
                    }
                }
            }
        }
    }

    /// The labels as a label set on the scan, with the table its names make: only the largest
    /// connected piece of everything labelled when the description asks.
    ///
    /// # Errors
    ///
    /// [`FinishError::Nothing`] when no voxel of the scan is labelled.
    pub fn finish(
        mut self,
        output: &LabelOutput,
        title: &str,
        grid: &Grid,
        volume: &Volume,
    ) -> Result<(LabelGrid, Table), FinishError> {
        if output.keep_largest_piece {
            let mask: Vec<bool> = self.ids.iter().map(|id| *id != 0).collect();
            let kept = largest_piece(&mask, self.region.size);
            for (id, kept) in self.ids.iter_mut().zip(kept) {
                if !kept {
                    *id = 0;
                }
            }
        }
        let labels = label_grid(grid, self.region, &self.ids, volume);
        if labels.ids.iter().all(|id| *id == 0) {
            return Err(FinishError::Nothing);
        }
        Ok((labels, table(output, title)))
    }
}

/// The table of a plugin's labels: one entry for each name, made by a model, with no verdict
/// and, until the run says who made it, no maker.
fn table(output: &LabelOutput, title: &str) -> Table {
    let entries = output
        .names
        .iter()
        .map(|name| Entry {
            name: name.name.clone(),
            group: name.group.clone(),
            side: name.side.into(),
            ids: vec![u32::from(name.id)],
            colour: name
                .colour
                .unwrap_or_else(|| colour_for(u32::from(name.id))),
            trust: Trust::Model,
        })
        .collect();
    Table {
        title: title.to_owned(),
        entries,
        verdict: None,
        made: None,
    }
}
