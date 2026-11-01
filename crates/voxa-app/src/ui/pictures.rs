//! The middle of the window: the strip that arranges the pictures, and beneath it the 3D
//! scene, the plane views and the 2D viewports, as many of them as the arrangement shows, each
//! in the part of it the layout gives them.

use std::sync::Arc;

use egui_wgpu::RenderState;
use voxa_core::HeldVolume;
use voxa_labels::LabelSet;
use voxa_render::VolumePipeline;

use super::{VoxaApp, document, labels, layout, planes, scene, strip, theme, viewport, welcome};
use crate::action::Action;
use crate::measuring::SliceValues;
use crate::session::Session;
use crate::state::AppState;

impl VoxaApp {
    /// Show the document that is open, if one is, in place of the strip and the pictures, and
    /// give up the slices' room on the graphics device. Whether there was one.
    fn show_document(
        &mut self,
        ui: &mut egui::Ui,
        whole: egui::Rect,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) -> bool {
        let state = self.session.state();
        let Some(open) = &state.document else {
            return false;
        };
        let asked = document::show(ui, whole, state, open, &mut self.document_view, actions);
        self.give_up_the_slices(ui, whole, render, actions);
        for request in asked {
            self.ask(ui.ctx(), request);
        }
        true
    }

    /// Say how to begin, with the ways to open something, in place of the pictures while
    /// nothing is open. Whether it did.
    fn show_how_to_begin(
        &mut self,
        ui: &mut egui::Ui,
        whole: egui::Rect,
        actions: &mut Vec<Action>,
    ) -> bool {
        let state = self.session.state();
        if !welcome::is_empty(state) {
            return false;
        }
        let asked = welcome::show(ui, whole, state, actions);
        ui.allocate_rect(whole, egui::Sense::hover());
        for request in asked {
            self.ask(ui.ctx(), request);
        }
        true
    }

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
        let whole = ui.available_rect_before_wrap();
        // Before anything is drawn, whatever stands in the middle of the window.
        let views = (&mut self.scene, &mut self.planes);
        let volume = settle_volume(&self.session, views, (render, ui.ctx()), actions);
        if self.show_document(ui, whole, render, actions)
            || self.show_how_to_begin(ui, whole, actions)
        {
            return;
        }
        theme::on_the_pictures_ground(ui);
        let state = self.session.state();
        let (along, rest) = layout::strip_and_pictures(whole);
        let asked = strip::show(ui, along, state, actions);
        // A scan with no 3D view has its slices alone: they are all there is.
        let areas = layout::areas(state.pictures.given(&state.scene.status), rest);
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

    /// With a document open in the place of the pictures, the 2D viewports are out of sight:
    /// what was half drawn on them is given up, and `whole` is taken up.
    fn give_up_the_slices(
        &mut self,
        ui: &mut egui::Ui,
        whole: egui::Rect,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) {
        let held = (&mut self.gpus[..], &mut self.drafts);
        slices(ui, None, (&self.session, held), render, actions);
        ui.allocate_rect(whole, egui::Sense::hover());
    }

    /// Offer the scene what the graphics device is to hold of its series: the volume if it has
    /// still to go there, or a preview of it while it builds. Gives the volume, if built.
    fn offer_to_the_scene(
        session: &Session,
        scene: &mut scene::SceneView,
    ) -> Option<Arc<HeldVolume>> {
        let state = session.state();
        let shown = state.scene.series.as_ref();
        let volume = shown.and_then(|series| session.volume(series));
        let waiting = volume.as_ref().is_some_and(|volume| scene.needs(volume));
        let ready = shown.filter(|_| waiting);
        scene.offer(ready.and_then(|series| session.for_card(series)));
        let previewed = shown.filter(|_| state.scene.preview);
        let preview =
            previewed.and_then(|series| Some((series.clone(), session.take_preview(series)?)));
        scene.offer_preview(preview);
        volume
    }

    /// Have the planes drawn smooth, from the curve through the scene's volume, while that
    /// is wanted; in sight or not, a curve that is no longer wanted is given up at once.
    /// Where the graphics device makes the curve itself it does so from the volume it
    /// holds, as soon as the scene has put it there; otherwise a worker's is taken up.
    fn smooth_planes(
        session: &Session,
        (scene, planes): (&scene::SceneView, &mut planes::PlanesView),
        render: Option<&RenderState>,
        volume: Option<&Arc<HeldVolume>>,
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

/// The volume of the scene's series, with what the graphics device is to hold of it (and, while
/// it builds, its preview) offered to the scene, which hands it over a slab a frame as it is
/// drawn. This is the first thing the middle of the window does, whatever it then shows: with
/// a document in the place of the pictures nothing is drawn to hand the volume over, so it goes
/// on being handed over here, and a smooth curve nobody draws is let go.
fn settle_volume(
    session: &Session,
    (scene, planes): (&mut scene::SceneView, &mut planes::PlanesView),
    (render, ctx): (Option<&RenderState>, &egui::Context),
    actions: &mut Vec<Action>,
) -> Option<Arc<HeldVolume>> {
    let volume = VoxaApp::offer_to_the_scene(session, scene);
    if session.state().document.is_some() {
        let held = volume.as_ref();
        keep_on_card(scene, (render, ctx), held, actions);
        VoxaApp::smooth_planes(session, (scene, planes), render, held, actions);
    }
    volume
}

/// Go on putting the scene's volume on the graphics card, a slab a frame, with no 3D view
/// drawn to do it, and say how often it had to be halved, once the whole of it is there.
fn keep_on_card(
    view: &mut scene::SceneView,
    (render, ctx): (Option<&RenderState>, &egui::Context),
    volume: Option<&Arc<HeldVolume>>,
    actions: &mut Vec<Action>,
) {
    let halved = view.keep_on_card(render, volume, ctx);
    actions.extend(halved.map(|halved| Action::SceneUploaded { halved }));
}

/// Draw the 3D scene in `area`; or, with no area for it in the arrangement shown, keep its
/// volume on the graphics card all the same, so that no second copy of it waits in memory.
fn scene_or_card(
    ui: &mut egui::Ui,
    area: Option<egui::Rect>,
    state: &AppState,
    sources: (Option<&Arc<HeldVolume>>, Option<&LabelSet>),
    view: &mut scene::SceneView,
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    if let Some(area) = area {
        scene::show(ui, area, state, sources, view, render, actions);
    } else {
        let (volume, _) = sources;
        keep_on_card(view, (render, ui.ctx()), volume, actions);
    }
}

#[cfg(test)]
mod tests;
