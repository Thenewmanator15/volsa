//! Handing a held volume to the device a slab at a time, a little each frame, from the one
//! thread that draws: begun by [`VolumePipeline::start_upload`], carried on by
//! [`VolumePipeline::continue_upload`] until the volume is there to draw.

use std::ops::Range;
use std::sync::Arc;

use egui_wgpu::wgpu;
use voxa_core::{Grid, HeldVolume, SlabRaw};

use super::{CardRoom, Mapping, SlabPlan, as_block_value, mapping, texels};
use crate::lost;
use crate::on_device::{BlockMaxima, BlocksMaker};
use crate::slice_pipeline::RenderError;
use crate::volume_pipeline::VolumePipeline;
use crate::volume_texture::{BLOCK, Placed, texture, write};

/// How far handing a volume to the device has got.
#[derive(Debug, PartialEq, Eq)]
pub enum UploadState {
    /// Nothing is being handed over.
    Idle,
    /// Some of it is on the device: `done` of `of` bytes.
    Going {
        /// Bytes handed over so far.
        done: u64,
        /// Bytes in all.
        of: u64,
    },
    /// All of it is there, and it is the volume drawn.
    Done,
    /// It could not be handed over, for the reason given; nothing of it is drawn.
    Failed(RenderError),
}

/// A volume being handed to the device.
pub(crate) struct Upload {
    held: Arc<HeldVolume>,
    /// The textures its values go into, cut along its slices as `plan` says; given up if
    /// the volume never gets there.
    values: Vec<wgpu::Texture>,
    plan: SlabPlan,
    format: wgpu::TextureFormat,
    mapping: Mapping,
    /// The next slab to hand over.
    next: usize,
    done: u64,
    of: u64,
    /// The texels of the slab in hand, where they are not the values as held, kept between
    /// slabs so as not to be made again.
    scratch: Vec<u16>,
    /// The largest texel in each block, gathered here for a device that cannot.
    blocks: Option<BlockMaxima>,
}

impl Drop for Upload {
    /// Textures never drawn from are given back to the device at once.
    fn drop(&mut self) {
        self.values.drain(..).for_each(|values| values.destroy());
    }
}

impl VolumePipeline {
    /// Begin handing `held` to the device, in place of any volume handed over or being
    /// handed over before, which is let go first so that the device has room. Nothing is
    /// drawn until [`VolumePipeline::continue_upload`] says it is done.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoRoom`] when a slice is wider or taller than the device allows, the
    /// series is too long for four textures, or the device has no room for them.
    pub fn start_upload(
        &mut self,
        device: &wgpu::Device,
        held: Arc<HeldVolume>,
        room: CardRoom,
    ) -> Result<(), RenderError> {
        self.upload = None;
        self.uploaded = None;
        let dims = held.dims();
        let room = CardRoom {
            longest_side: (room.longest_side).min(device.limits().max_texture_dimension_3d),
            ..room
        };
        let plan = SlabPlan::for_volume(dims, &room).ok_or(RenderError::NoRoom)?;
        let format = room.format(held.stored());
        let values = textures(device, (&plan, dims), format).ok_or(RenderError::NoRoom)?;
        let of = dims.into_iter().map(u64::from).product::<u64>() * 2;
        let blocks = self
            .blocks_maker(device)
            .is_none()
            .then(|| BlockMaxima::new(dims));
        self.upload = Some(Upload {
            mapping: mapping(&held, format),
            held,
            values,
            plan,
            format,
            next: 0,
            done: 0,
            of,
            scratch: Vec::new(),
            blocks,
        });
        Ok(())
    }

    /// Hand the device the next slabs of the volume being handed over, until at least
    /// `at_most_bytes` have gone or none is left, and say how far it has got. When the last
    /// is there the volume becomes the one drawn.
    pub fn continue_upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        at_most_bytes: u64,
    ) -> UploadState {
        let Some(upload) = self.upload.as_mut() else {
            return UploadState::Idle;
        };
        let mut handed = 0;
        while upload.next < upload.held.slab_count() && handed < at_most_bytes {
            handed += upload.hand_slab(queue);
        }
        // Sent on its way now, so that the device holds no more than this frame's bytes.
        queue.submit(std::iter::empty());
        if lost::poll(device, wgpu::PollType::Poll).is_none() {
            self.upload = None;
            return UploadState::Failed(RenderError::DeviceLost);
        }
        if upload.next < upload.held.slab_count() {
            return UploadState::Going {
                done: upload.done,
                of: upload.of,
            };
        }
        let Some(mut upload) = self.upload.take() else {
            return UploadState::Idle;
        };
        let scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let placed = upload.finish((device, queue), self.blocks_maker(device));
        if pollster::block_on(scope.pop()).is_some() {
            return UploadState::Failed(RenderError::NoRoom);
        }
        match placed {
            Some(placed) => {
                self.show(device, placed);
                UploadState::Done
            }
            None => UploadState::Failed(RenderError::NoRoom),
        }
    }
}

impl Upload {
    /// Hand the next slab to the device; the bytes handed over.
    fn hand_slab(&mut self, queue: &wgpu::Queue) -> u64 {
        let slab = self.next;
        self.next += 1;
        let slices = self.held.slab_slices(slab);
        let (held, scratch, blocks) = (&self.held, &mut self.scratch, &mut self.blocks);
        let to = (self.format, self.mapping);
        let as_stored = to.0 == wgpu::TextureFormat::R16Unorm;
        let into = (self.values.as_slice(), &self.plan);
        let handed = held.with_slab(slab, |raw| {
            // Unsigned values are handed over as they are held, with no copy made.
            let numbers: &[u16] = match raw {
                SlabRaw::Unsigned(numbers) if as_stored => numbers,
                _ => {
                    texels(raw, held, to, scratch);
                    scratch
                }
            };
            hand_texels(queue, into, held.dims(), slices.clone(), numbers);
            if let Some(blocks) = blocks {
                blocks.add(slices.start, numbers);
            }
            numbers.len() as u64 * 2
        });
        // A slab never put is left as the textures start: nought.
        let bytes = handed.unwrap_or(0);
        self.done += bytes;
        bytes
    }

    /// The volume as the device now holds it, with the largest value in each block worked
    /// out by the device or from what was gathered here. `None` if its texture is gone.
    fn finish(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        maker: Option<&BlocksMaker>,
    ) -> Option<Placed> {
        let values = std::mem::take(&mut self.values);
        if values.is_empty() {
            return None;
        }
        let views: Vec<wgpu::TextureView> = values
            .iter()
            .map(|values| values.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();
        let dims = self.held.dims();
        let blocks = dims.map(|count| count.div_ceil(BLOCK));
        let largest = match (maker, self.blocks.take()) {
            (Some(maker), _) => maker.make((device, queue), (&views, self.plan.step), blocks),
            (None, gathered) => {
                let format = self.format;
                let maxima: Vec<u16> = gathered
                    .map(BlockMaxima::finish)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|texel| as_block_value(texel, format))
                    .collect();
                let halves = wgpu::TextureFormat::R16Float;
                let largest = texture(device, "voxa volume block maxima", blocks, halves);
                write(device, queue, &largest, blocks, &maxima);
                largest
            }
        };
        let finest_mm = self
            .held
            .spacing_mm()
            .into_iter()
            .fold(f64::INFINITY, f64::min);
        let to_voxel = self.held.patient_to_voxel();
        Placed::from_parts(
            (values, views, largest),
            (self.plan.clone(), dims),
            (to_voxel, finest_mm),
            self.mapping,
        )
    }
}

/// The textures of a volume of `dims` cut as `plan` says, in `format`. `None`, with any made
/// given back, when the device has no room for them.
fn textures(
    device: &wgpu::Device,
    (plan, [cols, rows, _]): (&SlabPlan, [u32; 3]),
    format: wgpu::TextureFormat,
) -> Option<Vec<wgpu::Texture>> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let made: Vec<wgpu::Texture> = plan
        .depths
        .iter()
        .map(|&depth| texture(device, "voxa volume values", [cols, rows, depth], format))
        .collect();
    if pollster::block_on(scope.pop()).is_some() {
        made.iter().for_each(wgpu::Texture::destroy);
        return None;
    }
    Some(made)
}

/// Write whole slices of texels of a volume of `dims`, the first of them its slice
/// `slices.start`, into each of its textures, cut as `plan` says, that holds any of them.
fn hand_texels(
    queue: &wgpu::Queue,
    (values, plan): (&[wgpu::Texture], &SlabPlan),
    dims: [u32; 3],
    slices: Range<u32>,
    texels: &[u16],
) {
    let in_a_slice = dims[0] as usize * dims[1] as usize;
    for (slab, texture) in values.iter().enumerate() {
        let holds = plan.slices(slab);
        let (first, end) = (slices.start.max(holds.start), slices.end.min(holds.end));
        if first >= end {
            continue;
        }
        let from = (first - slices.start) as usize * in_a_slice;
        let to = (end - slices.start) as usize * in_a_slice;
        if let Some(part) = texels.get(from..to) {
            write_slices(queue, texture, dims, first - holds.start, part);
        }
    }
}

/// Write whole slices of texels into a 3D texture of `dims`, the first at slice `first`.
fn write_slices(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    dims: [u32; 3],
    first: u32,
    texels: &[u16],
) {
    let [cols, rows, _] = dims;
    let in_a_slice = cols as usize * rows as usize;
    let Ok(slices) = u32::try_from(texels.len() / in_a_slice.max(1)) else {
        return;
    };
    if slices == 0 {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: first,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(texels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(cols * 2),
            rows_per_image: Some(rows),
        },
        wgpu::Extent3d {
            width: cols,
            height: rows,
            depth_or_array_layers: slices,
        },
    );
}
