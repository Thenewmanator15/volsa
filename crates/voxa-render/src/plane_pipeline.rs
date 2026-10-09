//! The GPU pipeline that draws a plane through the volume the scene holds. It reads the
//! scene's own texture, so a plane view costs no second copy of the volume and moving through
//! its layers decodes nothing.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use voxa_core::WindowLevel;

use crate::plane_view::PlaneView;
use crate::volume_params::{MIN_WINDOW, texture_rows};
use crate::volume_pipeline::VolumePipeline;

/// What the shader needs besides the volume; the layout matches `View` in `plane.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    across: [f32; 4],
    down: [f32; 4],
    top_left: [f32; 4],
    to_u: [f32; 4],
    to_v: [f32; 4],
    to_w: [f32; 4],
    view: [f32; 4],
}

/// The volume a plane is drawn from now.
struct Shown {
    bind_group: wgpu::BindGroup,
    /// Which upload of the volume pipeline's this is, so that a newer one is noticed.
    upload: u64,
    rows: [[f32; 4]; 3],
    /// The scan value stored as 0, and the span from it to the one stored as 1.
    smallest: f32,
    span: f32,
}

/// Draws one plane through a volume. Each panel has its own, as each has its own plane.
pub struct PlanePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    blended: wgpu::Sampler,
    shown: Option<Shown>,
}

impl PlanePipeline {
    /// Build the pipeline for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("plane.wgsl"));
        let layout = bindings(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxa plane"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxa plane"),
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
            label: Some("voxa plane parameters"),
            size: size_of::<Params>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blended = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("voxa plane"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            params,
            blended,
            shown: None,
        }
    }

    /// Draw from the volume `volume` holds now, or draw nothing if it holds none. Call it
    /// before every picture: it does its work only when that volume has changed. A plane
    /// must never be drawn from a volume that has since been replaced, whose texture is gone.
    pub fn show(&mut self, device: &wgpu::Device, volume: &VolumePipeline) {
        let Some((placed, upload)) = volume.placed() else {
            self.shown = None;
            return;
        };
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.upload == upload)
        {
            return;
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxa plane"),
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
        self.shown = Some(Shown {
            bind_group,
            upload,
            rows: texture_rows(placed),
            smallest: placed.smallest,
            span: placed.span,
        });
    }

    /// Whether there is a volume to draw a plane from.
    pub fn has_volume(&self) -> bool {
        self.shown.is_some()
    }

    /// Set what the next picture shows: the plane, in a panel of `viewport` (width, height)
    /// pixels, through `window`. Does nothing while there is no volume.
    pub fn set_params(
        &self,
        queue: &wgpu::Queue,
        view: &PlaneView,
        viewport: [f32; 2],
        window: WindowLevel,
    ) {
        let Some(shown) = &self.shown else {
            return;
        };
        let per_pixel = |side: [f64; 3]| four(side.map(|part| part * view.mm_per_px));
        let corner = view.screen_to_patient(viewport, [0.0, 0.0]);
        let [to_u, to_v, to_w] = shown.rows;
        let [wide, high] = viewport;
        let black = (window.low() - shown.smallest) / shown.span;
        let black_to_white = (window.width / shown.span).max(MIN_WINDOW);
        let params = Params {
            across: per_pixel(view.plane.across),
            down: per_pixel(view.plane.down),
            top_left: four([corner.x, corner.y, corner.z]),
            to_u,
            to_v,
            to_w,
            view: [wide, high, black, black_to_white],
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
    }

    /// Draw the plane over the whole of the pass's viewport. Draws nothing while there is no
    /// volume.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(shown) = &self.shown else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &shown.bind_group, &[]);
        pass.draw(0..3, 0..1);
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
        label: Some("voxa plane"),
        entries: &[entry(0, parameters), entry(1, volume), entry(2, blended)],
    })
}

/// Three numbers as the four the shader keeps them in, at the precision it works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "millimetres and pixels are small"
)]
fn four([x, y, z]: [f64; 3]) -> [f32; 4] {
    [x as f32, y as f32, z as f32, 0.0]
}
