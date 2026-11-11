//! Labelled parts on the graphics device: the part at every voxel of a label volume, on that
//! volume's own grid, and a small table of how each part is to look. Changing what is shown,
//! faded or selected rewrites the table and nothing else.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use egui_wgpu::wgpu;
use voxa_core::{Affine, Patient, Voxel};
use voxa_labels::LabelSet;

use crate::volume_texture::{texture, write};

/// How much of the way to a part's colour the scan is tinted inside it, when labels are
/// drawn filled and the part is solid.
pub const FILL: f32 = 1.0 / 3.0;
/// How many parts wide the table of looks is.
const LOOKS_ACROSS: u32 = 256;

/// How many label volumes have been put on a device, so that each can be told from the rest.
static PLACED: AtomicU64 = AtomicU64::new(0);

/// How one part is to look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartLook {
    /// Its colour: red, green, blue.
    pub colour: [u8; 3],
    /// How solid it is, from 0, hidden, to 1.
    pub opacity: f32,
}

/// How labels are drawn over a plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overlay {
    /// Not at all.
    Off,
    /// A line round each part, the scan untouched inside it.
    #[default]
    Outline,
    /// The line, and the scan inside tinted with the part's colour.
    Filled,
}

/// The textures themselves, given back to the device as soon as nothing draws from them.
struct Held {
    parts: wgpu::Texture,
    parts_view: wgpu::TextureView,
    looks: wgpu::Texture,
    looks_view: wgpu::TextureView,
    /// The number of the part selected, counted from 1; 0 when none is.
    selected: AtomicU32,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.parts.destroy();
        self.looks.destroy();
    }
}

/// A label set on a graphics device. It is put there once and shared: a copy of this is
/// another hold on the same textures.
#[derive(Clone)]
pub struct LabelsOnDevice {
    held: Arc<Held>,
    dims: [u32; 3],
    patient_to_voxel: Affine<Patient, Voxel>,
    finest_mm: f64,
    parts: usize,
    number: u64,
}

impl LabelsOnDevice {
    /// Put a label set on a graphics device, every part shown solid in its own colour. `None`
    /// when the device has no room for it, a side of it is longer than the device holds, or
    /// its placement cannot be turned round.
    pub fn put(device: &wgpu::Device, queue: &wgpu::Queue, set: &LabelSet) -> Option<Self> {
        let patient_to_voxel = set.placement.inverse()?;
        let longest = device.limits().max_texture_dimension_3d;
        if set.dims.iter().any(|side| *side == 0 || *side > longest) {
            return None;
        }
        let rows = u32::try_from(set.parts.len()).ok()? / LOOKS_ACROSS + 1;
        // Watched for running out of room, as the volume itself is.
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let kind = wgpu::TextureFormat::R16Uint;
        let parts = texture(device, "voxa label parts", set.dims, kind);
        write(device, queue, &parts, set.dims, &set.numbers);
        let looks = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("voxa label looks"),
            size: wgpu::Extent3d {
                width: LOOKS_ACROSS,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if pollster::block_on(room.pop()).is_some() {
            parts.destroy();
            looks.destroy();
            return None;
        }
        let whole = wgpu::TextureViewDescriptor::default();
        let placed = Self {
            held: Arc::new(Held {
                parts_view: parts.create_view(&whole),
                looks_view: looks.create_view(&whole),
                parts,
                looks,
                selected: AtomicU32::new(0),
            }),
            dims: set.dims,
            patient_to_voxel,
            finest_mm: set.finest_mm(),
            parts: set.parts.len(),
            number: PLACED.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let solid = |part: &voxa_labels::Part| PartLook {
            colour: part.colour,
            opacity: 1.0,
        };
        let as_given: Vec<PartLook> = set.parts.iter().map(solid).collect();
        placed.set_looks(queue, &as_given, None);
        Some(placed)
    }

    /// Say how each part is to look, in the order of the set's parts, and which is selected.
    /// A part with no look given is hidden; a selected part that is not there selects none.
    pub fn set_looks(&self, queue: &wgpu::Queue, looks: &[PartLook], selected: Option<usize>) {
        let rows = self.held.looks.height();
        let count = (LOOKS_ACROSS * rows) as usize;
        // The first texel is for "no part" and stays clear.
        let mut bytes = vec![0_u8; count * 4];
        let (texels, _) = bytes.as_chunks_mut::<4>();
        let slots = texels.iter_mut().skip(1).take(self.parts);
        for (slot, look) in slots.zip(looks) {
            let [red, green, blue] = look.colour;
            let opacity = if look.opacity.is_finite() {
                look.opacity.clamp(0.0, 1.0)
            } else {
                0.0
            };
            *slot = [red, green, blue, byte(opacity * 255.0)];
        }
        queue.write_texture(
            self.held.looks.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(LOOKS_ACROSS * 4),
                rows_per_image: Some(rows),
            },
            self.held.looks.size(),
        );
        let chosen = selected.filter(|part| *part < self.parts.min(looks.len()));
        let number = chosen.and_then(|part| u32::try_from(part + 1).ok());
        self.held
            .selected
            .store(number.unwrap_or(0), Ordering::Relaxed);
    }

    /// The part at each voxel, as a shader reads it.
    pub(crate) fn parts_view(&self) -> &wgpu::TextureView {
        &self.held.parts_view
    }

    /// The table of looks, as a shader reads it.
    pub(crate) fn looks_view(&self) -> &wgpu::TextureView {
        &self.held.looks_view
    }

    /// Columns, rows and slices of the label volume.
    pub(crate) fn dims(&self) -> [u32; 3] {
        self.dims
    }

    /// The least distance between neighbouring label voxels, in millimetres.
    pub(crate) fn finest_mm(&self) -> f64 {
        self.finest_mm
    }

    /// From a point of the patient to a place in the label volume, in voxels.
    pub(crate) fn patient_to_voxel(&self) -> Affine<Patient, Voxel> {
        self.patient_to_voxel
    }

    /// The number of the part selected, counted from 1; 0 when none is.
    pub(crate) fn selected(&self) -> u32 {
        self.held.selected.load(Ordering::Relaxed)
    }

    /// A number no other label set put on a device has.
    pub(crate) fn number(&self) -> u64 {
        self.number
    }
}

/// A share of 255 as a byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn byte(share: f32) -> u8 {
    share.round().clamp(0.0, 255.0) as u8
}
