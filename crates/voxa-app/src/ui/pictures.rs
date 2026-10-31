//! The middle of the window: the strip that arranges the pictures, and beneath it the 3D
//! scene, the plane views and the 2D viewports, as many of them as the arrangement shows, each
//! in the part of it the layout gives them.

use std::sync::Arc;

use egui_wgpu::RenderState;
use voxa_core::Volume;
use voxa_labels::LabelSet;
use voxa_render::VolumePipeline;

use super::{VoxaApp, labels, layout, planes, scene, strip, viewport};
use crate::action::Action;
use crate::measuring::SliceValues;
use crate::session::Session;
use crate::state::AppState;

impl VoxaApp {
    /// Draw the strip and, beneath it, the pictures of the arrangement shown in what is left of
    /// the window. The 3D scene is drawn only when the arrangement has it; without it, its
    /// volume is put on the graphics card all the same. The volume becomes ready either way, as
    /// the worker that builds it says so, and measuring needs no picture of it.
    pub(super) fn pictures(
        &mut self,
        ui: &mut egui::Ui,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) {
        let state = self.session.state();
        let whole = ui.available_rect_before_wrap();
        let (along, rest) = layout::strip_and_pictures(whole);
        let asked = strip::show(ui, along, state, actions);
        // A scan with no 3D view has its slices alone: they are all there is.
        let areas = layout::areas(state.pictures.given(&state.scene.status), rest);
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
        let volume = volume.as_ref();
        let sources = (volume, set.as_deref());
        scene_or_card(
            ui,
            areas.scene,
            state,
            sources,
            &mut self.scene,
            render,
            actions,
        );
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
        let held = (&mut self.gpus[..], &mut self.drafts);
        slices(ui, areas.slices, (&self.session, held), render, actions);
        ui.allocate_rect(whole, egui::Sense::hover());
        for request in asked {
            self.ask(ui.ctx(), request);
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

    /// Whether the 2D viewports are out of sight, the arrangement shown having none.
    pub(super) fn slices_hidden(&self) -> bool {
        !crate::planes::slices_in_sight(self.session.state())
    }

    /// Whether the 3D scene is in sight, the arrangement shown having it.
    pub(super) fn scene_in_sight(&self) -> bool {
        let state = self.session.state();
        state.pictures.given(&state.scene.status).has_scene()
    }
}

/// Draw the 2D viewports in `area`, reading their slices from the session: whole, to draw
/// them, and in place, for the value under the pointer and for what is measured on them. With
/// no area, the arrangement shown has no slices, and what was half drawn on them is given up.
fn slices(
    ui: &mut egui::Ui,
    area: Option<egui::Rect>,
    (session, held): (&Session, (&mut [viewport::Gpu], &mut viewport::Drafts)),
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    let frame = |series: &_, slice| session.cached(series, slice);
    let value = |series: &_, slice, pixel| session.value_at(series, slice, pixel);
    let read = |series: &_, slice, with: &mut dyn FnMut(SliceValues<'_>)| {
        let _ = session.read_slice(series, slice, with);
    };
    let slices = viewport::Slices {
        frame: &frame,
        value: &value,
        read: &read,
    };
    viewport::show(ui, area, session.state(), &slices, held, render, actions);
}

/// Draw the 3D scene in `area`; or, with no area for it in the arrangement shown, keep its
/// volume on the graphics card all the same, so that no second copy of it waits in memory.
fn scene_or_card(
    ui: &mut egui::Ui,
    area: Option<egui::Rect>,
    state: &AppState,
    sources: (Option<&Arc<Volume>>, Option<&LabelSet>),
    view: &mut scene::SceneView,
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    if let Some(area) = area {
        scene::show(ui, area, state, sources, view, render, actions);
    } else {
        let (volume, _) = sources;
        let halved = view.keep_on_card(render, volume);
        actions.extend(halved.map(|halved| Action::SceneUploaded { halved }));
    }
}

#[cfg(test)]
mod tests;
