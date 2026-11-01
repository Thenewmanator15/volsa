//! A held volume handed to the graphics device by the window's own thread, a slab at a time
//! and a little each frame, so that no other thread hands the device work while the window
//! may be setting itself up, and the window never stalls on a large volume. And how much of
//! a volume a device can hold.

use egui_wgpu::wgpu;
use half::f16;
use volsa_core::{HeldVolume, SlabRaw, Stored};

use crate::prepared::as_half;

mod paced;
mod slabs;

pub(crate) use paced::Upload;
pub use paced::UploadState;
pub(crate) use slabs::SlabPlan;

/// The most bytes handed to the device in one frame: one slab of a held volume, which takes
/// about 11 ms on the RTX, inside a frame of 16 ms (64 MiB took 21 ms).
pub const UPLOAD_BYTES_A_FRAME: u64 = 32 << 20;
/// The most one texture of a volume may take.
pub(crate) const MAX_TEXTURE_BYTES: u64 = 512 << 20;
/// What the stored number of a signed value is moved up by, so that it is held as unsigned.
const SIGNED_OFFSET: f64 = 32768.0;
/// The largest stored number of 16 bits, which a normalised texel of 1 stands for.
const TOP: f64 = 65535.0;

/// How much of a volume a graphics device can hold, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardRoom {
    /// The longest side, in voxels, of a 3D texture on the device.
    pub longest_side: u32,
    /// The most bytes a volume may take on the device, where that is known.
    pub most_bytes: Option<u64>,
    /// Whether the device holds 16-bit normalised numbers, so that a volume of 16-bit whole
    /// numbers is held exactly as stored.
    pub norm16: bool,
    /// The most bytes one texture of a volume may take; a longer series is held in several.
    pub texture_bytes: u64,
    /// Whether the device's memory is the computer's own, as an integrated card's is: what it
    /// holds then counts against the memory budget.
    pub shares_memory: bool,
}

impl Default for CardRoom {
    /// What any device can do.
    fn default() -> Self {
        Self {
            longest_side: 2048,
            most_bytes: None,
            norm16: false,
            texture_bytes: MAX_TEXTURE_BYTES,
            shares_memory: false,
        }
    }
}

impl CardRoom {
    /// What `device` can hold, given at most `most_bytes`.
    pub fn of(device: &wgpu::Device, most_bytes: Option<u64>) -> Self {
        Self {
            longest_side: device.limits().max_texture_dimension_3d,
            most_bytes,
            norm16: device
                .features()
                .contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM),
            texture_bytes: MAX_TEXTURE_BYTES,
            shares_memory: false,
        }
    }

    /// How many times a volume of `dims` must be halved, each way, for the device to hold
    /// it: 0 when it fits, in up to four textures, within the room the device has.
    pub fn halvings(&self, dims: [u32; 3]) -> u32 {
        let (mut dims, mut halvings) = (dims, 0);
        while SlabPlan::for_volume(dims, self).is_none() && dims.iter().any(|side| *side > 1) {
            dims = dims.map(|side| side.div_ceil(2));
            halvings += 1;
        }
        halvings
    }

    /// How many textures the device holds a volume of `dims` in; `None` when it cannot hold
    /// it without halving it.
    pub fn textures(&self, dims: [u32; 3]) -> Option<usize> {
        SlabPlan::for_volume(dims, self).map(|plan| plan.count())
    }

    /// The format a volume held as `stored` takes on the device.
    pub(crate) fn format(&self, stored: Stored) -> wgpu::TextureFormat {
        if self.norm16 && stored != Stored::Float32 {
            wgpu::TextureFormat::R16Unorm
        } else {
            wgpu::TextureFormat::R16Float
        }
    }
}

/// The value a texel of 0 stands for, and the span to the value a texel of 1 stands for.
pub(crate) type Mapping = (f32, f32);

/// How the texels of `held`, in `format`, stand for its values: a texel `t` of 0 to 1 is the
/// value `smallest + t × span`. As stored, a texel is a stored number over 65,535 (a signed
/// one moved up by 32,768 first); as a 16-bit float, it is how far the value lies from the
/// smallest of the volume's values towards the largest.
pub(crate) fn mapping(held: &HeldVolume, format: wgpu::TextureFormat) -> Mapping {
    let rescale = held.rescale();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a 32-bit value is what the scan's values are read as"
    )]
    let narrowed = |value: f64| value as f32;
    match (format, held.stored()) {
        (wgpu::TextureFormat::R16Unorm, Stored::Unsigned16) => {
            (narrowed(rescale.intercept), narrowed(TOP * rescale.slope))
        }
        (wgpu::TextureFormat::R16Unorm, Stored::Signed16) => (
            narrowed(rescale.intercept - SIGNED_OFFSET * rescale.slope),
            narrowed(TOP * rescale.slope),
        ),
        _ => {
            let (smallest, largest) = value_range(held).unwrap_or((0.0, 0.0));
            let span = if largest > smallest {
                largest - smallest
            } else {
                1.0
            };
            (smallest, span)
        }
    }
}

/// How the device holds the values of `held` on a device with `room`: the value a stored 0
/// stands for and the span to the one a stored 1 stands for. Coefficients worked out away
/// from the device for planes drawn smooth are stored the same way.
pub fn card_mapping(held: &HeldVolume, room: &CardRoom) -> (f32, f32) {
    mapping(held, room.format(held.stored()))
}

/// The smallest and largest values of a held volume that are numbers. `None` when none is.
fn value_range(held: &HeldVolume) -> Option<(f32, f32)> {
    let rescale = held.rescale();
    let mut found: Option<(f32, f32)> = None;
    let mut take = |value: f32| {
        if value.is_finite() {
            let (low, high) = found.unwrap_or((value, value));
            found = Some((low.min(value), high.max(value)));
        }
    };
    for slab in 0..held.slab_count() {
        held.with_slab(slab, |raw| match raw {
            SlabRaw::Unsigned(numbers) => {
                let low = numbers.iter().min().copied();
                let high = numbers.iter().max().copied();
                for number in low.into_iter().chain(high) {
                    take(rescale.value(i32::from(number)));
                }
            }
            SlabRaw::Signed(numbers) => {
                let low = numbers.iter().min().copied();
                let high = numbers.iter().max().copied();
                for number in low.into_iter().chain(high) {
                    take(rescale.value(i32::from(number)));
                }
            }
            SlabRaw::Float(values) => values.iter().copied().for_each(&mut take),
        });
    }
    found
}

/// The texels of a slab in `format`, written into `into` (emptied first): as stored for
/// 16-bit normalised texels (a signed number moved up by 32,768), and otherwise each value as
/// a 16-bit float by `mapping`, as [`mapping`] gives it.
pub(crate) fn texels(
    raw: SlabRaw<'_>,
    held: &HeldVolume,
    (format, (smallest, span)): (wgpu::TextureFormat, Mapping),
    into: &mut Vec<u16>,
) {
    into.clear();
    let rescale = held.rescale();
    let half = |value: f32| as_half(value, smallest, span);
    let as_stored = format == wgpu::TextureFormat::R16Unorm;
    match raw {
        SlabRaw::Unsigned(numbers) if as_stored => into.extend_from_slice(numbers),
        SlabRaw::Signed(numbers) if as_stored => {
            into.extend(numbers.iter().map(|n| n.cast_unsigned() ^ 0x8000));
        }
        SlabRaw::Unsigned(numbers) => {
            into.extend(numbers.iter().map(|n| half(rescale.value(i32::from(*n)))));
        }
        SlabRaw::Signed(numbers) => {
            into.extend(numbers.iter().map(|n| half(rescale.value(i32::from(*n)))));
        }
        SlabRaw::Float(values) => into.extend(values.iter().map(|value| half(*value))),
    }
}

/// A texel as the blocks of largest values hold it: a 16-bit float from 0 to 1.
pub(crate) fn as_block_value(texel: u16, format: wgpu::TextureFormat) -> u16 {
    if format == wgpu::TextureFormat::R16Unorm {
        f16::from_f64(f64::from(texel) / TOP).to_bits()
    } else {
        texel
    }
}

#[cfg(test)]
mod tests;
