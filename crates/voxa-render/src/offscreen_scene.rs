//! Drawing a volume without a window, for tests and exported pictures.

use voxa_core::Volume;

use crate::gpu::{Gpu, TARGET};
use crate::scene::SceneParams;
use crate::slice_pipeline::RenderError;
use crate::volume_pipeline::VolumePipeline;

/// A graphics device with no window, holding one [`VolumePipeline`].
pub struct HeadlessScene {
    gpu: Gpu,
    pipeline: VolumePipeline,
}

impl HeadlessScene {
    /// Start a graphics device: the machine's own adapter if it has one, otherwise one
    /// emulated in software.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoAdapter`] when the machine has neither, and [`RenderError::Device`]
    /// when the adapter found will not start.
    pub fn new() -> Result<Self, RenderError> {
        let gpu = Gpu::start()?;
        let pipeline = VolumePipeline::new(&gpu.device, TARGET);
        Ok(Self { gpu, pipeline })
    }

    /// Put a volume on the graphics device, and say how many times it had to be halved to
    /// fit; see [`VolumePipeline::upload`].
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload`].
    pub fn upload(&mut self, volume: &Volume) -> Result<u32, RenderError> {
        self.pipeline
            .upload(&self.gpu.device, &self.gpu.queue, volume)
    }

    /// Draw the uploaded volume into a picture `size` (width, height) pixels large and return
    /// it as red, green, blue, alpha bytes, row by row from the top. Every pixel is an opaque
    /// grey. With nothing uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold, and
    /// [`RenderError::Readback`] when the picture cannot be copied back.
    pub fn render(&mut self, params: &SceneParams, size: [u32; 2]) -> Result<Vec<u8>, RenderError> {
        let viewport = size.map(|side| side as f32);
        self.pipeline.set_params(&self.gpu.queue, params, viewport);
        self.gpu.picture(size, |pass| self.pipeline.draw(pass))
    }
}

#[cfg(test)]
mod solid_tests;
#[cfg(test)]
mod tests;
