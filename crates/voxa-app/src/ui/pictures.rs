//! The middle of the window: the 3D scene, the plane views and the 2D viewports, each in the
//! part of it the layout gives them.

use egui_wgpu::RenderState;

use super::{VoxaApp, labels, layout, parts, planes, scene, viewport};
use crate::action::Action;
use crate::scene::SceneStatus;

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
        // In sight or not: a curve that is no longer wanted is given up at once.
        let session = &self.session;
        let smoothed = volume.filter(|_| state.planes.smooth);
        let take = || shown.and_then(|series| session.take_smoothed(series));
        if self.planes.smooth(render, smoothed, take) {
            actions.push(Action::Post(planes::no_room_to_smooth()));
        }
        // One copy of the labels on the device, for the planes and the scene to draw from.
        let set = self.session.labels();
        if self.held.keep(render, state, set.as_ref()) {
            actions.push(Action::Post(labels::no_room()));
        }
        self.planes
            .label(self.held.on_device(), state.labels.overlay);
        if let Some(cells) = areas.planes {
            // After the scene, which is what puts the volume on the graphics device.
            let held = self.scene.volume_pipeline();
            let on_device = held.zip(render);
            let scan = volume.map(|volume| &**volume);
            planes::show(
                ui,
                cells,
                state,
                (on_device, scan, set.as_deref()),
                &mut self.planes,
                actions,
            );
        }
        if let Some(for_slices) = areas.slices {
            let (session, gpus) = (&self.session, &mut self.gpus);
            let frames = |series: &_, slice| session.cached(series, slice);
            viewport::show(ui, for_slices, state, &frames, gpus, render, actions);
        }
        if let Some(area) = areas.parts {
            let asked = parts::show(ui, area, state, &mut self.parts, actions);
            actions.extend(asked.and_then(|asked| self.labels.chosen(asked)));
        }
        ui.allocate_rect(whole, egui::Sense::hover());
    }

    /// Whether the 2D viewports are out of sight, their place in the column taken by the
    /// planes or the parts.
    pub(super) fn slices_hidden(&self) -> bool {
        !crate::planes::slices_in_sight(self.session.state())
    }
}
