//! Drawing a slice without a window, for tests and thumbnails.

use voxa_core::WindowLevel;

use crate::gpu::{Gpu, TARGET};
use crate::slice_pipeline::{RenderError, SlicePipeline};
use crate::view::ViewTransform;

/// A graphics device with no window, holding one [`SlicePipeline`].
pub struct Headless {
    gpu: Gpu,
    pipeline: SlicePipeline,
}

impl Headless {
    /// Start a graphics device: the machine's own adapter if it has one, otherwise one
    /// emulated in software.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoAdapter`] when the machine has neither, and [`RenderError::Device`]
    /// when the adapter found will not start.
    pub fn new() -> Result<Self, RenderError> {
        let gpu = Gpu::start()?;
        let pipeline = SlicePipeline::new(&gpu.device, TARGET);
        Ok(Self { gpu, pipeline })
    }

    /// Put an image on the graphics device; see [`SlicePipeline::upload`].
    ///
    /// # Errors
    ///
    /// As [`SlicePipeline::upload`], and [`RenderError::DeviceLost`] when the device has
    /// stopped answering.
    pub fn upload(&mut self, rows: u32, cols: u32, values: &[f32]) -> Result<(), RenderError> {
        if self.gpu.is_lost() {
            return Err(RenderError::DeviceLost);
        }
        self.pipeline
            .upload(&self.gpu.device, &self.gpu.queue, rows, cols, values)
    }

    /// Draw the uploaded image into a picture `size` (width, height) pixels large and return
    /// it as red, green, blue, alpha bytes, row by row from the top. Square pixels are
    /// assumed. With nothing uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold,
    /// [`RenderError::Readback`] when the picture cannot be copied back,
    /// [`RenderError::DeviceLost`] when the device has stopped answering.
    pub fn render(
        &mut self,
        wl: WindowLevel,
        view: &ViewTransform,
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        let viewport = size.map(|side| side as f32);
        self.pipeline
            .set_params(&self.gpu.queue, wl, view, viewport, 1.0);
        let drawn = self.gpu.picture(size, |pass| self.pipeline.draw(pass));
        self.gpu.reported(drawn)
    }
}

/// Draw one image into a picture in a single call: start a device, upload, draw, read back.
/// `size` is the picture's width and height.
///
/// # Errors
///
/// Any [`RenderError`]: no adapter, a frame whose values do not match its size, a picture of
/// no size, or a failed read-back.
pub fn render_to_rgba(
    rows: u32,
    cols: u32,
    values: &[f32],
    wl: WindowLevel,
    view: &ViewTransform,
    size: [u32; 2],
) -> Result<Vec<u8>, RenderError> {
    let mut gpu = Headless::new()?;
    gpu.upload(rows, cols, values)?;
    gpu.render(wl, view, size)
}

#[cfg(test)]
mod lost_tests;
#[cfg(test)]
mod tests;
