//! Drawing a volume without a window, for tests and exported pictures.

use voxa_core::Volume;
use voxa_labels::LabelSet;

use voxa_core::WindowLevel;

use crate::gpu::{Gpu, TARGET};
use crate::labels::{LabelsOnDevice, Overlay, PartLook};
use crate::parts_pipeline::PartsPipeline;
use crate::plane_pipeline::PlanePipeline;
use crate::plane_view::PlaneView;
use crate::prepared::Prepared;
use crate::scene::SceneParams;
use crate::slice_pipeline::RenderError;
use crate::smoothed::Smoothed;
use crate::volume_pipeline::VolumePipeline;

/// A graphics device with no window, holding one [`VolumePipeline`].
pub struct HeadlessScene {
    gpu: Gpu,
    pipeline: VolumePipeline,
    plane: PlanePipeline,
    parts: PartsPipeline,
    /// The labels drawn over the planes and as solids, and how.
    labels: Option<LabelsOnDevice>,
    overlay: Overlay,
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
        let plane = PlanePipeline::new(&gpu.device, TARGET);
        let parts = PartsPipeline::new(&gpu.device, TARGET);
        Ok(Self {
            gpu,
            pipeline,
            plane,
            parts,
            labels: None,
            overlay: Overlay::default(),
        })
    }

    /// Put a volume on the graphics device, and say how many times it had to be halved to
    /// fit; see [`VolumePipeline::upload`].
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload`].
    pub fn upload(&mut self, volume: &Volume) -> Result<u32, RenderError> {
        self.plane.smooth(None);
        self.label(None);
        self.pipeline
            .upload(&self.gpu.device, &self.gpu.queue, volume)
    }

    /// Put a volume that was made ready beforehand on the graphics device; see
    /// [`VolumePipeline::upload_prepared`].
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload_prepared`].
    pub fn upload_prepared(&mut self, ready: &Prepared) -> Result<u32, RenderError> {
        self.plane.smooth(None);
        self.label(None);
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.pipeline.upload_prepared(device, queue, ready)
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
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.pipeline.picture(device, queue, params, viewport, size)
    }

    /// Put a label set on the device to be drawn over the planes, every part shown, or take
    /// the labels away with `None`. Whether there are now labels to draw: false with `None`,
    /// and when the device has no room for them. Uploading a volume takes them away.
    pub fn label(&mut self, set: Option<&LabelSet>) -> bool {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.labels = set.and_then(|set| LabelsOnDevice::put(device, queue, set));
        self.plane.labels(self.labels.as_ref(), self.overlay);
        self.parts.show(device, self.labels.as_ref());
        self.labels.is_some()
    }

    /// Whether there are labels to draw as solids.
    pub fn has_parts(&self) -> bool {
        self.parts.has_labels()
    }

    /// Draw the labelled parts that are shown, as lit solids, into a picture `size` (width,
    /// height) pixels large, and return it as red, green, blue, alpha bytes, row by row
    /// from the top. Every pixel is opaque, and black where no part is. With no labels the
    /// picture is black.
    ///
    /// # Errors
    ///
    /// As [`HeadlessScene::render`].
    pub fn render_parts(
        &mut self,
        params: &SceneParams,
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        let viewport = size.map(|side| side as f32);
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.parts.picture(device, queue, params, viewport, size)
    }

    /// Say how each part of the labels is to look, and which is selected.
    pub fn set_looks(&mut self, looks: &[PartLook], selected: Option<usize>) {
        if let Some(labels) = &self.labels {
            labels.set_looks(&self.gpu.queue, looks, selected);
        }
    }

    /// Choose how labels are drawn over the planes.
    pub fn overlay(&mut self, overlay: Overlay) {
        self.overlay = overlay;
        self.plane.labels(self.labels.as_ref(), overlay);
    }

    /// Draw planes from the curve through the uploaded volume, which `with` must be, or with
    /// `None` from the volume's own values again. Whether there is now a curve on the device
    /// to draw from: false with `None`, and when the device has no room for it. Uploading a
    /// volume takes the curve away, as it is another volume's.
    pub fn smooth_planes(&mut self, with: Option<&Smoothed>) -> bool {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let placed = with.and_then(|curve| curve.on(device, queue));
        self.plane.smooth(placed.as_ref());
        placed.is_some()
    }

    /// Draw a plane through the uploaded volume into a picture `size` (width, height) pixels
    /// large, through `window`, and return it as red, green, blue, alpha bytes, row by row
    /// from the top. Every pixel is an opaque grey, black outside the volume. With nothing
    /// uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// As [`HeadlessScene::render`].
    pub fn render_plane(
        &mut self,
        view: &PlaneView,
        window: WindowLevel,
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        let viewport = size.map(|side| side as f32);
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.plane.show(device, &self.pipeline);
        self.plane.set_params(queue, view, viewport, window);
        Gpu::on(device, queue).picture(size, |pass| self.plane.draw(pass))
    }
}

#[cfg(test)]
mod cut_tests;
#[cfg(test)]
mod haze_tests;
#[cfg(test)]
mod label_tests;
#[cfg(test)]
mod parts_tests;
#[cfg(test)]
mod plane_tests;
#[cfg(test)]
mod skip_tests;
#[cfg(test)]
mod smooth_tests;
#[cfg(test)]
mod solid_tests;
#[cfg(test)]
mod tests;
