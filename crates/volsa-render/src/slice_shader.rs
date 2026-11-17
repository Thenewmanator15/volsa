//! The slice shader, compiled once for the device and shared by every slice drawn: each
//! viewport keeps only its own numbers and its own image.

use egui_wgpu::wgpu;

/// The compiled slice shader with the layouts of what it reads. Cheap to clone: every part
/// is a handle on what the device holds once.
#[derive(Clone)]
pub struct SliceShader {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) labels_layout: wgpu::BindGroupLayout,
    /// One voxel of nothing, bound where there are no labels.
    pub(crate) no_labels: (wgpu::TextureView, wgpu::TextureView),
    /// One value and one whole number of nothing, bound where the image is of the other kind.
    pub(crate) blank: (wgpu::TextureView, wgpu::TextureView),
}

impl SliceShader {
    /// Compile the shader for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("slice.wgsl"));
        let layout = bindings(device);
        let labels_layout = crate::slice_labels::layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("volsa slice"),
            bind_group_layouts: &[Some(&layout), Some(&labels_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("volsa slice"),
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
            labels_layout,
            no_labels: crate::plane_pipeline::no_labels(device),
            blank: blank(device),
        }
    }
}

/// What the shader is given: the parameters, then the image as a texture it reads value by
/// value (floating-point textures cannot be blended by the device itself everywhere, so the
/// shader blends).
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
    let image = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: false },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let numbers = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Uint,
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("volsa slice"),
        entries: &[entry(0, parameters), entry(1, image), entry(2, numbers)],
    })
}

/// One texel of each kind of image, holding nothing.
fn blank(device: &wgpu::Device) -> (wgpu::TextureView, wgpu::TextureView) {
    let one = |format| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volsa no slice"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    };
    (
        one(wgpu::TextureFormat::R32Float),
        one(wgpu::TextureFormat::R16Uint),
    )
}
