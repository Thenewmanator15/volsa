//! The grid the map is worked out on: the scan's own grid with neighbouring voxels taken
//! together until they are about [`WORK_MM`] apart, and room left round it so that the
//! transforms do not carry one edge of the head round to the other. The map is put back on
//! the scan's own grid when it is done.

use rayon::prelude::*;
use rustfft::num_complex::Complex32;

use super::SusceptibilityError;
use super::fft::smooth_length;

/// About how far apart the voxels of the working grid are, in millimetres.
const WORK_MM: f64 = 0.9;
/// The room left round the head on each side, in millimetres.
const ROOM_MM: f64 = 15.0;

/// The working grid and how it lies on the scan's own.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Work {
    /// Its columns, rows and slices, room included.
    pub sides: [usize; 3],
    /// How far apart its voxels are along each side, in millimetres.
    pub step: [f64; 3],
    /// How many of the scan's voxels are taken together along each side.
    factor: [usize; 3],
    /// Where the scan's first voxel falls, in working voxels from the first, along each side.
    offset: [usize; 3],
    /// The scan's own sides.
    scan: [usize; 3],
}

impl Work {
    /// The working grid for a scan of `scan` voxels `spacing` millimetres apart.
    pub(super) fn for_scan(scan: [usize; 3], spacing: [f64; 3]) -> Self {
        let factor = spacing.map(|step| (WORK_MM / step).round().max(1.0) as usize);
        let mut sides = [0; 3];
        let mut step = [0.0; 3];
        let mut offset = [0; 3];
        for axis in 0..3 {
            let (Some(&own), Some(&by), Some(&apart)) =
                (scan.get(axis), factor.get(axis), spacing.get(axis))
            else {
                continue;
            };
            let worked = apart * by as f64;
            let room = (ROOM_MM / worked).ceil() as usize;
            let inner = own.div_ceil(by);
            if let (Some(side), Some(gap), Some(at)) = (
                sides.get_mut(axis),
                step.get_mut(axis),
                offset.get_mut(axis),
            ) {
                *side = smooth_length(inner + 2 * room);
                *gap = worked;
                *at = (*side - inner) / 2;
            }
        }
        Self {
            sides,
            step,
            factor,
            offset,
            scan,
        }
    }

    /// How many voxels the working grid has.
    pub(super) fn count(&self) -> usize {
        self.sides.iter().product()
    }

    /// The working voxel a voxel of the scan is taken into.
    fn work_place(&self, at: [usize; 3]) -> usize {
        let place = |axis: usize| {
            let (own, by, from) = (at.get(axis), self.factor.get(axis), self.offset.get(axis));
            own.zip(by)
                .zip(from)
                .map_or(0, |((own, by), from)| own / by + from)
        };
        let [columns, rows, _] = self.sides;
        (place(2) * rows + place(1)) * columns + place(0)
    }

    /// The scan's phase, in radians, taken onto this grid: each working voxel has the angle
    /// of the mean of its voxels' phases as turns, and the share of its voxels that were
    /// exactly 0, which a scanner writes where it has blanked the phase. The room round the
    /// scan has phase 0 and is wholly blank.
    ///
    /// # Errors
    ///
    /// [`SusceptibilityError::NoMemory`] when there is not the room for them.
    pub(super) fn take_in(
        &self,
        phase: impl Iterator<Item = f32>,
    ) -> Result<(Vec<f32>, Vec<f32>), SusceptibilityError> {
        let count = self.count();
        let mut turns = super::zeroed::<Complex32>(count)?;
        let mut blank = super::zeroed::<f32>(count)?;
        let mut taken = super::zeroed::<f32>(count)?;
        let [columns, rows, _] = self.scan;
        for (place, value) in phase.enumerate() {
            let at = [
                place % columns,
                place / columns % rows,
                place / (columns * rows),
            ];
            let into = self.work_place(at);
            if let (Some(turn), Some(empty), Some(seen)) = (
                turns.get_mut(into),
                blank.get_mut(into),
                taken.get_mut(into),
            ) {
                *turn += Complex32::from_polar(1.0, value);
                *empty += f32::from(u8::from(value == 0.0));
                *seen += 1.0;
            }
        }
        let angles = turns.par_iter().map(|turn| turn.arg()).collect();
        blank
            .par_iter_mut()
            .zip(&taken)
            .for_each(|(empty, seen)| *empty = if *seen > 0.0 { *empty / seen } else { 1.0 });
        Ok((angles, blank))
    }

    /// `map`, on this grid, put back on the scan's own grid: each voxel of the scan takes the
    /// value at its place in the working grid, blended from the eight working voxels round
    /// it.
    ///
    /// # Errors
    ///
    /// [`SusceptibilityError::NoMemory`] when there is not the room for it.
    pub(super) fn give_back(&self, map: &[f32]) -> Result<Vec<f32>, SusceptibilityError> {
        let [columns, rows, slices] = self.scan;
        let mut out = super::zeroed::<f32>(columns * rows * slices)?;
        out.par_chunks_mut(columns * rows)
            .enumerate()
            .for_each(|(slice, plane)| {
                for (place, value) in plane.iter_mut().enumerate() {
                    let at = [place % columns, place / columns, slice];
                    *value = self.blended(map, at);
                }
            });
        Ok(out)
    }

    /// The value of `map` where the centre of the scan's voxel `at` lies on this grid.
    fn blended(&self, map: &[f32], at: [usize; 3]) -> f32 {
        let mut low = [0_usize; 3];
        let mut part = [0.0_f64; 3];
        for axis in 0..3 {
            let (Some(&own), Some(&by), Some(&from), Some(&side)) = (
                at.get(axis),
                self.factor.get(axis),
                self.offset.get(axis),
                self.sides.get(axis),
            ) else {
                continue;
            };
            let place = ((own as f64 + 0.5) / by as f64 - 0.5 + from as f64).max(0.0);
            let below = (place.floor() as usize).min(side.saturating_sub(2));
            if let (Some(low), Some(part)) = (low.get_mut(axis), part.get_mut(axis)) {
                *low = below;
                *part = (place - below as f64).clamp(0.0, 1.0);
            }
        }
        let [columns, rows, _] = self.sides;
        let mut sum = 0.0;
        for corner in 0..8_usize {
            let pick = [corner & 1, corner >> 1 & 1, corner >> 2 & 1];
            let weight: f64 = pick
                .iter()
                .zip(&part)
                .map(|(&bit, &share)| if bit == 1 { share } else { 1.0 - share })
                .product();
            let [x, y, z] = [0, 1, 2].map(|axis| {
                low.get(axis).copied().unwrap_or(0) + pick.get(axis).copied().unwrap_or(0)
            });
            let value = map
                .get((z * rows + y) * columns + x)
                .copied()
                .unwrap_or(0.0);
            sum += weight * f64::from(value);
        }
        sum as f32
    }
}
