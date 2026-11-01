//! The GPU pipeline that draws a plane through the volume the scene holds. It reads the
//! scene's own texture, so a plane view costs no second copy of the volume and moving through
//! its layers decodes nothing.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use volsa_core::WindowLevel;

use crate::labels::{FILL, LabelsOnDevice, Overlay};
use crate::plane_view::PlaneView;
use crate::smoothed::SmoothedTexture;
use crate::volume_params::{MIN_WINDOW, texture_rows};
use crate::volume_pipeline::VolumePipeline;
use crate::volume_texture::{Placed, shader_reading_slabs};

mod layout;
use layout::bindings;

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
    grid: [f32; 4],
    label_u: [f32; 4],
    label_v: [f32; 4],
    label_w: [f32; 4],
    label_grid: [f32; 4],
    label_how: [f32; 4],
    slabs: [f32; 4],
    depths: [f32; 4],
}

/// The volume a plane is drawn from now.
struct Shown {
    bind_group: wgpu::BindGroup,
    /// Which upload of the volume pipeline's this is, so that a newer one is noticed.
    upload: u64,
    /// Which curve the planes are drawn from, if they are drawn smooth.
    curve: Option<u64>,
    /// Which labels are bound, if any are.
    labels: Option<u64>,
    /// Columns, rows and slices of the volume as the device holds it.
    dims: [u32; 3],
    rows: [[f32; 4]; 3],
    /// The scan value stored as 0, and the span from it to the one stored as 1.
    smallest: f32,
    span: f32,
    /// How the volume is cut into textures, as the shader is told it.
    slabs: ([f32; 4], [f32; 4]),
}

/// Draws one plane through a volume. Each panel has its own, as each has its own plane.
pub struct PlanePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    blended: wgpu::Sampler,
    /// Reads beyond a face what lies as far inside it, as the curve's coefficients suppose.
    mirrored: wgpu::Sampler,
    /// The curve through the volume, when planes are to be drawn smooth.
    curve: Option<SmoothedTexture>,
    /// The labels drawn over the planes, and how, when there are any.
    labels: Option<LabelsOnDevice>,
    overlay: Overlay,
    /// What the shader is given to read when there are no labels: one voxel of nothing.
    no_labels: (wgpu::TextureView, wgpu::TextureView),
    shown: Option<Shown>,
}

impl PlanePipeline {
    /// Build the pipeline for drawing into a target of the given format.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let shader = shader_reading_slabs(device, "volsa plane", include_str!("plane.wgsl"));
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
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volsa plane parameters"),
            size: size_of::<Params>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            params,
            blended: sampler(device, wgpu::AddressMode::ClampToEdge),
            mirrored: sampler(device, wgpu::AddressMode::MirrorRepeat),
            curve: None,
            labels: None,
            overlay: Overlay::Off,
            no_labels: no_labels(device),
            shown: None,
        }
    }

    /// Draw these labels over the planes in this way, or none with `None`. They lie where
    /// the patient has them, on their own grid, whatever volume is shown.
    pub fn labels(&mut self, with: Option<&LabelsOnDevice>, overlay: Overlay) {
        self.labels = with.cloned();
        self.overlay = overlay;
    }

    /// Draw planes from the curve through the volume, or, with `None`, from the volume's own
    /// values blended. The curve is used only while it is the size of the volume shown: it
    /// must be the curve through that very volume, and be taken away when the volume changes.
    pub fn smooth(&mut self, with: Option<&SmoothedTexture>) {
        self.curve = with.cloned();
    }

    /// Whether planes are drawn from the curve now. True only after [`PlanePipeline::show`]
    /// has taken up a curve given to [`PlanePipeline::smooth`].
    pub fn is_smooth(&self) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|shown| shown.curve.is_some())
    }

    /// Draw from the volume `volume` holds now, or draw nothing if it holds none. Call it
    /// before every picture: it does its work only when that volume, or the curve to draw
    /// it from, has changed. A plane must never be drawn from a volume that has since been
    /// replaced, whose texture is gone.
    pub fn show(&mut self, device: &wgpu::Device, volume: &VolumePipeline) {
        let Some((placed, upload)) = volume.placed() else {
            self.shown = None;
            return;
        };
        let curve = self.curve.as_ref();
        let curve = curve.filter(|curve| curve.dims() == placed.dims);
        let wanted = curve.map(SmoothedTexture::number);
        let labels = self.labels.as_ref().map(LabelsOnDevice::number);
        let as_it_is = |shown: &Shown| {
            shown.upload == upload && shown.curve == wanted && shown.labels == labels
        };
        if self.shown.as_ref().is_some_and(as_it_is) {
            return;
        }
        // With no curve the shader is still given something to read there, and does not.
        let coefficients = curve.map_or(placed.view(), SmoothedTexture::view);
        let bind_group = self.bound(device, placed, coefficients);
        self.shown = Some(Shown {
            bind_group,
            upload,
            curve: wanted,
            labels,
            dims: placed.dims,
            rows: texture_rows(placed),
            smallest: placed.smallest,
            span: placed.span,
            slabs: placed.plan.uniforms(),
        });
    }

    /// Everything the shader reads, bound together: the parameters, the volume and the
    /// curve through it, and the labels, or one voxel of nothing where there are none.
    fn bound(
        &self,
        device: &wgpu::Device,
        volume: &Placed,
        coefficients: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let (no_parts, no_looks) = &self.no_labels;
        let labels = self.labels.as_ref();
        let parts = labels.map_or(no_parts, LabelsOnDevice::parts_view);
        let looks = labels.map_or(no_looks, LabelsOnDevice::looks_view);
        let view = wgpu::BindingResource::TextureView;
        let resources = [
            self.params.as_entire_binding(),
            view(volume.view()),
            wgpu::BindingResource::Sampler(&self.blended),
            view(coefficients),
            wgpu::BindingResource::Sampler(&self.mirrored),
            view(parts),
            view(looks),
        ];
        let entries: Vec<wgpu::BindGroupEntry> = (0..)
            .zip(resources)
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .chain(volume.more_slabs())
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volsa plane"),
            layout: &self.layout,
            entries: &entries,
        })
    }

    /// What the shader needs to find a point of the patient in the labels and to draw them:
    /// three rows from the patient to label voxels, the size of the label volume with how
    /// they are drawn, and which part is selected with how strongly the inside is tinted.
    fn label_params(&self, shown: &Shown) -> [[f32; 4]; 5] {
        let labels = self.labels.as_ref().filter(|_| shown.labels.is_some());
        let Some(labels) = labels.filter(|_| self.overlay != Overlay::Off) else {
            return [[0.0; 4]; 5];
        };
        let to_voxel = labels.patient_to_voxel();
        let row = |pick: fn([f64; 3]) -> f64| {
            let [x, y, z] = to_voxel.columns().map(pick);
            let [x, y, z, _] = four([x, y, z]);
            let [first, ..] = four([pick(to_voxel.origin()), 0.0, 0.0]);
            [x, y, z, first]
        };
        let [cols, rows, slices] = labels.dims();
        let mode = if self.overlay == Overlay::Filled {
            2.0
        } else {
            1.0
        };
        [
            row(|[col, _, _]| col),
            row(|[_, line, _]| line),
            row(|[_, _, slice]| slice),
            [cols as f32, rows as f32, slices as f32, mode],
            [labels.selected() as f32, FILL, 0.0, 0.0],
        ]
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
        let [cols, rows, slices] = shown.dims;
        let smooth = if shown.curve.is_some() { 1.0 } else { 0.0 };
        let [label_u, label_v, label_w, label_grid, label_how] = self.label_params(shown);
        let (slabs, depths) = shown.slabs;
        let params = Params {
            across: per_pixel(view.plane.across),
            down: per_pixel(view.plane.down),
            top_left: four([corner.x, corner.y, corner.z]),
            to_u,
            to_v,
            to_w,
            view: [wide, high, black, black_to_white],
            grid: [cols as f32, rows as f32, slices as f32, smooth],
            label_u,
            label_v,
            label_w,
            label_grid,
            label_how,
            slabs,
            depths,
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

/// What the shader reads in place of labels when there are none: a label volume of one
/// voxel that holds nothing, and a table of looks with one clear entry.
fn no_labels(device: &wgpu::Device) -> (wgpu::TextureView, wgpu::TextureView) {
    let one = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let made = |dimension, format| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volsa no labels"),
            size: one,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    };
    (
        made(wgpu::TextureDimension::D3, wgpu::TextureFormat::R16Uint),
        made(wgpu::TextureDimension::D2, wgpu::TextureFormat::Rgba8Unorm),
    )
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

/// Three numbers as the four the shader keeps them in, at the precision it works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "millimetres and pixels are small"
)]
fn four([x, y, z]: [f64; 3]) -> [f32; 4] {
    [x as f32, y as f32, z as f32, 0.0]
}
