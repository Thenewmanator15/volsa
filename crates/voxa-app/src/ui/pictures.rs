//! The middle of the window: the 3D scene, the plane views and the 2D viewports, each in the
//! part of it the layout gives them.

use std::sync::Arc;

use egui_wgpu::RenderState;
use voxa_core::Volume;
use voxa_render::VolumePipeline;

use super::{VoxaApp, labels, layout, planes, scene, shares, viewport};
use crate::action::Action;
use crate::scene::SceneStatus;
use crate::session::Session;

impl VoxaApp {
    /// Draw the 3D scene, the planes and the 2D viewports in what is left of the window. One
    /// of them has the large part and another a column beside it, under a strip that says
    /// what the column shows and has the button that enlarges it. The line between the two
    /// can be dragged to give the column more or less of the width.
    pub(super) fn pictures(
        &mut self,
        ui: &mut egui::Ui,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) {
        let state = self.session.state();
        let whole = ui.available_rect_before_wrap();
        let share = self.screen.column_share();
        let layout::Panels { main, side } = layout::panels(whole, share);
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
        // One copy of the labels on the device, for the planes and the scene to draw from.
        let set = self.session.labels();
        if self.held.keep(render, state, set.as_ref()) {
            actions.push(Action::Post(labels::no_room()));
        }
        let (on_device, stamp) = (self.held.on_device(), self.held.stamp());
        self.planes.label(on_device, state.labels.overlay);
        self.scene.label(render, on_device, stamp);
        let scene = &mut self.scene;
        let volume = volume.as_ref();
        let sources = (volume, set.as_deref());
        scene::show(ui, areas.scene, state, sources, scene, render, actions);
        let views = (&self.scene, &mut self.planes);
        Self::smooth_planes(&self.session, views, render, volume, actions);
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
        ui.allocate_rect(whole, egui::Sense::hover());
        // After everything beside it, so that the drag is the line's.
        self.drag_the_column(ui, whole, f32::midpoint(main.right(), side.left()));
    }

    /// Let the line at `between` the large part and the column, down the middle of the
    /// window (`whole`), be dragged to give the column more or less of its width.
    fn drag_the_column(&mut self, ui: &egui::Ui, whole: egui::Rect, between: f32) {
        let edge = egui::Id::new("column edge");
        let by = shares::divider(ui, edge, between, whole.y_range());
        if by.abs() > 0.0 {
            let share = self.screen.column_share();
            let share = layout::column_dragged(whole.width(), share, by);
            self.screen.set_column_share(share);
        }
    }

    /// Have the planes drawn smooth, from the curve through the scene's volume, while that
    /// is wanted; in sight or not, a curve that is no longer wanted is given up at once.
    /// Where the graphics device makes the curve itself it does so from the volume it
    /// holds, as soon as the scene has put it there; otherwise a worker's is taken up.
    fn smooth_planes(
        session: &Session,
        (scene, planes): (&scene::SceneView, &mut planes::PlanesView),
        render: Option<&RenderState>,
        volume: Option<&Arc<Volume>>,
        actions: &mut Vec<Action>,
    ) {
        let state = session.state();
        let shown = state.scene.series.as_ref();
        let smoothed = volume.filter(|_| state.planes.smooth);
        let take = || shown.and_then(|series| session.take_smoothed(series));
        let held = volume.is_some_and(|volume| !scene.needs(volume));
        let makes = |pipeline: &&VolumePipeline| {
            held && render.is_some_and(|render| pipeline.makes_curves(&render.device))
        };
        let on_device = scene.volume_pipeline().filter(makes);
        match planes.smooth(render, smoothed, on_device, take) {
            planes::Smoothing::NoRoom => actions.push(Action::Post(planes::no_room_to_smooth())),
            planes::Smoothing::LeftToAWorker => {
                // The device had no room to work it out: a worker does, and hands it over.
                if let Some(series) = shown {
                    session.workers().work_out_curve(series.clone());
                }
            }
            planes::Smoothing::Nothing => {}
        }
    }

    /// Whether the 2D viewports are out of sight, their place in the column taken by the
    /// planes.
    pub(super) fn slices_hidden(&self) -> bool {
        !crate::planes::slices_in_sight(self.session.state())
    }
}
