//! The GPU pipeline that draws one slice with window/level applied in the shader.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use volsa_core::{Affine, Patient, Voxel, WindowLevel};

use crate::labels::{LabelsOnDevice, Overlay};
use crate::slice_labels::SliceLabels;
use crate::slice_shader::SliceShader;
use crate::view::ViewTransform;

mod upload;
pub use upload::SliceNumbers;

/// Why a slice could not be drawn.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The machine has no graphics adapter, not even one emulated in software. A device with no
    /// window leaves OpenGL out on Windows unless `WGPU_BACKEND` is set, so a Windows machine
    /// that could draw only with OpenGL has none until `WGPU_BACKEND=gl` says to try it.
    #[error("No graphics adapter is available, not even a software one.")]
    NoAdapter,
    /// An adapter was found but would not start.
    #[error("The graphics device could not be started: {0}.")]
    Device(String),
    /// The number of values is not rows times columns, or the image has no size.
    #[error("The image has {values} values but its size is {cols} × {rows}.")]
    FrameSize {
        /// Rows the image claims.
        rows: u32,
        /// Columns the image claims.
        cols: u32,
        /// Values actually given.
        values: usize,
    },
    /// The image is larger than the graphics device can hold as one texture.
    #[error("The image ({cols} × {rows}) is larger than this graphics device can hold ({limit}).")]
    TooLarge {
        /// Rows in the image.
        rows: u32,
        /// Columns in the image.
        cols: u32,
        /// The longest side the device accepts.
        limit: u32,
    },
    /// The graphics device has no room for a volume, however much it is reduced.
    #[error("The graphics device has no room for this volume, even much reduced.")]
    NoRoom,
    /// A picture with no width or no height was asked for.
    #[error("A picture with no size cannot be drawn.")]
    EmptyTarget,
    /// The drawn picture could not be copied back from the graphics device.
    #[error("The picture could not be read back from the graphics device: {0}.")]
    Readback(String),
    /// The graphics device stopped answering, as after a driver reset. Nothing more can be
    /// drawn on it until the program is started again.
    #[error("The graphics card stopped answering.")]
    DeviceLost,
}

/// What the shader needs besides the image; the layout matches `Params` in `slice.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    to_col: [f32; 4],
    to_row: [f32; 4],
    sizes: [f32; 4],
    window: [f32; 4],
    label_u: [f32; 4],
    label_v: [f32; 4],
    label_w: [f32; 4],
    label_grid: [f32; 4],
    label_how: [f32; 4],
    stored: [f32; 4],
}

/// The image now on the graphics device.
struct Uploaded {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    rows: u32,
    cols: u32,
    /// How the shader makes values of the numbers, as `stored` in `slice.wgsl`.
    stored: [f32; 4],
}

/// Draws one slice. The image is uploaded once as a single-channel floating-point texture;
/// window, level and view are a small block of numbers the shader reads, so changing them
/// never uploads the image again.
pub struct SlicePipeline {
    shader: SliceShader,
    params: wgpu::Buffer,
    /// The numbers last written to the device, so the same are not written again.
    written: Option<Params>,
    uploaded: Option<Uploaded>,
    /// The labels drawn over the slice, read where the device already holds them.
    labels: SliceLabels,
    /// Which slice of its series the image is, for finding it among the labels.
    slice: f64,
}

impl SlicePipeline {
    /// Build the pipeline for drawing into a target of the given format, with a shader of its
    /// own. Where several slices are drawn, [`Self::of`] shares one.
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        Self::of(device, &SliceShader::new(device, target))
    }

    /// A pipeline drawing with `shader`, compiled once for every slice drawn.
    pub fn of(device: &wgpu::Device, shader: &SliceShader) -> Self {
        let labels = SliceLabels::new(device, shader);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volsa slice parameters"),
            size: size_of::<Params>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            shader: shader.clone(),
            params,
            written: None,
            uploaded: None,
            labels,
            slice: 0.0,
        }
    }

    /// Draw `with` over the slice in the way `overlay` says, or no labels with `None`. The
    /// slice is number `slice` of a series laid out in the patient by `place`, which takes
    /// column, row and slice; labels on a grid of their own are drawn where they lie. Takes
    /// effect at the next [`Self::set_params`].
    pub fn labels(
        &mut self,
        device: &wgpu::Device,
        with: Option<&LabelsOnDevice>,
        overlay: Overlay,
        place: Option<(&Affine<Voxel, Patient>, usize)>,
    ) {
        self.labels
            .set(device, with, overlay, place.map(|(place, _)| place));
        self.slice = place.map_or(0.0, |(_, slice)| {
            f64::from(u32::try_from(slice).unwrap_or(u32::MAX))
        });
    }

    /// Set the window, level and view for the next draw. `viewport` is the width and height,
    /// in screen pixels, of the area drawn into; `pixel_aspect` is how many times taller than
    /// wide one pixel of the image is. Does nothing until an image has been uploaded.
    pub fn set_params(
        &mut self,
        queue: &wgpu::Queue,
        wl: WindowLevel,
        view: &ViewTransform,
        viewport: [f32; 2],
        pixel_aspect: f32,
    ) {
        let Some(uploaded) = &self.uploaded else {
            return;
        };
        let image = [uploaded.cols, uploaded.rows];
        let to_screen = view.image_to_screen(image, viewport, pixel_aspect);
        let Some(to_image) = to_screen.inverse() else {
            return;
        };
        let [[col_x, row_x, _], [col_y, row_y, _], _] = to_image.columns();
        let [col_0, row_0, _] = to_image.origin();
        let [wide, high] = viewport;
        let [label_u, label_v, label_w, label_grid, label_how] = self.labels.params(self.slice);
        let params = Params {
            to_col: [single(col_x), single(col_y), single(col_0), 0.0],
            to_row: [single(row_x), single(row_y), single(row_0), 0.0],
            sizes: [wide, high, uploaded.cols as f32, uploaded.rows as f32],
            window: [wl.low(), wl.width.max(f32::MIN_POSITIVE), 0.0, 0.0],
            label_u,
            label_v,
            label_w,
            label_grid,
            label_how,
            stored: uploaded.stored,
        };
        let same = |written: &Params| bytemuck::bytes_of(written) == bytemuck::bytes_of(&params);
        if !self.written.as_ref().is_some_and(same) {
            queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
            self.written = Some(params);
        }
    }

    /// Draw the uploaded image over the whole of the pass's viewport. Draws nothing until an
    /// image has been uploaded.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(uploaded) = &self.uploaded else {
            return;
        };
        pass.set_pipeline(&self.shader.pipeline);
        pass.set_bind_group(0, &uploaded.bind_group, &[]);
        pass.set_bind_group(1, self.labels.bound(), &[]);
        pass.draw(0..3, 0..1);
    }
}

/// A coordinate as the single-precision number the shader works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "screen and image coordinates are small"
)]
fn single(value: f64) -> f32 {
    value as f32
}
