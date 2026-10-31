//! The plugin's grid: the scan's own axes, turned and ordered as the description's code says,
//! at the description's spacing. Voxa never rotates a scan; the grid stays aligned to the axes
//! it was stored along.
//!
//! Along the grid's axis `a`, taken from the scan's axis `o` of `n` voxels `s` mm apart, at the
//! plugin's spacing `t`, there are `m = max(1, round(n·s/t))` voxels, centred on the scan's own
//! centre: grid voxel `k` lies at scan index `c + (k − (m − 1)/2)·t/s`, with `c = (n − 1)/2`,
//! counted from the far end of the scan's axis when the grid's runs the other way. A whole box
//! of the description's size is centred on the grid; blocks are laid over it.

use voxa_core::{Affine, Patient, Point, Volume};
use voxa_labels::LabelGrid;
use voxa_models::RunError;
use voxa_segment::Turn;

use crate::Input;

mod sample;
pub use sample::sample;

#[cfg(test)]
mod bead_tests;
#[cfg(test)]
pub(crate) mod tests;

/// The most voxels a grid may have along a side; past it the plugin's spacing is so fine
/// against the scan that no run could hold it.
const LONGEST: f64 = 1_000_000.0;

/// One axis of the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axis {
    /// The scan's axis it runs along: 0 for columns, 1 for rows, 2 for slices.
    pub(crate) from: usize,
    /// How many voxels the scan has along that axis.
    pub(crate) scan: u32,
    /// How many voxels the grid has along it.
    pub(crate) size: usize,
    /// The scan index of the grid's first voxel along it.
    pub(crate) origin: f64,
    /// How far along the scan's axis one step of the grid goes, in scan voxels; below 0 when
    /// the grid runs the other way.
    pub(crate) stride: f64,
    /// The plugin's spacing along it, in millimetres.
    pub(crate) spacing_mm: f64,
}

/// The plugin's grid over one scan.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// How the scan's axes are turned for the grid.
    pub turn: Turn,
    /// How many voxels the grid has along each of its axes, in the tensor's order.
    pub size: [usize; 3],
    pub(crate) axes: [Axis; 3],
}

/// A block of the grid: where it starts, which may be before the grid's first voxel, and its
/// size. The part outside the grid lies outside the scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// The grid voxel of its first voxel, along each axis.
    pub from: [isize; 3],
    /// How many voxels it has along each axis.
    pub size: [usize; 3],
}

/// A count of voxels from a number that is one: held from 1 to [`LONGEST`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held from 1 to a million first"
)]
fn count(voxels: f64) -> usize {
    voxels.round().clamp(1.0, LONGEST) as usize
}

impl Grid {
    /// The grid of the description's `input` over this scan.
    pub fn of(volume: &Volume, input: &Input) -> Grid {
        let scan_spacing = volume.spacing_mm();
        let turn = Turn::towards(volume, scan_spacing, input.axes);
        let dims = volume.dims();
        let make = |place: usize| {
            let from = turn.order.get(place).copied().unwrap_or(place);
            let round = turn.turned_round.get(place).copied().unwrap_or(false);
            let scan = dims.get(from).copied().unwrap_or(1);
            let s = scan_spacing.get(from).copied().unwrap_or(1.0);
            let t = input.spacing_mm.get(place).copied().unwrap_or(1.0);
            let (n, ratio) = (f64::from(scan), t / s);
            let size = count(n * s / t);
            let (centre, half) = ((n - 1.0) / 2.0, (size as f64 - 1.0) / 2.0 * ratio);
            let (origin, stride) = if round {
                (centre + half, -ratio)
            } else {
                (centre - half, ratio)
            };
            Axis {
                from,
                scan,
                size,
                origin,
                stride,
                spacing_mm: t,
            }
        };
        let axes = [make(0), make(1), make(2)];
        Grid {
            turn,
            size: axes.map(|axis| axis.size),
            axes,
        }
    }

    /// The whole grid, as a region.
    pub fn whole(&self) -> Region {
        Region {
            from: [0; 3],
            size: self.size,
        }
    }

    /// A box of this size centred on the grid: along each axis it starts
    /// `floor((m − size) / 2)` voxels in, which is before the grid's start when the box is the
    /// larger.
    pub fn whole_box(&self, size: [usize; 3]) -> Region {
        let start = |(grid, size): (usize, usize)| {
            let (grid, size) = (signed(grid), signed(size));
            (grid - size).div_euclid(2)
        };
        let mut from = [0; 3];
        for (slot, pair) in from.iter_mut().zip(self.size.into_iter().zip(size)) {
            *slot = start(pair);
        }
        Region { from, size }
    }

    /// The scan index, along the scan's own columns, rows and slices, of a place in the grid.
    pub(crate) fn scan_index(&self, at: [f64; 3]) -> [f64; 3] {
        let mut index = [0.0; 3];
        for (axis, k) in self.axes.iter().zip(at) {
            if let Some(slot) = index.get_mut(axis.from) {
                *slot = axis.origin + k * axis.stride;
            }
        }
        index
    }

    /// Where a place in the grid lies in the patient.
    pub fn patient_of(&self, volume: &Volume, at: [f64; 3]) -> Point<Patient> {
        let [col, row, slice] = self.scan_index(at);
        volume.voxel_to_patient().apply(Point::new(col, row, slice))
    }
}

/// A count as a signed number; no grid or box comes near the largest.
fn signed(count: usize) -> isize {
    isize::try_from(count).unwrap_or(isize::MAX)
}

/// The part of a region inside the grid, along each axis: where it starts and how many.
fn clipped(grid: &Grid, region: Region) -> ([usize; 3], [usize; 3]) {
    let (mut from, mut size) = ([0; 3], [0; 3]);
    for (place, axis) in grid.axes.iter().enumerate() {
        let start = region.from.get(place).copied().unwrap_or(0);
        let length = signed(region.size.get(place).copied().unwrap_or(0));
        let low = start.max(0);
        let high = (start + length).min(signed(axis.size));
        if let (Some(from), Some(size)) = (from.get_mut(place), size.get_mut(place)) {
            *from = usize::try_from(low).unwrap_or(0);
            *size = usize::try_from(high - low).unwrap_or(0);
        }
    }
    (from, size)
}

/// Labels on the part of `region` inside the grid, `ids` given for every voxel of the region
/// in the tensor's order (the last axis fastest), turned back to lie along the scan's own axes,
/// columns fastest, each voxel where the grid's voxel lies in the patient. Never resampled:
/// the labels keep the plugin's spacing.
pub fn label_grid(grid: &Grid, region: Region, ids: &[u16], volume: &Volume) -> LabelGrid {
    let (from, size) = clipped(grid, region);
    let mut dims = [1_u32; 3];
    for (axis, size) in grid.axes.iter().zip(size) {
        if let Some(slot) = dims.get_mut(axis.from) {
            *slot = u32::try_from(size).unwrap_or(u32::MAX);
        }
    }
    let [cols, rows, slices] = dims.map(|side| side as usize);
    let mut labels = vec![0_u32; cols * rows * slices];
    let scan_stride = [1, cols, cols * rows];
    let offset = |place: usize, inside: usize| {
        let (axis, size) = (grid.axes.get(place), size.get(place).copied().unwrap_or(1));
        let Some(axis) = axis else { return 0 };
        let q = if axis.stride < 0.0 {
            size - 1 - inside
        } else {
            inside
        };
        q * scan_stride.get(axis.from).copied().unwrap_or(0)
    };
    let [s0, s1, s2] = size;
    let region_at = |place: usize, inside: usize| {
        let skipped = from.get(place).copied().unwrap_or(0);
        let start = region.from.get(place).copied().unwrap_or(0);
        usize::try_from(signed(skipped + inside) - start).unwrap_or(0)
    };
    let [_, r1, r2] = region.size;
    for a in 0..s0 {
        for b in 0..s1 {
            for c in 0..s2 {
                let at = (region_at(0, a) * r1 + region_at(1, b)) * r2 + region_at(2, c);
                let to = offset(0, a) + offset(1, b) + offset(2, c);
                if let (Some(id), Some(slot)) = (ids.get(at), labels.get_mut(to)) {
                    *slot = u32::from(*id);
                }
            }
        }
    }
    LabelGrid {
        dims,
        ids: labels,
        placement: placement(grid, from, size, volume),
    }
}

/// Where the labels of the part of the grid from `from`, of `size`, lie in the patient, their
/// voxels counted along the scan's own axes in the scan's own direction.
fn placement(
    grid: &Grid,
    from: [usize; 3],
    size: [usize; 3],
    volume: &Volume,
) -> Affine<voxa_core::Voxel, Patient> {
    let scan = volume.voxel_to_patient();
    let steps = scan.columns();
    let (mut base, mut columns) = ([0.0; 3], [[0.0; 3]; 3]);
    for ((axis, from), size) in grid.axes.iter().zip(from).zip(size) {
        // The first label along the scan's axis is the grid's first voxel there, or, when the
        // grid runs the other way, its last.
        let first = if axis.stride < 0.0 {
            from + size.max(1) - 1
        } else {
            from
        };
        let ratio = axis.stride.abs();
        if let (Some(base), Some(column), Some(step)) = (
            base.get_mut(axis.from),
            columns.get_mut(axis.from),
            steps.get(axis.from),
        ) {
            *base = axis.origin + first as f64 * axis.stride;
            *column = step.map(|part| part * ratio);
        }
    }
    let [col, row, slice] = base;
    let origin = scan.apply(Point::new(col, row, slice));
    let [x, y, z] = columns;
    Affine::from_columns(x, y, z, [origin.x, origin.y, origin.z])
}

/// How far the scan's grid reaches past a region along any axis, at most, in millimetres:
/// `None` when the region holds all of it.
pub fn overhang_mm(grid: &Grid, region: Region) -> Option<f64> {
    let mut most: f64 = 0.0;
    for (place, axis) in grid.axes.iter().enumerate() {
        let start = region.from.get(place).copied().unwrap_or(0);
        let end = start + signed(region.size.get(place).copied().unwrap_or(0));
        let before = start.max(0);
        let after = (signed(axis.size) - end).max(0);
        let voxels = before.max(after) as f64;
        most = most.max(voxels * axis.spacing_mm);
    }
    (most > 0.0).then_some(most)
}

/// Asks for room for so many values, refusing rather than ending the program.
pub(crate) fn room<T>(count: usize) -> Result<Vec<T>, RunError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RunError::NoRoom)?;
    Ok(values)
}
