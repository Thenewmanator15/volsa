//! Drawing the 3D scene's picture: the volume as the graphics device holds it, or the preview
//! of a long series while it builds, as finely as the device keeps up with.

use std::sync::Arc;
use std::time::Instant;

use egui::{Rect, Ui};
use egui_wgpu::RenderState;
use volsa_core::HeldVolume;
use volsa_render::SceneParams;

use super::{SceneView, pace, quality, scene_params, until_full_quality, whole};
use crate::action::Action;
use crate::state::AppState;

impl SceneView {
    /// Draw the scene's picture if there is a volume and a device to draw it with, and give
    /// the name the interface shows it by. Reports, through `actions`, how often a volume
    /// just put on the device had to be halved.
    pub(super) fn picture(
        &mut self,
        ui: &Ui,
        area: Rect,
        state: &AppState,
        volume: Option<&Arc<HeldVolume>>,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) -> Option<egui::TextureId> {
        let shown = state.scene.series.as_ref();
        if let Some(gpu) = self.gpu.as_mut() {
            // A preview, and a volume on its way, are of their own series only.
            if self.preview_of.as_ref() != shown {
                gpu.drop_preview();
                self.preview_of = None;
            }
            if volume.is_none() {
                gpu.give_up_upload();
                self.card = None;
            }
        }
        let ready = scene_params(state, 1.0).zip(self.gpu.as_mut()).zip(render);
        let Some(((params, gpu), render)) =
            ready.filter(|_| volume.is_some() || state.scene.preview)
        else {
            // Whatever was drawn before is of a volume no longer shown.
            self.shown = None;
            if let Some(gpu) = &mut self.gpu {
                gpu.forget();
            }
            return None;
        };
        let drawable = if let Some(volume) = volume {
            let card = (self.card.take(), self.room);
            if let Some(halved) = gpu.hold(render, volume, card, ui.ctx()) {
                actions.push(Action::SceneUploaded { halved });
            }
            gpu.has_volume()
        } else {
            // While its series builds, the preview of it, and only of it.
            let offered = self.preview.take().filter(|(of, _)| shown == Some(of));
            let offered = offered.map(|(of, preview)| {
                self.preview_of = Some(of);
                preview
            });
            gpu.preview(render, offered)
        };
        if !drawable {
            // The device had no room for it at all: there is nothing to draw.
            gpu.forget();
            return None;
        }
        if self.shown.as_ref() != Some(&params) {
            self.shown = Some(params.clone());
            self.changed = Instant::now();
        }
        let since = self.changed.elapsed();
        if let Some(left) = until_full_quality(since) {
            ui.ctx().request_repaint_after(left);
        }
        // While it moves it is drawn as finely as the device has shown it keeps up with.
        let size = [area.width(), area.height()];
        let per_point = ui.ctx().pixels_per_point();
        let in_full = size.map(|side| whole(side * per_point));
        let quality = quality(since, gpu.full_took(in_full, &params));
        let pixels = size.map(|side| whole(side * quality * per_point));
        let params = SceneParams { quality, ..params };
        let picture = gpu.draw(render, &params, size, pixels);
        if gpu.wants_timing(in_full, &params) {
            // It was busy when the full picture was drawn: time one when it is not.
            ui.ctx().request_repaint_after(pace::TIME_AGAIN);
        }
        Some(picture)
    }
}
