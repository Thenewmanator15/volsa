//! The curve the planes are drawn from when they are drawn smooth: made by the graphics
//! device from the volume it holds where it can, and otherwise taken from a worker.

use std::sync::Arc;

use egui_wgpu::RenderState;
use voxa_core::Volume;
use voxa_render::{Smoothed, SmoothedTexture, VolumePipeline};

use super::PlanesView;

/// What came of wanting the planes drawn smooth, this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Smoothing {
    /// Nothing to say: there is a curve, or one is on its way, or none is wanted.
    Nothing,
    /// A worker's curve was there and the graphics device had no room for it.
    NoRoom,
    /// The graphics device had no room to work the curve out: a worker is to be asked.
    LeftToAWorker,
}

impl PlanesView {
    /// Draw the planes smooth, from the curve through `volume`, or plainly with `None`.
    /// Where the graphics device makes the curve itself, `on_device` is the pipeline that
    /// holds `volume` there, and the curve is made at once. Otherwise `take` hands over
    /// the curve once a worker has made it; it is asked each frame until then, and the
    /// planes are drawn plainly meanwhile. A curve of another volume is given up.
    pub fn smooth(
        &mut self,
        render: Option<&RenderState>,
        volume: Option<&Arc<Volume>>,
        on_device: Option<&VolumePipeline>,
        take: impl FnOnce() -> Option<Smoothed>,
    ) -> Smoothing {
        let its_own = |(of, _): &(Arc<Volume>, SmoothedTexture)| {
            volume.is_some_and(|volume| Arc::ptr_eq(of, volume))
        };
        if self.curve.as_ref().is_some_and(|held| !its_own(held)) {
            self.set_curve(None);
        }
        let same =
            |refused: &Arc<Volume>| volume.is_some_and(|volume| Arc::ptr_eq(refused, volume));
        self.refused = self.refused.take().filter(same);
        let (Some(volume), None) = (volume, &self.curve) else {
            return Smoothing::Nothing;
        };
        let asked = on_device.filter(|_| self.refused.is_none()).zip(render);
        if let Some((pipeline, render)) = asked {
            let Some(made) = pipeline.curve_on_device(&render.device, &render.queue) else {
                // No room to work it out there: a worker does, and hands it over.
                self.refused = Some(Arc::clone(volume));
                return Smoothing::LeftToAWorker;
            };
            self.set_curve(Some((Arc::clone(volume), made)));
            return Smoothing::Nothing;
        }
        let Some((curve, render)) = take().zip(render) else {
            return Smoothing::Nothing;
        };
        let placed = curve.on(&render.device, &render.queue);
        let came = if placed.is_some() {
            Smoothing::Nothing
        } else {
            Smoothing::NoRoom
        };
        self.set_curve(placed.map(|placed| (Arc::clone(volume), placed)));
        came
    }

    /// Whether the planes are drawn smooth now.
    pub fn is_smooth(&self) -> bool {
        self.curve.is_some()
    }

    /// Hold a curve, or none, and have every panel draw from it.
    fn set_curve(&mut self, curve: Option<(Arc<Volume>, SmoothedTexture)>) {
        self.curve = curve;
        let placed = self.curve.as_ref().map(|(_, placed)| placed);
        for gpu in self.gpus.iter().flatten() {
            gpu.smooth(placed);
        }
    }
}
