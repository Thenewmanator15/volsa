//! The curve through a volume's values, worked out by the graphics device from the volume it
//! holds: no pass over the values on the processor, and nothing handed across.

use egui_wgpu::wgpu;

use super::{IntoSlices, REALS, if_the_device_can, read_by_texel, texture};
use crate::smoothed::SmoothedTexture;

/// The coefficients as planes are drawn from them: held as the volume's values are.
const HALVES: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// Where they are worked out: every graphics device can read and write this in one shader.
const WORK: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
/// The most voxels one dispatch filters. A large volume is done in several, so that the
/// device is never busy with one piece of work for long.
const VOXELS_AT_ONCE: u64 = 1 << 24;
/// The side of a work group, as the shader has it.
const GROUP: u32 = 8;

/// Works out, on the device, the curve through the values of a volume the device holds.
pub(crate) struct CurveMaker {
    layout: wgpu::BindGroupLayout,
    seed: wgpu::ComputePipeline,
    along: wgpu::ComputePipeline,
    copy: IntoSlices,
}

impl CurveMaker {
    /// `None` on a device with no compute shaders, or one that cannot read and write a
    /// texture in the same shader.
    pub(crate) fn new(device: &wgpu::Device) -> Option<Self> {
        if_the_device_can(device, || {
            let shader = device.create_shader_module(wgpu::include_wgsl!("curve.wgsl"));
            let layout = bindings(device);
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("voxa curve"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = |entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("voxa curve"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
            };
            Self {
                seed: pipeline("seed"),
                along: pipeline("along"),
                copy: IntoSlices::new(
                    device,
                    include_str!("curve_copy.wgsl"),
                    "stored",
                    HALVES,
                    REALS,
                ),
                layout,
            }
        })
    }

    /// The curve through the volume of `dims` voxels whose values `values` reads. `None`
    /// when the device has no room to work it out. The device does the work after this
    /// returns, before anything drawn afterwards.
    pub(crate) fn make(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        values: &wgpu::TextureView,
        dims: [u32; 3],
    ) -> Option<SmoothedTexture> {
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let worked_in = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
        let work = texture(
            device,
            "voxa curve, being worked out",
            dims,
            WORK,
            worked_in,
        );
        let work_view = work.create_view(&wgpu::TextureViewDescriptor::default());
        let drawn_and_read = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        let coefficients = texture(device, "voxa volume curve", dims, HALVES, drawn_and_read);
        let [cols, rows, slices] = dims;
        let textures = (values, &work_view);
        // The values copied in a slab of slices at a time, then filtered each way in turn.
        let in_a_slice = u64::from(cols) * u64::from(rows);
        let slab = at_once(in_a_slice, 1);
        for first in (0..slices).step_by(slab as usize) {
            let counts = [groups(cols), groups(rows), slab.min(slices - first)];
            self.dispatch((device, queue), textures, &self.seed, [0, first], counts);
        }
        let ways = [
            (0, rows, slices, cols),
            (1, cols, slices, rows),
            (2, cols, rows, slices),
        ];
        for (axis, across, down, along) in ways {
            // Whole work groups of lines, so that no line is filtered twice.
            let lines = at_once(u64::from(across) * u64::from(along), GROUP);
            for first in (0..down).step_by(lines as usize) {
                let counts = [groups(across), groups(lines.min(down - first)), 1];
                self.dispatch(
                    (device, queue),
                    textures,
                    &self.along,
                    [axis, first],
                    counts,
                );
            }
        }
        self.copy
            .draw((device, queue), &work_view, &coefficients, 0..slices);
        // Given back once the device has drawn from it, not before.
        work.destroy();
        if pollster::block_on(room.pop()).is_some() {
            coefficients.destroy();
            return None;
        }
        Some(SmoothedTexture::holding(coefficients, dims))
    }

    /// Send one piece of the work to the device: `pipeline` over `groups` work groups, told
    /// which way its lines run and where it starts.
    fn dispatch(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        (values, work): (&wgpu::TextureView, &wgpu::TextureView),
        pipeline: &wgpu::ComputePipeline,
        [axis, first]: [u32; 2],
        [across, down, deep]: [u32; 3],
    ) {
        let told = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxa curve lines"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&told, 0, bytemuck::cast_slice(&[axis, first, 0, 0]));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxa curve"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: told.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(work),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(values),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("voxa curve"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("voxa curve"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(across, down, deep);
        }
        queue.submit(Some(encoder.finish()));
    }
}

/// How many work groups cover `count` voxels along one way.
fn groups(count: u32) -> u32 {
    count.div_ceil(GROUP)
}

/// How many layers of `in_a_layer` voxels one dispatch takes: as many as keep it within
/// [`VOXELS_AT_ONCE`], in whole multiples of `whole`, and at least that many.
fn at_once(in_a_layer: u64, whole: u32) -> u32 {
    let layers = VOXELS_AT_ONCE / in_a_layer.max(1);
    let whole = whole.max(1);
    let fit = u32::try_from(layers).unwrap_or(u32::MAX) / whole * whole;
    fit.max(whole)
}

fn bindings(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let told = wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let work = wgpu::BindGroupLayoutEntry {
        binding: 1,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::ReadWrite,
            format: WORK,
            view_dimension: wgpu::TextureViewDimension::D3,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("voxa curve"),
        entries: &[told, work, read_by_texel(2, wgpu::ShaderStages::COMPUTE)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_volume_is_filtered_in_pieces_of_whole_work_groups() {
        // 512 by 512 voxels a slice: 64 slices are as many as one dispatch takes.
        assert_eq!(at_once(512 * 512, 1), 64);
        // Lines 512 long, 512 of them across: 64 rows of lines, which is 8 work groups.
        assert_eq!(at_once(512 * 512, GROUP), 64);
        assert_eq!(
            at_once(300 * 512, GROUP),
            104,
            "a whole number of groups of 8"
        );
        assert_eq!(at_once(4096 * 4096, GROUP), 8, "never less than one group");
        assert_eq!(at_once(0, 1), u32::try_from(VOXELS_AT_ONCE).expect("fits"));
        assert_eq!((groups(1), groups(8), groups(9)), (1, 1, 2));
    }
}
