//! Drawing a volume without a window, for tests and exported pictures.

use std::sync::Arc;

use egui_wgpu::wgpu;
use voxa_core::{HeldVolume, Volume};
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
use crate::smoothed::{Smoothed, SmoothedTexture};
use crate::upload::{CardRoom, UploadState};
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
    /// The curve planes are drawn from, when they are drawn smooth.
    curve: Option<SmoothedTexture>,
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
        Ok(Self::on(Gpu::start()?))
    }

    /// Start the graphics device emulated in software; see [`Gpu::start_in_software`].
    ///
    /// # Errors
    ///
    /// As [`Gpu::start_in_software`].
    #[cfg(test)]
    pub(crate) fn in_software() -> Result<Self, RenderError> {
        Ok(Self::on(Gpu::start_in_software()?))
    }

    /// The pipelines made on a device that is running, with nothing on it yet.
    fn on(gpu: Gpu) -> Self {
        let pipeline = VolumePipeline::new(&gpu.device, TARGET);
        let plane = PlanePipeline::new(&gpu.device, TARGET);
        let parts = PartsPipeline::new(&gpu.device, TARGET);
        Self {
            gpu,
            pipeline,
            plane,
            parts,
            labels: None,
            overlay: Overlay::default(),
            curve: None,
        }
    }

    /// Put a volume on the graphics device, and say how many times it had to be halved to
    /// fit; see [`VolumePipeline::upload`].
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload`], and [`RenderError::DeviceLost`] when the device has
    /// stopped answering.
    pub fn upload(&mut self, volume: &Volume) -> Result<u32, RenderError> {
        self.curve = None;
        self.plane.smooth(None);
        self.label(None);
        if self.gpu.is_lost() {
            return Err(RenderError::DeviceLost);
        }
        self.pipeline
            .upload(&self.gpu.device, &self.gpu.queue, volume)
    }

    /// Put a volume that was made ready beforehand on the graphics device; see
    /// [`VolumePipeline::upload_prepared`].
    ///
    /// # Errors
    ///
    /// As [`VolumePipeline::upload_prepared`], and [`RenderError::DeviceLost`] when the
    /// device has stopped answering.
    pub fn upload_prepared(&mut self, ready: &Prepared) -> Result<u32, RenderError> {
        self.curve = None;
        self.plane.smooth(None);
        self.label(None);
        if self.gpu.is_lost() {
            return Err(RenderError::DeviceLost);
        }
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.pipeline.upload_prepared(device, queue, ready)
    }

    /// Hand a held volume to the device whole, as the window does a little each frame, as
    /// stored where the device can hold it so. Gives how many times it was halved: none,
    /// since a volume too large for the device is halved before it is held for it.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoRoom`] when the device has no room for it, and
    /// [`RenderError::DeviceLost`] when the device has stopped answering.
    pub fn upload_held(&mut self, held: &Arc<HeldVolume>) -> Result<u32, RenderError> {
        self.curve = None;
        self.plane.smooth(None);
        self.label(None);
        if self.gpu.is_lost() {
            return Err(RenderError::DeviceLost);
        }
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let room = CardRoom::of(device, None);
        self.pipeline.start_upload(device, Arc::clone(held), room)?;
        match self.pipeline.continue_upload(device, queue, u64::MAX) {
            UploadState::Done => Ok(0),
            UploadState::Failed(why) => Err(why),
            UploadState::Idle | UploadState::Going { .. } => Err(RenderError::NoRoom),
        }
    }

    /// Draw the uploaded volume into a picture `size` (width, height) pixels large and return
    /// it as red, green, blue, alpha bytes, row by row from the top. Every pixel is an opaque
    /// grey. With nothing uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold,
    /// [`RenderError::Readback`] when the picture cannot be copied back,
    /// [`RenderError::DeviceLost`] when the device has stopped answering.
    pub fn render(&mut self, params: &SceneParams, size: [u32; 2]) -> Result<Vec<u8>, RenderError> {
        let viewport = size.map(|side| side as f32);
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let drawn = self.pipeline.picture(device, queue, params, viewport, size);
        self.gpu.reported(drawn)
    }

    /// Put a label set on the device to be drawn over the planes, every part shown, or take
    /// the labels away with `None`. Whether there are now labels to draw: false with `None`,
    /// and when the device has no room for them, which it has not once it has stopped
    /// answering. Uploading a volume takes them away.
    pub fn label(&mut self, set: Option<&LabelSet>) -> bool {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let answering = !self.gpu.is_lost();
        self.labels = set
            .filter(|_| answering)
            .and_then(|set| LabelsOnDevice::put(device, queue, set));
        self.plane.labels(self.labels.as_ref(), self.overlay);
        self.parts.show(device, queue, self.labels.as_ref());
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
        let drawn = self.parts.picture(device, queue, params, viewport, size);
        self.gpu.reported(drawn)
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
        self.curve = with.and_then(|curve| curve.on(device, queue));
        self.plane.smooth(self.curve.as_ref());
        self.curve.is_some()
    }

    /// As [`HeadlessScene::smooth_planes`], with the curve worked out by the device from
    /// the volume it holds. False with no volume uploaded, on a device that cannot, and
    /// when it has no room.
    pub fn smooth_planes_on_device(&mut self) -> bool {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        self.curve = self.pipeline.curve_on_device(device, queue);
        self.plane.smooth(self.curve.as_ref());
        self.curve.is_some()
    }

    /// The graphics device this scene draws with, and its queue, for handing it work from
    /// elsewhere: a volume made ready on another thread, say.
    pub fn device(&self) -> (wgpu::Device, wgpu::Queue) {
        (self.gpu.device.clone(), self.gpu.queue.clone())
    }

    /// Which graphics device this is, and how it is driven, in a few words: for saying
    /// what a measurement was made on.
    pub fn device_name(&self) -> String {
        let info = self.gpu.device.adapter_info();
        format!("{} ({:?})", info.name, info.backend)
    }

    /// Wait until the device has done everything handed to it so far. Work handed to a
    /// graphics device is done some time afterwards; this is for timing it.
    pub fn settle(&self) {
        let _ = crate::lost::poll(&self.gpu.device, wgpu::PollType::wait_indefinitely());
    }

    /// Whether this device works out the largest value in each block of a volume itself.
    pub fn makes_blocks(&self) -> bool {
        self.pipeline.makes_blocks(&self.gpu.device)
    }

    /// Whether this device works out the curve through a volume itself.
    pub fn makes_curves(&self) -> bool {
        self.pipeline.makes_curves(&self.gpu.device)
    }

    /// The largest value in each block of the uploaded volume, as the device holds it.
    #[cfg(test)]
    pub(crate) fn blocks_as_held(&self) -> Option<Vec<u16>> {
        let (placed, _) = self.pipeline.placed()?;
        Some(self.gpu.halves_of(placed.largest()))
    }

    /// Which blocks of the label volume hold a part, as the device worked it out.
    #[cfg(test)]
    pub(crate) fn part_blocks_as_held(&self) -> Option<Vec<u16>> {
        Some(self.gpu.halves_of(self.parts.filled()?))
    }

    /// The coefficients of the curve planes are drawn from, as the device holds them.
    #[cfg(test)]
    pub(crate) fn curve_as_held(&self) -> Option<Vec<u16>> {
        Some(self.gpu.halves_of(self.curve.as_ref()?.texture()))
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
        let drawn = Gpu::on(device, queue).picture(size, |pass| self.plane.draw(pass));
        self.gpu.reported(drawn)
    }
}

#[cfg(test)]
mod cut_tests;
#[cfg(test)]
mod device_tests;
#[cfg(test)]
mod haze_tests;
#[cfg(test)]
mod held_tests;
#[cfg(test)]
mod label_tests;
#[cfg(test)]
mod lost_tests;
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
