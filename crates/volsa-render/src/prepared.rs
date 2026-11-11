//! A volume made ready for the graphics device. This is the slow part of showing a large
//! series in 3D, and it needs no device, so it can be done away from the interface's thread.

use std::borrow::Cow;
use std::cell::Cell;

use egui_wgpu::wgpu;
use half::f16;
use rayon::prelude::*;
use voxa_core::{Affine, Patient, Volume, Voxel};
use voxa_volume::{block_maxima, halved, halvings_needed};

/// The side, in voxels, of the blocks whose largest values are kept.
pub(crate) const BLOCK: u32 = 8;
/// The most a volume may take on the graphics device. A device does not say how much room it
/// has, so a volume is reduced to this before it is tried: 512 MiB, which holds 512 x 512
/// voxels in 1024 slices whole.
pub const MAX_VOLUME_BYTES: u64 = 512 << 20;

/// A volume as the graphics device will hold it: every value as it is stored, the largest
/// value in each block, and what is needed to find a point of the patient in it.
pub struct Prepared {
    /// Every value, from 0 at the smallest to 1 at the largest, as 16-bit floating point.
    pub(crate) halves: Vec<u16>,
    /// The largest value in each block and the blocks touching it, stored the same way.
    /// `None` when that is left for the graphics device to work out from the values.
    pub(crate) maxima: Option<Vec<u16>>,
    /// How many blocks there are along columns, rows and slices.
    pub(crate) blocks: [u32; 3],
    /// Columns, rows and slices as stored, after any halving.
    pub(crate) dims: [u32; 3],
    pub(crate) patient_to_voxel: Affine<Patient, Voxel>,
    /// The smallest distance between neighbouring voxels as stored, in millimetres.
    pub(crate) finest_mm: f64,
    /// The scan value stored as 0.
    pub(crate) smallest: f32,
    /// The span of scan values from the one stored as 0 to the one stored as 1.
    pub(crate) span: f32,
    halved: u32,
    /// The values on the graphics device already, where they were handed over ahead of
    /// time; taken up by whatever puts the volume there.
    pub(crate) handed: Cell<Option<wgpu::Texture>>,
}

impl Prepared {
    /// Make a volume ready for a device whose longest side for a volume is `max_side` voxels
    /// and which is to be given no more than `max_bytes`: the volume is halved, each way,
    /// until it fits.
    pub fn of(volume: &Volume, max_side: u32, max_bytes: u64) -> Self {
        let halvings = halvings_needed(volume.dims(), max_side, max_bytes);
        let mut shown = Cow::Borrowed(volume);
        for _ in 0..halvings {
            shown = Cow::Owned(halved(&shown));
        }
        Self::as_it_is(&shown, halvings)
    }

    /// As [`Prepared::of`], but with the largest value in each block left for the graphics
    /// device to work out once it holds the values: a pass over every voxel the less.
    pub fn without_blocks(volume: &Volume, max_side: u32, max_bytes: u64) -> Self {
        let halvings = halvings_needed(volume.dims(), max_side, max_bytes);
        let mut shown = Cow::Borrowed(volume);
        for _ in 0..halvings {
            shown = Cow::Owned(halved(&shown));
        }
        let blocks = shown.dims().map(|count| count.div_ceil(BLOCK));
        let mut ready = Self::with_maxima(&shown, (&[], blocks), halvings);
        ready.maxima = None;
        ready
    }

    /// Make a volume ready at the size it is. `halved` is how often it has been halved.
    pub(crate) fn as_it_is(volume: &Volume, halved: u32) -> Self {
        let (maxima, blocks) = block_maxima(volume, BLOCK);
        let maxima = with_neighbours(&maxima, blocks);
        Self::with_maxima(volume, (&maxima, blocks), halved)
    }

    /// As [`Prepared::as_it_is`], with the largest scan value in each block, and the number
    /// of blocks each way, given and not worked out.
    pub(crate) fn with_maxima(
        volume: &Volume,
        (maxima, blocks): (&[f32], [u32; 3]),
        times_halved: u32,
    ) -> Self {
        let (halves, smallest, span) = as_halves(volume);
        let stored = |value: &f32| as_half(*value, smallest, span);
        Self {
            halves,
            maxima: Some(maxima.iter().map(stored).collect()),
            blocks,
            dims: volume.dims(),
            patient_to_voxel: volume.patient_to_voxel(),
            finest_mm: volume
                .spacing_mm()
                .into_iter()
                .fold(f64::INFINITY, f64::min),
            smallest,
            span,
            halved: times_halved,
            handed: Cell::new(None),
        }
    }

    /// Hand the values to the graphics device now, from whatever thread this is, so that
    /// putting the volume there later has no bytes left to hand over: that is the part
    /// that takes time, and it need not be the interface's thread that spends it. False,
    /// with nothing handed over, when the device has no room or the volume is too long a
    /// side for it.
    pub fn hand_over(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        let longest = device.limits().max_texture_dimension_3d;
        if self.dims.iter().any(|side| *side == 0 || *side > longest) {
            return false;
        }
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let values = crate::volume_texture::values_on(device, queue, self);
        if pollster::block_on(room.pop()).is_some() {
            values.destroy();
            return false;
        }
        if let Some(before) = self.handed.replace(Some(values)) {
            before.destroy();
        }
        true
    }

    /// Whether the values are on the graphics device already and have not been taken up.
    pub fn is_handed_over(&self) -> bool {
        let handed = self.handed.take();
        let there = handed.is_some();
        self.handed.set(handed);
        there
    }

    /// Whether the largest value in each block has been worked out, and is not left for
    /// the graphics device.
    pub fn has_blocks(&self) -> bool {
        self.maxima.is_some()
    }

    /// How many times the volume was halved, each way, to fit.
    pub fn halved(&self) -> u32 {
        self.halved
    }

    /// Columns, rows and slices as they will be stored.
    pub fn dims(&self) -> [u32; 3] {
        self.dims
    }
}

impl Drop for Prepared {
    /// Values handed to the device and never taken up are given back to it at once.
    fn drop(&mut self) {
        if let Some(unused) = self.handed.take() {
            unused.destroy();
        }
    }
}

impl std::fmt::Debug for Prepared {
    /// Prints the size, not the values: there are millions of them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut shown = f.debug_struct("Prepared");
        shown
            .field("dims", &self.dims)
            .field("halved", &self.halved);
        shown.finish_non_exhaustive()
    }
}

/// The smallest and largest of the values that are finite numbers. `None` when none is.
fn range_of(values: &[f32]) -> Option<(f32, f32)> {
    let widest = || (f32::INFINITY, f32::NEG_INFINITY);
    let (smallest, largest) = values
        .par_iter()
        .filter(|value| value.is_finite())
        .fold(widest, |(low, high), value| {
            (low.min(*value), high.max(*value))
        })
        .reduce(widest, |(low, high), (lower, higher)| {
            (low.min(lower), high.max(higher))
        });
    (smallest <= largest).then_some((smallest, largest))
}

/// The scan value a volume's values are stored from, as 0, and the span from it to the one
/// stored as 1: its smallest value that is a number, and how far its largest is above that.
/// A volume all of one value has a span of 1.
pub(crate) fn stored_range(volume: &Volume) -> (f32, f32) {
    let (smallest, largest) = range_of(volume.values()).unwrap_or((0.0, 0.0));
    let span = if largest > smallest {
        largest - smallest
    } else {
        1.0
    };
    (smallest, span)
}

/// The values of a volume as 16-bit floating-point numbers from 0, its smallest value, to 1,
/// its largest; then that smallest value and the span from it to the largest. A value that
/// is not a number is stored as 0. A volume all of one value is stored as zeros, with a span
/// of 1.
pub(crate) fn as_halves(volume: &Volume) -> (Vec<u16>, f32, f32) {
    let (smallest, span) = stored_range(volume);
    let stored = |value: &f32| as_half(*value, smallest, span);
    (
        volume.values().par_iter().map(stored).collect(),
        smallest,
        span,
    )
}

/// One scan value as it is stored: how far it is from `smallest` towards the top of `span`,
/// from 0 to 1, as the bits of a 16-bit floating-point number.
pub(crate) fn as_half(value: f32, smallest: f32, span: f32) -> u16 {
    let part = (value - smallest) / span;
    // Infinities land on the nearer end; what is not a number has no end to land on.
    f16::from_f32(if part.is_nan() {
        0.0
    } else {
        part.clamp(0.0, 1.0)
    })
    .to_bits()
}

/// The largest value of each block made the largest of its own and those of the blocks
/// touching it, corners included. A value read anywhere in a block is blended from voxels
/// that may lie just over the block's faces; after this the block's figure covers them too.
/// `blocks` is how many blocks there are along columns, rows and slices.
pub(crate) fn with_neighbours(maxima: &[f32], blocks: [u32; 3]) -> Vec<f32> {
    let [cols, rows, slices] = blocks.map(|count| count as usize);
    // Along columns, then rows, then slices: each pass spreads what the last one gathered.
    let axes = [(1, cols), (cols, rows), (cols * rows, slices)];
    axes.into_iter()
        .fold(maxima.to_vec(), |values, (apart, count)| {
            let largest_about = |at: usize| {
                let place = at / apart.max(1) % count.max(1);
                let before = at.checked_sub(apart).filter(|_| place > 0);
                let after = (place + 1 < count).then_some(at + apart);
                let about = [Some(at), before, after].into_iter().flatten();
                about
                    .filter_map(|at| values.get(at))
                    .fold(f32::NEG_INFINITY, |a, b| a.max(*b))
            };
            (0..values.len()).map(largest_about).collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MM, ball_on, centred};

    #[test]
    fn a_volume_that_fits_is_made_ready_as_it_is() {
        let volume = ball_on([24; 3], centred([24; 3], MM));
        let ready = Prepared::of(&volume, 2048, MAX_VOLUME_BYTES);
        assert_eq!((ready.dims(), ready.halved()), ([24, 24, 24], 0));
        assert_eq!(
            (
                ready.halves.len(),
                ready.blocks,
                ready.maxima.as_ref().map(Vec::len)
            ),
            (24 * 24 * 24, [3; 3], Some(27))
        );
        assert_eq!(ready.halves, as_halves(&volume).0);
        assert_eq!((ready.smallest, ready.span), (0.0, 400.0));
    }

    #[test]
    fn a_volume_too_long_or_too_large_is_halved_until_it_fits() {
        let volume = ball_on([24; 3], centred([24; 3], MM));
        let by_side = Prepared::of(&volume, 12, MAX_VOLUME_BYTES);
        assert_eq!((by_side.dims(), by_side.halved()), ([12, 12, 12], 1));
        assert_eq!(by_side.finest_mm, 2.0, "its voxels are twice as far apart");
        let by_bytes = Prepared::of(&volume, 2048, 6 * 6 * 6 * 2);
        assert_eq!((by_bytes.dims(), by_bytes.halved()), ([6, 6, 6], 2));
        assert_eq!(by_bytes.halves.len(), 216);
    }

    #[test]
    fn the_range_of_values_leaves_out_what_is_not_a_number() {
        assert_eq!(
            range_of(&[3.0, f32::NAN, -1.0, f32::INFINITY]),
            Some((-1.0, 3.0))
        );
        assert_eq!(range_of(&[f32::NAN, f32::NEG_INFINITY]), None);
        assert_eq!(range_of(&[]), None);
        let many: Vec<f32> = (0..100_000u32).map(|n| (n % 977) as f32 - 100.0).collect();
        assert_eq!(
            range_of(&many),
            Some((-100.0, 876.0)),
            "the same however the work is shared"
        );
    }
}
