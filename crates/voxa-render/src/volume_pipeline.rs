//! The GPU pipeline that draws a volume by following a line of sight through it for every
//! pixel.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use voxa_core::Volume;
use voxa_volume::halvings_needed;

use crate::scene::{Look, SceneParams};
use crate::slice_pipeline::RenderError;
use crate::volume_texture::{Placed, placed_by_halving};

/// The most a volume may take on the graphics device. A device does not say how much room it
/// has, so a volume is reduced to this before it is tried: 512 MiB, which holds 512 x 512
/// voxels in 1024 slices whole.
const MAX_VOLUME_BYTES: u64 = 512 << 20;
/// How much of the light a millimetre of the brightest tissue stops in the see-through look.
const VR_STOPPED_PER_MM: f32 = 0.05;
/// How much of the light a millimetre of solid tissue stops in the surface look: all but a
/// fiftieth of it within 4 mm, so the first tissue met hides what is behind it.
const SURFACE_STOPPED_PER_MM: f32 = 1.0;
/// The share of the window, up from its low end, over which tissue becomes solid in the
/// surface look. In the see-through look it is the whole window.
const SURFACE_RAMP: f32 = 0.1;
/// The coarsest a picture is drawn: steps ten times as long as at full quality.
const MIN_QUALITY: f32 = 0.1;
/// The narrowest window the shader is given, as a share of the volume's range of values.
const MIN_WINDOW: f32 = 1.0e-6;

/// What the shader needs besides the volume; the layout matches `Scene` in `volume.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    across: [f32; 4],
    along: [f32; 4],
    forward: [f32; 4],
    top_left: [f32; 4],
    to_u: [f32; 4],
    to_v: [f32; 4],
    to_w: [f32; 4],
    view: [f32; 4],
    window: [f32; 4],
    voxel: [f32; 4],
}

/// The volume now on the graphics device.
struct Uploaded {
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
    uploaded: Option<Uploaded>,
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
        }
    }

    /// Put a volume on the graphics device, replacing any volume already there. A volume
    /// larger than the device can hold is halved, each way, as often as it takes; the number
    /// of halvings is returned, 0 for a volume shown whole.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoRoom`] when the device has no room even for a single voxel. The
    /// volume already uploaded, if any, stays in place.
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
        let place = |shown: &Volume| Placed::new(device, queue, shown);
        let (placed, halvings) =
            placed_by_halving(volume, halvings, place).ok_or(RenderError::NoRoom)?;
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
            ],
        });
        self.uploaded = Some(Uploaded { placed, bind_group });
        Ok(halvings)
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

/// The numbers the shader draws one picture from.
fn shader_params(placed: &Placed, scene: &SceneParams, viewport: [f32; 2]) -> Params {
    let to_patient = scene.camera.screen_to_patient(viewport);
    let [across, along, forward] = to_patient.columns().map(four);
    // How far along one axis of the volume a point of the patient is, from 0 to 1: its
    // voxel coordinate, counted from the face half a voxel before the first voxel's centre.
    let to_voxel = placed.patient_to_voxel;
    let along_axis = |pick: fn([f64; 3]) -> f64, count: u32| {
        let [x, y, z] = to_voxel.columns().map(pick);
        let from_face = pick(to_voxel.origin()) + 0.5;
        [x, y, z, from_face].map(|part| single(part / f64::from(count)))
    };
    let [cols, rows, slices] = placed.dims;
    let quality = if scene.quality.is_finite() {
        scene.quality
    } else {
        1.0
    };
    let step_mm = single(placed.finest_mm / 2.0) / quality.clamp(MIN_QUALITY, 1.0);
    // The look as the shader knows it, how much light a millimetre stops, and the ramp.
    let (look, stopped_per_mm, ramp) = match scene.look {
        Look::Vr => (0.0, VR_STOPPED_PER_MM, 1.0),
        Look::Mip => (1.0, 0.0, 1.0),
        Look::Surface => (2.0, SURFACE_STOPPED_PER_MM, SURFACE_RAMP),
    };
    let [wide, high] = viewport;
    let black = (scene.window.low() - placed.smallest) / placed.span;
    let black_to_white = (scene.window.width / placed.span).max(MIN_WINDOW);
    Params {
        across,
        along,
        forward,
        top_left: four(to_patient.origin()),
        to_u: along_axis(|[col, _, _]| col, cols),
        to_v: along_axis(|[_, row, _]| row, rows),
        to_w: along_axis(|[_, _, slice]| slice, slices),
        view: [wide, high, step_mm, look],
        window: [black, black_to_white, stopped_per_mm, ramp],
        voxel: [
            1.0 / cols as f32,
            1.0 / rows as f32,
            1.0 / slices as f32,
            0.0,
        ],
    }
}

/// What the shader is given: the parameters, the volume as a texture, and the way values
/// between its voxels are blended.
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
        entries: &[entry(0, parameters), entry(1, volume), entry(2, blended)],
    })
}

/// Three numbers as the four the shader keeps them in.
fn four([x, y, z]: [f64; 3]) -> [f32; 4] {
    [single(x), single(y), single(z), 0.0]
}

/// A length or coordinate as the single-precision number the shader works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "millimetres and pixels are small"
)]
fn single(value: f64) -> f32 {
    value as f32
}
