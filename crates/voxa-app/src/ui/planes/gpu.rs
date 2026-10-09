//! A plane view on the graphics device: its pipeline, drawn inside the interface's own pass.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Rect, Ui};
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, wgpu};
use voxa_core::WindowLevel;
use voxa_render::{PlanePipeline, PlaneView, SmoothedTexture, VolumePipeline};

/// What one plane's panel keeps on the graphics device between frames.
pub struct PlaneGpu {
    pipeline: Arc<Mutex<PlanePipeline>>,
}

impl PlaneGpu {
    /// A pipeline drawing into the window's own format, with no volume to draw from yet.
    pub fn new(render: &RenderState) -> Self {
        let pipeline = PlanePipeline::new(&render.device, render.target_format);
        Self {
            pipeline: Arc::new(Mutex::new(pipeline)),
        }
    }

    /// Draw from the curve through the scene's volume from now on, or plainly with `None`.
    pub fn smooth(&self, with: Option<&SmoothedTexture>) {
        let mut pipeline = self.pipeline.lock().unwrap_or_else(PoisonError::into_inner);
        pipeline.smooth(with);
    }

    /// Whether the plane last queued is drawn from the curve.
    pub fn is_smooth(&self) -> bool {
        let pipeline = self.pipeline.lock().unwrap_or_else(PoisonError::into_inner);
        pipeline.is_smooth()
    }

    /// Queue the drawing of a plane into `cell`, from the volume the scene holds, through
    /// `window`. False, with nothing queued, when the scene holds no volume.
    pub fn draw(
        &self,
        ui: &Ui,
        cell: Rect,
        render: &RenderState,
        volume: &VolumePipeline,
        view: &PlaneView,
        window: WindowLevel,
    ) -> bool {
        let mut pipeline = self.pipeline.lock().unwrap_or_else(PoisonError::into_inner);
        // Every frame: the scene's volume may have been replaced since the last one.
        pipeline.show(&render.device, volume);
        if !pipeline.has_volume() {
            return false;
        }
        pipeline.set_params(&render.queue, view, [cell.width(), cell.height()], window);
        let draw = DrawPlane(Arc::clone(&self.pipeline));
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(cell, draw));
        true
    }
}

/// Draws a plane inside egui's own render pass.
struct DrawPlane(Arc<Mutex<PlanePipeline>>);

impl CallbackTrait for DrawPlane {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &CallbackResources,
    ) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .draw(pass);
    }
}
