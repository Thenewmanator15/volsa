//! The curve through a volume's values, for planes drawn smooth: the coefficients of a cubic
//! B-spline, made ready away from the interface's thread and held by the graphics device
//! beside the volume itself.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use egui_wgpu::wgpu;
use half::f16;
use voxa_core::Volume;
use voxa_volume::{coefficients, halved, halvings_needed};

use crate::prepared::stored_range;
use crate::volume_texture::{texture, write};

/// How many curves have been put on a device, so that each can be told from the others.
static PLACED: AtomicU64 = AtomicU64::new(0);

/// The curve through a volume as the graphics device will hold it: a coefficient for every
/// voxel, stored as the volume's own values are, from 0 at its smallest value to 1 at its
/// largest. A coefficient can lie a little outside that span, and is kept as it is.
pub struct Smoothed {
    halves: Vec<u16>,
    dims: [u32; 3],
}

impl Smoothed {
    /// Work out the curve through a volume as a device holds that volume: halved as often
    /// as [`Prepared::of`](crate::Prepared::of) halves it for the same `max_side` and
    /// `max_bytes`. This reads every value several times over and is not quick.
    pub fn of(volume: &Volume, max_side: u32, max_bytes: u64) -> Self {
        let mut shown = Cow::Borrowed(volume);
        for _ in 0..halvings_needed(volume.dims(), max_side, max_bytes) {
            shown = Cow::Owned(halved(&shown));
        }
        let (smallest, span) = stored_range(&shown);
        let stored = |coefficient: f32| {
            let part = (coefficient - smallest) / span;
            f16::from_f32(if part.is_finite() { part } else { 0.0 }).to_bits()
        };
        Self {
            halves: coefficients(&shown).into_iter().map(stored).collect(),
            dims: shown.dims(),
        }
    }

    /// Columns, rows and slices of the volume this is the curve through, as it is held.
    pub fn dims(&self) -> [u32; 3] {
        self.dims
    }

    /// The coefficients as they are stored.
    #[cfg(test)]
    pub(crate) fn halves(&self) -> &[u16] {
        &self.halves
    }

    /// Put the curve on a graphics device. `None` when the device has no room for it.
    pub fn on(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<SmoothedTexture> {
        // Watched for running out of room, as the volume itself is.
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let halves = wgpu::TextureFormat::R16Float;
        let coefficients = texture(device, "voxa volume curve", self.dims, halves);
        write(device, queue, &coefficients, self.dims, &self.halves);
        if pollster::block_on(room.pop()).is_some() {
            coefficients.destroy();
            return None;
        }
        Some(SmoothedTexture::holding(coefficients, self.dims))
    }
}

impl std::fmt::Debug for Smoothed {
    /// Prints the size, not the coefficients: there are millions of them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut shown = f.debug_struct("Smoothed");
        shown.field("dims", &self.dims);
        shown.finish_non_exhaustive()
    }
}

/// The texture itself, given back to the device as soon as nothing draws from it.
struct Held {
    coefficients: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Drop for Held {
    /// Give the device its memory back now, not whenever it gets round to it.
    fn drop(&mut self) {
        self.coefficients.destroy();
    }
}

/// The curve through a volume on a graphics device. It is as large as the volume, so it is
/// put there once and shared: a copy of this is another hold on the same texture.
#[derive(Clone)]
pub struct SmoothedTexture {
    held: Arc<Held>,
    dims: [u32; 3],
    number: u64,
}

impl SmoothedTexture {
    /// The curve through a volume of `dims` voxels, whose coefficients `coefficients` holds
    /// or is about to: the graphics device may still be working them out.
    pub(crate) fn holding(coefficients: wgpu::Texture, dims: [u32; 3]) -> Self {
        let view = coefficients.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            held: Arc::new(Held { coefficients, view }),
            dims,
            number: PLACED.fetch_add(1, Ordering::Relaxed) + 1,
        }
    }

    /// The texture itself, for reading the coefficients back.
    #[cfg(test)]
    pub(crate) fn texture(&self) -> &wgpu::Texture {
        &self.held.coefficients
    }

    /// The coefficients as a shader reads them.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.held.view
    }

    /// Columns, rows and slices of the volume this is the curve through.
    pub(crate) fn dims(&self) -> [u32; 3] {
        self.dims
    }

    /// A number no other curve put on a device has.
    pub(crate) fn number(&self) -> u64 {
        self.number
    }
}
