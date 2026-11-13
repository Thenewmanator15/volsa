//! The GPU pipeline that draws labelled parts as lit solids. It reads the labels the planes
//! read, on the labels' own grid, so showing them in 3D costs no second copy.

use std::sync::OnceLock;

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;

use crate::gpu::Gpu;
use crate::labels::LabelsOnDevice;
use crate::on_device::FilledMaker;
use crate::scene::{MAX_CUTS, SceneParams};
use crate::slice_pipeline::RenderError;
use crate::volume_params::{MIN_QUALITY, cuts_in_use, four, single};

/// What the shader needs besides the labels; the layout matches `Scene` in `parts.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    across: [f32; 4],
    along: [f32; 4],
    forward: [f32; 4],
    top_left: [f32; 4],
    to_col: [f32; 4],
    to_row: [f32; 4],
    to_slice: [f32; 4],
    view: [f32; 4],
    grid: [f32; 4],
    cuts: [[f32; 4]; MAX_CUTS],
}

/// The labels the solids are drawn from now.
struct Shown {
    bind_group: wgpu::BindGroup,
    labels: LabelsOnDevice,
    /// Which blocks of the label volume hold a part, where the device worked that out.
    filled: Option<wgpu::Texture>,
}

impl Drop for Shown {
    /// Give the device its memory back now, not whenever it gets round to it.
    fn drop(&mut self) {
        if let Some(filled) = &self.filled {
            filled.destroy();
        }
    }
}

/// Draws the parts of a label set that are shown, each as a lit solid in its colour.
pub struct PartsPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    shown: Option<Shown>,
    /// What finds, on the device, the blocks of a label volume that hold no part; made
    /// when first wanted, and `None` on a device that cannot.
    filler: OnceLock<Option<FilledMaker>>,
    /// What the shader is given to read when no blocks have been worked out.
    no_blocks: wgpu::TextureView,
}

impl PartsPipeline {
    /// Build the pipeline for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("parts.wgsl"));
        let layout = bindings(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("volsa parts"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("volsa parts"),
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
            label: Some("volsa parts parameters"),
            size: size_of::<Params>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let one = wgpu::TextureFormat::R16Float;
        let no_blocks =
            crate::volume_texture::texture(device, "volsa no label blocks", [1; 3], one);
        Self {
            pipeline,
            layout,
            params,
            shown: None,
            filler: OnceLock::new(),
            no_blocks: no_blocks.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }

    /// Draw from these labels, or draw nothing with `None`. It does its work only when they
    /// are not the ones already drawn from: then the device is also set to find the blocks
    /// of the label volume that hold no part, which lines of sight step over.
    pub fn show(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        labels: Option<&LabelsOnDevice>,
    ) {
        let Some(labels) = labels else {
            self.shown = None;
            return;
        };
        let same = |shown: &Shown| shown.labels.number() == labels.number();
        if self.shown.as_ref().is_some_and(same) {
            return;
        }
        let maker = self
            .filler
            .get_or_init(|| FilledMaker::new(device))
            .as_ref();
        let filled =
            maker.map(|maker| maker.make((device, queue), labels.parts_view(), labels.dims()));
        let whole = wgpu::TextureViewDescriptor::default();
        let blocks = filled.as_ref().map(|filled| filled.create_view(&whole));
        let view = wgpu::BindingResource::TextureView;
        let resources = [
            self.params.as_entire_binding(),
            view(labels.parts_view()),
            view(labels.looks_view()),
            view(blocks.as_ref().unwrap_or(&self.no_blocks)),
        ];
        let entries: Vec<wgpu::BindGroupEntry> = (0..)
            .zip(resources)
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volsa parts"),
            layout: &self.layout,
            entries: &entries,
        });
        self.shown = Some(Shown {
            bind_group,
            labels: labels.clone(),
            filled,
        });
    }

    /// The texture that says which blocks of the label volume hold a part, for reading
    /// it back.
    #[cfg(test)]
    pub(crate) fn filled(&self) -> Option<&wgpu::Texture> {
        self.shown.as_ref()?.filled.as_ref()
    }

    /// Whether there are labels to draw solids from.
    pub fn has_labels(&self) -> bool {
        self.shown.is_some()
    }

    /// Set what the next picture is drawn from. `viewport` is the width and height, in
    /// pixels, of the picture. The scene's window and look play no part: a part is drawn in
    /// its own colour. Does nothing while there are no labels.
    pub fn set_params(&self, queue: &wgpu::Queue, scene: &SceneParams, viewport: [f32; 2]) {
        let Some(shown) = &self.shown else {
            return;
        };
        let labels = &shown.labels;
        let to_patient = scene.camera.screen_to_patient(viewport);
        let [across, along, forward] = to_patient.columns().map(four);
        let to_voxel = labels.patient_to_voxel();
        let row = |pick: fn([f64; 3]) -> f64| {
            let [x, y, z] = to_voxel.columns().map(pick);
            [x, y, z, pick(to_voxel.origin())].map(single)
        };
        let quality = if scene.quality.is_finite() {
            scene.quality
        } else {
            1.0
        };
        // Half a label voxel at full quality: no part a voxel thick is stepped over.
        let step_mm = single(labels.finest_mm() / 2.0) / quality.clamp(MIN_QUALITY, 1.0);
        let (cuts, cuts_used) = cuts_in_use(scene);
        let [wide, high] = viewport;
        let [cols, rows, slices] = labels.dims();
        let params = Params {
            across,
            along,
            // The fourth number is the part drawn through the rest, 0 for none.
            forward: {
                let [x, y, z, _] = forward;
                [x, y, z, labels.through() as f32]
            },
            top_left: {
                // The fourth number says whether to step over the blocks that hold no part.
                let [x, y, z, _] = four(to_patient.origin());
                let stepped_over = scene.skip_empty && shown.filled.is_some();
                [x, y, z, f32::from(u8::from(stepped_over))]
            },
            to_col: row(|[col, _, _]| col),
            to_row: row(|[_, line, _]| line),
            to_slice: row(|[_, _, slice]| slice),
            view: [wide, high, step_mm, labels.selected() as f32],
            grid: [cols as f32, rows as f32, slices as f32, cuts_used],
            cuts,
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
    }

    /// Draw the solids over the whole of the pass's viewport. Draws nothing while there are
    /// no labels.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(shown) = &self.shown else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &shown.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Draw the solids into a picture `size` (width, height) pixels large, off the screen,
    /// and return it as red, green, blue, alpha bytes, row by row from the top. Every pixel
    /// is opaque, and black where no part is. `viewport` is the size the picture is laid out
    /// at, which is what the camera measures in. With no labels the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold, and
    /// [`RenderError::Readback`] when the picture cannot be copied back.
    pub fn picture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneParams,
        viewport: [f32; 2],
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        self.set_params(queue, scene, viewport);
        Gpu::on(device, queue).picture(size, |pass| self.draw(pass))
    }
}

/// What the shader is given: the parameters, the part at every label voxel, and the table of
/// how each part looks.
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
    let blocks = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: false },
        view_dimension: wgpu::TextureViewDimension::D3,
        multisampled: false,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("volsa parts"),
        entries: &[
            entry(0, parameters),
            entry(1, parts),
            entry(2, looks),
            entry(3, blocks),
        ],
    })
}
