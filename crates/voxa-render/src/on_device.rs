//! Work the graphics device does in place of the processor, from a volume it already holds:
//! the largest value in each block of voxels, and the curve through the values. Everything
//! here uses only what every graphics device with compute shaders is bound to have, and each
//! maker is `None` on a device that cannot do its work, which is then left to the processor.

use std::ops::Range;

use egui_wgpu::wgpu;

mod blocks;
mod curve;
mod filled;

pub(crate) use blocks::{BlockMaxima, BlocksMaker, largest_in_blocks};
pub(crate) use curve::CurveMaker;
pub(crate) use filled::FilledMaker;

/// The vertex shader every drawing into slices shares.
const SLICES: &str = include_str!("on_device/slices.wgsl");
/// How many slices are drawn into before they are sent to the device, so that it is never
/// handed more than a moment's work in one piece.
const SLICES_AT_ONCE: u32 = 64;

/// Run `build` and give what it made, or `None` if the device objected to any of it: a
/// device that cannot do this work says so when the pipeline is made.
fn if_the_device_can<T>(device: &wgpu::Device, build: impl FnOnce() -> T) -> Option<T> {
    let objections = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let made = build();
    pollster::block_on(objections.pop())
        .is_none()
        .then_some(made)
}

/// A 3D texture of `dims` texels each way, for the uses given.
fn texture(
    device: &wgpu::Device,
    label: &str,
    dims: [u32; 3],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
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
        usage,
        view_formats: &[],
    })
}

/// The kind a 3D texture of real numbers is read as with `textureLoad`.
const REALS: wgpu::TextureSampleType = wgpu::TextureSampleType::Float { filterable: false };

/// A 3D texture of real numbers as a shader reads it with `textureLoad`: one texel at a
/// time, not blended.
fn read_by_texel(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    read_as(binding, visibility, REALS)
}

/// A 3D texture read one texel at a time, as numbers of the kind given.
fn read_as(
    binding: u32,
    visibility: wgpu::ShaderStages,
    sample_type: wgpu::TextureSampleType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        count: None,
    }
}

/// Draws into every slice of a 3D texture: one triangle over the whole of each slice, with
/// a fragment shader that reads one 3D texture and is told which slice it is drawing.
struct IntoSlices {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl IntoSlices {
    /// A drawing with the fragment shader `entry` of `source`, into slices of `format`,
    /// from a texture whose numbers are of the kind `reads`.
    fn new(
        device: &wgpu::Device,
        source: &str,
        entry: &str,
        format: wgpu::TextureFormat,
        reads: wgpu::TextureSampleType,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxa into slices"),
            source: wgpu::ShaderSource::Wgsl(format!("{SLICES}\n{source}").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxa into slices"),
            entries: &[read_as(0, wgpu::ShaderStages::FRAGMENT, reads)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxa into slices"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxa into slices"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("across"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    /// Draw into each of the `slices` of `into`, reading from `from`. The shader is told
    /// each slice counted from the first drawn into, so that `from` may hold only the part
    /// of a volume those slices are worked out from.
    fn draw(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        from: &wgpu::TextureView,
        into: &wgpu::Texture,
        slices: Range<u32>,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxa into slices"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(from),
            }],
        });
        let view = into.create_view(&wgpu::TextureViewDescriptor::default());
        let (mut slice, first) = (slices.start, slices.start);
        while slice < slices.end {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxa into slices"),
            });
            for slice in slice..(slice + SLICES_AT_ONCE).min(slices.end) {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("voxa into a slice"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: Some(slice),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                // The slice is the instance, which is how the fragment shader is told it.
                pass.draw(0..3, slice - first..slice - first + 1);
            }
            queue.submit(Some(encoder.finish()));
            slice += SLICES_AT_ONCE;
        }
    }
}
