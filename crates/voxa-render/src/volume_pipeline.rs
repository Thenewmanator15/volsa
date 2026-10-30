//! The GPU pipeline that draws a volume by following a line of sight through it for every
//! pixel.

use egui_wgpu::wgpu;
use voxa_core::Volume;
use voxa_volume::halvings_needed;

use std::sync::OnceLock;

use crate::gpu::Gpu;
use crate::on_device::{BlocksMaker, CurveMaker};
use crate::prepared::{MAX_VOLUME_BYTES, Prepared};
use crate::scene::SceneParams;
use crate::slice_pipeline::RenderError;
use crate::smoothed::SmoothedTexture;
use crate::upload::Upload;
use crate::volume_params::{Params, shader_params};
use crate::volume_texture::{Placed, placed_by_halving};

/// The format of a picture read back with [`VolumePipeline::picture`]: 8-bit red, green, blue
/// and alpha, with no gamma curve, so the grey read back is the grey the shader wrote.
pub const PICTURE_FORMAT: wgpu::TextureFormat = crate::gpu::TARGET;

/// The volume now on the graphics device.
pub(crate) struct Uploaded {
    placed: Placed,
    bind_group: wgpu::BindGroup,
}

/// Draws a volume. Its values are uploaded once as a 3D texture; the camera, window and look
/// are a small block of numbers the shader reads, so changing them never uploads the volume
/// again.
pub struct VolumePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    blended: wgpu::Sampler,
    pub(crate) uploaded: Option<Uploaded>,
    /// A held volume being handed over a little each frame, if one is.
    pub(crate) upload: Option<Upload>,
    /// What works out, on the device, the largest value in each block of a volume, and
    /// the curve through its values: each made when first wanted, and `None` on a device
    /// that cannot.
    blocks: OnceLock<Option<BlocksMaker>>,
    curves: OnceLock<Option<CurveMaker>>,
    /// How many volumes have been shown, so that what draws from the same texture can tell
    /// when it has been replaced.
    uploads: u64,
}

impl VolumePipeline {
    /// Build the pipeline for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("volume.wgsl"));
        let layout = bindings(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxa volume"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxa volume"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(target.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxa volume parameters"),
            size: size_of::<Params>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Values between voxels are blended by the device; past the last voxel its own value
        // carries on to the face of the volume.
        let blended = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("voxa volume"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            params,
            blended,
            uploaded: None,
            upload: None,
            blocks: OnceLock::new(),
            curves: OnceLock::new(),
            uploads: 0,
        }
    }

    /// Put a volume on the graphics device, replacing any volume already there. A volume
    /// larger than the device can hold is halved, each way, as often as it takes; the number
    /// of halvings is returned, 0 for a volume shown whole.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoRoom`] when the device has no room even for a single voxel. No volume
    /// is then left uploaded: the one there before is given up first, to make room.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        volume: &Volume,
    ) -> Result<u32, RenderError> {
        self.upload_within(device, queue, volume, u32::MAX, MAX_VOLUME_BYTES)
    }

    /// As [`VolumePipeline::upload`], with the volume first halved until no side is longer
    /// than `max_side` voxels (or than the device's own longest side, if that is shorter) and
    /// it takes no more than `max_bytes`.
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload`].
    pub fn upload_within(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        volume: &Volume,
        max_side: u32,
        max_bytes: u64,
    ) -> Result<u32, RenderError> {
        let max_side = max_side.min(device.limits().max_texture_dimension_3d);
        let halvings = halvings_needed(volume.dims(), max_side, max_bytes);
        // The volume there now goes first, so that the device has its room for this one.
        self.uploaded = None;
        self.upload = None;
        let place = |shown: &Volume| Placed::new(device, queue, shown);
        let (placed, halvings) =
            placed_by_halving(volume, halvings, place).ok_or(RenderError::NoRoom)?;
        self.show(device, placed);
        Ok(halvings)
    }

    /// Put a volume that was made ready beforehand on the graphics device, replacing any
    /// volume already there, and say how many times it had been halved. This is quick: the
    /// slow part was making it ready, which can be done on another thread.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoRoom`] when the volume was made ready for a larger device than this
    /// one, or the device has no room for it; [`VolumePipeline::upload`] can then be tried
    /// with the volume itself. When the device has no room, no volume is left uploaded.
    pub fn upload_prepared(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ready: &Prepared,
    ) -> Result<u32, RenderError> {
        if too_long(ready.dims(), device.limits().max_texture_dimension_3d) {
            return Err(RenderError::NoRoom);
        }
        // The volume there now goes first, so that the device has its room for this one.
        self.uploaded = None;
        self.upload = None;
        let blocks = self.blocks_maker(device);
        let placed =
            Placed::from_prepared(device, queue, ready, blocks).ok_or(RenderError::NoRoom)?;
        self.show(device, placed);
        Ok(ready.halved())
    }

    /// Draw from a volume already on the graphics device, in place of any shown before.
    pub(crate) fn show(&mut self, device: &wgpu::Device, placed: Placed) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxa volume"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&placed.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.blended),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&placed.maxima),
                },
            ],
        });
        self.uploaded = Some(Uploaded { placed, bind_group });
        self.uploads += 1;
    }

    pub(crate) fn blocks_maker(&self, device: &wgpu::Device) -> Option<&BlocksMaker> {
        self.blocks
            .get_or_init(|| BlocksMaker::new(device))
            .as_ref()
    }

    /// Whether this device can work out the largest value in each block of a volume from
    /// the values it is given, so that a volume can be made ready for it without them
    /// ([`Prepared::without_blocks`]).
    pub fn makes_blocks(&self, device: &wgpu::Device) -> bool {
        self.blocks_maker(device).is_some()
    }

    /// Whether this device can work out the curve through a volume it holds.
    pub fn makes_curves(&self, device: &wgpu::Device) -> bool {
        self.curves
            .get_or_init(|| CurveMaker::new(device))
            .is_some()
    }

    /// The curve through the volume on the device now, worked out by the device itself.
    /// `None` with no volume there, on a device that cannot, and when it has no room.
    pub fn curve_on_device(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<SmoothedTexture> {
        let (placed, _) = self.placed()?;
        let maker = self
            .curves
            .get_or_init(|| CurveMaker::new(device))
            .as_ref()?;
        maker.make((device, queue), &placed.view, placed.dims)
    }

    /// The volume on the graphics device now, and which upload it is.
    pub(crate) fn placed(&self) -> Option<(&Placed, u64)> {
        let uploaded = self.uploaded.as_ref()?;
        Some((&uploaded.placed, self.uploads))
    }

    /// Whether a volume has been uploaded.
    pub fn has_volume(&self) -> bool {
        self.uploaded.is_some()
    }

    /// Set what the next picture is drawn from. `viewport` is the width and height, in
    /// pixels, of the picture. Does nothing until a volume has been uploaded.
    pub fn set_params(&mut self, queue: &wgpu::Queue, params: &SceneParams, viewport: [f32; 2]) {
        let Some(uploaded) = &self.uploaded else {
            return;
        };
        let params = shader_params(&uploaded.placed, params, viewport);
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
    }

    /// Draw the uploaded volume into a picture `size` (width, height) pixels large, off the
    /// screen, and return it as red, green, blue, alpha bytes, row by row from the top. Every
    /// pixel is an opaque grey. `viewport` is the size the picture is laid out at, which is
    /// what the camera measures in. The pipeline must have been made for [`PICTURE_FORMAT`].
    /// With nothing uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold, and
    /// [`RenderError::Readback`] when the picture cannot be copied back.
    pub fn picture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        params: &SceneParams,
        viewport: [f32; 2],
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        self.set_params(queue, params, viewport);
        Gpu::on(device, queue).picture(size, |pass| self.draw(pass))
    }

    /// Draw the uploaded volume over the whole of the pass's viewport. Draws nothing until a
    /// volume has been uploaded.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(uploaded) = &self.uploaded else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &uploaded.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Whether a volume has a side longer than a device allows.
pub(crate) fn too_long(dims: [u32; 3], longest_allowed: u32) -> bool {
    dims.into_iter().any(|side| side > longest_allowed)
}

/// What the shader is given: the parameters, the volume as a texture, the way values between
/// its voxels are blended, and the largest value in each block of the volume.
fn bindings(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let parameters = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let volume = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D3,
        multisampled: false,
    };
    let blended = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("voxa volume"),
        entries: &[
            entry(0, parameters),
            entry(1, volume),
            entry(2, blended),
            entry(3, volume),
        ],
    })
}
