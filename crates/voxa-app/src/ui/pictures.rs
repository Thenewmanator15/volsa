//! The middle of the window: the 3D scene, the plane views and the 2D viewports, each in the
//! part of it the layout gives them.

use egui_wgpu::RenderState;

use super::{VoxaApp, layout, planes, scene, viewport};
use crate::action::Action;
use crate::scene::{MainPanel, SceneStatus};

impl VoxaApp {
    /// Draw the 3D scene, the planes and the 2D viewports in what is left of the window. One
    /// of them has the large part and another a column beside it, under a strip that says
    /// what the column shows and has the button that enlarges it.
    pub(super) fn pictures(
        &mut self,
        ui: &mut egui::Ui,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) {
        let state = self.session.state();
        let whole = ui.available_rect_before_wrap();
        let layout::Panels { main, side } = layout::panels(whole);
        let (strip, side) = layout::header_and_rest(side);
        // A series with no 3D view has its slices in the large part: they are all there is.
        let large = state.main.given(&state.scene.status);
        let column = state.planes.column;
        let there_is_3d = !matches!(state.scene.status, SceneStatus::Unavailable(_));
        layout::header(ui, strip, (large, column), there_is_3d, actions);
        let areas = layout::areas(large, column, main, side);
        let shown = state.scene.series.as_ref();
        let volume = shown.and_then(|series| self.session.volume(series));
        // If it has still to go on the graphics device, take what a worker made ready of it.
        let waiting = volume
            .as_ref()
            .is_some_and(|volume| self.scene.needs(volume));
        let ready = shown.filter(|_| waiting);
        self.scene
            .offer(ready.and_then(|series| self.session.take_prepared(series)));
        let scene = &mut self.scene;
        let volume = volume.as_ref();
        scene::show(ui, areas.scene, state, volume, scene, render, actions);
        if let Some(cells) = areas.planes {
            // After the scene, which is what puts the volume on the graphics device.
            let held = self.scene.volume_pipeline();
            planes::show(ui, cells, state, held, &mut self.planes, render, actions);
        }
        if let Some(for_slices) = areas.slices {
            let (session, gpus) = (&self.session, &mut self.gpus);
            let frames = |series: &_, slice| session.cached(series, slice);
            viewport::show(ui, for_slices, state, &frames, gpus, render, actions);
        }
        ui.allocate_rect(whole, egui::Sense::hover());
    }

    /// Whether the 2D viewports are out of sight, their place in the column taken by the
    /// planes.
    pub(super) fn slices_hidden(&self) -> bool {
        let state = self.session.state();
        let large = state.main.given(&state.scene.status);
        large == MainPanel::Scene && crate::planes::in_sight(state)
    }
}
