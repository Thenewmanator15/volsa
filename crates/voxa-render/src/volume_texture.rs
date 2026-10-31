//! A volume on the graphics device: its values as a 3D texture, or up to four cut along its
//! slices, reduced if it has to be, and beside them the largest value in each block of
//! voxels, for stepping over empty space.

use std::borrow::Cow;

use egui_wgpu::wgpu;
use voxa_core::{Affine, Patient, Volume, Voxel};
use voxa_volume::halved;

use crate::on_device::{BlocksMaker, largest_in_blocks};
pub(crate) use crate::prepared::BLOCK;
use crate::prepared::Prepared;
use crate::upload::SlabPlan;

/// The shaders' function that reads a value from a volume held in one texture or several.
const SLABS: &str = include_str!("slabs.wgsl");
/// The bindings of the textures after the first, which the shaders' own binding 1 holds.
const MORE_SLABS: [u32; 3] = [10, 11, 12];

/// The most bytes of slices handed to the device in one piece.
const SLAB_BYTES: usize = 16 << 20;
const BYTES_PER_VOXEL: u32 = 2;

/// A volume as the graphics device holds it, and what is needed to find a point of the
/// patient in it and to turn a stored value back into the scan's own.
pub(crate) struct Placed {
    /// The textures themselves, kept so that their memory can be given back at once.
    values: Vec<wgpu::Texture>,
    largest: wgpu::Texture,
    /// The values as the shaders read them, one view a texture: the first, the whole volume
    /// when it is held in one, and any after it. The textures last as long as these do.
    view: wgpu::TextureView,
    more: Vec<wgpu::TextureView>,
    /// How the volume is cut into those textures along its slices.
    pub(crate) plan: SlabPlan,
    /// The largest value in each block of [`BLOCK`] voxels a side and in the blocks touching
    /// it, stored as the values are.
    pub(crate) maxima: wgpu::TextureView,
    /// Columns, rows and slices as stored, after any halving.
    pub(crate) dims: [u32; 3],
    pub(crate) patient_to_voxel: Affine<Patient, Voxel>,
    /// The smallest distance between neighbouring voxels as stored, in millimetres.
    pub(crate) finest_mm: f64,
    /// The scan value stored as 0.
    pub(crate) smallest: f32,
    /// The span of scan values from the one stored as 0 to the one stored as 1.
    pub(crate) span: f32,
}

impl Placed {
    /// Put a volume on the graphics device as it is. `None` when the device has no room.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, volume: &Volume) -> Option<Self> {
        Self::from_prepared(device, queue, &Prepared::as_it_is(volume, 0), None)
    }

    /// As [`Placed::new`], with the largest scan value in each block, and the number of
    /// blocks each way, given and not worked out.
    #[cfg(test)]
    pub(crate) fn with_maxima(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        volume: &Volume,
        maxima: (&[f32], [u32; 3]),
    ) -> Option<Self> {
        Self::from_prepared(
            device,
            queue,
            &Prepared::with_maxima(volume, maxima, 0),
            None,
        )
    }

    /// Put a volume that has been made ready on the graphics device: all that is left to do
    /// is hand over the bytes. Where the largest value in each block was left to be worked
    /// out, `blocks` does it on the device, or, with no `blocks`, it is done here from the
    /// values as they are stored. `None` when the device has no room.
    pub(crate) fn from_prepared(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ready: &Prepared,
        blocks: Option<&BlocksMaker>,
    ) -> Option<Self> {
        // Watched for running out of room from making the textures to the last byte handed
        // over: left unwatched, either would stop the program.
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let halves = wgpu::TextureFormat::R16Float;
        // Handed over ahead of time, on another thread, or handed over here and now.
        let values = match ready.handed.take() {
            Some(values) => values,
            None => values_on(device, queue, ready),
        };
        let view = values.create_view(&wgpu::TextureViewDescriptor::default());
        let largest = match (&ready.maxima, blocks) {
            (None, Some(blocks)) => {
                let whole = (std::slice::from_ref(&view), ready.dims[2]);
                blocks.make((device, queue), whole, ready.blocks)
            }
            (worked_out, _) => {
                let largest = texture(device, "voxa volume block maxima", ready.blocks, halves);
                let here = || largest_in_blocks(&ready.halves, ready.dims);
                let maxima = worked_out.as_ref().map_or_else(here, Clone::clone);
                write(device, queue, &largest, ready.blocks, &maxima);
                largest
            }
        };
        if pollster::block_on(room.pop()).is_some() {
            values.destroy();
            largest.destroy();
            return None;
        }
        Some(Self {
            view,
            more: Vec::new(),
            maxima: largest.create_view(&wgpu::TextureViewDescriptor::default()),
            values: vec![values],
            largest,
            plan: SlabPlan::whole(ready.dims[2]),
            dims: ready.dims,
            patient_to_voxel: ready.patient_to_voxel,
            finest_mm: ready.finest_mm,
            smallest: ready.smallest,
            span: ready.span,
        })
    }
}

impl Placed {
    /// A volume already on the device: its values textures, cut as `plan` says, with their
    /// views, and its blocks of largest values; its size, its place in the patient with the
    /// smallest distance between its voxels, and how a texel turns back into a value. `None`
    /// with no textures.
    pub(crate) fn from_parts(
        (values, views, largest): (Vec<wgpu::Texture>, Vec<wgpu::TextureView>, wgpu::Texture),
        (plan, dims): (SlabPlan, [u32; 3]),
        (patient_to_voxel, finest_mm): (Affine<Patient, Voxel>, f64),
        (smallest, span): (f32, f32),
    ) -> Option<Self> {
        let maxima = largest.create_view(&wgpu::TextureViewDescriptor::default());
        let mut views = views.into_iter();
        let view = views.next()?;
        Some(Self {
            values,
            largest,
            view,
            more: views.collect(),
            plan,
            maxima,
            dims,
            patient_to_voxel,
            finest_mm,
            smallest,
            span,
        })
    }

    /// The texture that holds the largest value in each block, for reading it back.
    #[cfg(test)]
    pub(crate) fn largest(&self) -> &wgpu::Texture {
        &self.largest
    }

    /// The first texture of values as a shader reads it: the whole volume when it is held in
    /// one.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The textures after the first, bound where the shaders' `slabs.wgsl` reads them; the
    /// first stands in for those a volume does not have.
    pub(crate) fn more_slabs(&self) -> [wgpu::BindGroupEntry<'_>; 3] {
        MORE_SLABS.map(|binding| {
            let after_the_first = (binding - MORE_SLABS[0]) as usize;
            let view = self.more.get(after_the_first).unwrap_or(&self.view);
            wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            }
        })
    }
}

/// The bindings of the textures after the first, for a shader's layout.
pub(crate) fn more_slabs_layout() -> [wgpu::BindGroupLayoutEntry; 3] {
    MORE_SLABS.map(|binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        count: None,
    })
}

/// A shader that reads a volume, made with the function that reads it from its slabs.
pub(crate) fn shader_reading_slabs(
    device: &wgpu::Device,
    label: &str,
    source: &str,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{SLABS}\n{source}").into()),
    })
}

impl Drop for Placed {
    /// Give the device its memory back now. Left to itself the device frees a texture some
    /// time later, and a second large volume put there in the meantime may not fit.
    fn drop(&mut self) {
        self.values.iter().for_each(wgpu::Texture::destroy);
        self.largest.destroy();
    }
}

/// The values of a volume made ready, in a texture of their own on the device.
pub(crate) fn values_on(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ready: &Prepared,
) -> wgpu::Texture {
    let halves = wgpu::TextureFormat::R16Float;
    let values = texture(device, "voxa volume values", ready.dims, halves);
    write(device, queue, &values, ready.dims, &ready.halves);
    values
}

/// A 3D texture of 16-bit numbers of one kind, `dims` texels each way.
pub(crate) fn texture(
    device: &wgpu::Device,
    label: &str,
    dims: [u32; 3],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let [width, height, depth_or_array_layers] = dims;
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Hand the values to the device a slab of whole slices at a time, sending each on its way
/// before the next, so that the device never holds more than one slab waiting to be copied.
/// Values beyond the last whole slice, if there are any, are left out.
pub(crate) fn write(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    dims: [u32; 3],
    halves: &[u16],
) {
    let answering = || crate::lost::poll(device, wgpu::PollType::Poll).is_some();
    write_slabs(queue, texture, dims, halves, SLAB_BYTES, answering);
}

/// As [`write`], with slabs of at most `slab_bytes`. After each slab is sent on its way,
/// `answering` is asked whether the device still answers, and the writing stops, with the rest
/// of the values left out, the first time it says not: a device that has stopped answering
/// gets nothing more, and is not asked again.
fn write_slabs(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    dims: [u32; 3],
    halves: &[u16],
    slab_bytes: usize,
    mut answering: impl FnMut() -> bool,
) {
    let [cols, rows, slices] = dims;
    let in_a_slice = cols as usize * rows as usize;
    let slab_slices = (slab_bytes / (in_a_slice * BYTES_PER_VOXEL as usize)).max(1);
    let starts = (0..slices).step_by(slab_slices);
    for (first, slab) in starts.zip(halves.chunks(in_a_slice * slab_slices)) {
        let whole_slices = slab.len() / in_a_slice;
        let (Some(slab), Ok(in_this_slab)) = (
            slab.get(..whole_slices * in_a_slice),
            u32::try_from(whole_slices),
        ) else {
            return;
        };
        if in_this_slab == 0 {
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
            bytemuck::cast_slice(slab),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(cols * BYTES_PER_VOXEL),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: in_this_slab,
            },
        );
        // Send this slab on its way and let the device give back what carried the last.
        queue.submit(std::iter::empty());
        if !answering() {
            return;
        }
    }
}

/// Halve `volume` `halvings` times and hand it to `place`. Each time `place` has no room for
/// it, halve it once more and try again, down to a single voxel. Gives what `place` made and
/// how many times the volume was halved; `None` when there was no room even for one voxel.
pub(crate) fn placed_by_halving<T>(
    volume: &Volume,
    halvings: u32,
    mut place: impl FnMut(&Volume) -> Option<T>,
) -> Option<(T, u32)> {
    let mut shown = Cow::Borrowed(volume);
    for _ in 0..halvings {
        shown = Cow::Owned(halved(&shown));
    }
    let mut halvings = halvings;
    loop {
        if let Some(placed) = place(&shown) {
            return Some((placed, halvings));
        }
        if shown.dims() == [1, 1, 1] {
            return None;
        }
        shown = Cow::Owned(halved(&shown));
        halvings += 1;
    }
}

#[cfg(test)]
mod slab_tests;
#[cfg(test)]
mod tests;
