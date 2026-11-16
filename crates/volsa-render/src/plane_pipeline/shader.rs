//! The plane shader, compiled once for the device and shared by every plane drawn, in the
//! plane views and in the slices drawn from the volume alike.

use egui_wgpu::wgpu;

use super::layout::bindings;
use crate::volume_texture::shader_reading_slabs;

/// The compiled plane shader, the layout of what it reads and how it reads it. Cheap to
/// clone: every part is a handle on what the device holds once.
#[derive(Clone)]
pub struct PlaneShader {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) blended: wgpu::Sampler,
    /// Reads beyond a face what lies as far inside it, as the curve's coefficients suppose.
    pub(super) mirrored: wgpu::Sampler,
    /// What the shader is given to read when there are no labels: one voxel of nothing.
    pub(super) no_labels: (wgpu::TextureView, wgpu::TextureView),
}

impl PlaneShader {
    /// Compile the shader for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = shader_reading_slabs(device, "volsa plane", include_str!("../plane.wgsl"));
        let layout = bindings(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("volsa plane"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("volsa plane"),
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
        Self {
            pipeline,
            layout,
            blended: sampler(device, wgpu::AddressMode::ClampToEdge),
            mirrored: sampler(device, wgpu::AddressMode::MirrorRepeat),
            no_labels: super::no_labels(device),
        }
    }
}

/// A sampler that blends the values round a place, reading beyond a face as `beyond` says.
fn sampler(device: &wgpu::Device, beyond: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("volsa plane"),
        address_mode_u: beyond,
        address_mode_v: beyond,
        address_mode_w: beyond,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}
