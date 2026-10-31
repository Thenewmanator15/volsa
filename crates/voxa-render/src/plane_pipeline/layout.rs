//! What the plane shader is given, as the device is told it.

use egui_wgpu::wgpu;

use crate::volume_texture::more_slabs_layout;

/// What the shader is given: the parameters, the volume as a texture and the way values
/// between its voxels are blended, the curve through it and the way that is read, then the
/// part at every label voxel and the table of how each part looks.
pub(super) fn bindings(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
    let parts = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Uint,
        view_dimension: wgpu::TextureViewDimension::D3,
        multisampled: false,
    };
    let looks = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let [one, two, three] = more_slabs_layout();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("voxa plane"),
        entries: &[
            entry(0, parameters),
            entry(1, volume),
            entry(2, blended),
            entry(3, volume),
            entry(4, blended),
            entry(5, parts),
            entry(6, looks),
            one,
            two,
            three,
        ],
    })
}
