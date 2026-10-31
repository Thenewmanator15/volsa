//! Saving the 3D scene to a file from the window: a picture, or a turntable drawn a picture
//! a frame and put together on a worker.

use egui_wgpu::RenderState;

use super::VoxaApp;
use super::scene::{self, export};
use crate::action::Action;

impl VoxaApp {
    /// Save the 3D scene as the person asked: a picture at once, a turntable a picture a frame
    /// with its progress shown and a way to stop. How it went is posted as a notice.
    pub(super) fn save_scene(
        &mut self,
        ctx: &egui::Context,
        render: Option<&RenderState>,
        asked: Option<export::Asked>,
        actions: &mut Vec<Action>,
    ) {
        if asked.is_some() || self.turntable.is_some() {
            self.hold_the_scene(render, actions);
        }
        let state = self.session.state();
        let viewport = state.scene.viewport;
        let wanted = asked.zip(scene::scene_params(state, 1.0));
        if let Some((asked, scene, path)) =
            wanted.and_then(|(asked, scene)| Some((asked, scene, export::choose(asked)?)))
        {
            if asked == export::Asked::Turntable {
                self.turntable = Some(export::Turntable::new(path, &scene, viewport));
            } else {
                let pixels = scene::pixels(viewport, ctx.pixels_per_point());
                let notice = match self.scene.snapshot(render, &scene, viewport, pixels) {
                    Some(Ok(rgba)) => export::save_picture(&path, &rgba, pixels),
                    Some(Err(why)) => export::not_drawn(&why),
                    None => return,
                };
                actions.push(Action::Post(notice));
            }
        }
        let Some(mut making) = self.turntable.take() else {
            return;
        };
        if export::progress(ctx, &making) {
            return;
        }
        let next = making
            .next()
            .map(|(scene, viewport, size)| (scene.clone(), viewport, size));
        let Some((scene, viewport, size)) = next else {
            // Putting the pictures together takes a moment: a worker does it, and says how
            // it went.
            let meanwhile = export::finish_on(self.session.workers(), making);
            actions.push(Action::Post(meanwhile));
            return;
        };
        match self.scene.snapshot(render, &scene, viewport, size) {
            Some(Ok(rgba)) => {
                making.take(rgba);
                self.turntable = Some(making);
                ctx.request_repaint();
            }
            Some(Err(why)) => actions.push(Action::Post(export::not_drawn(&why))),
            None => {}
        }
    }

    /// Put the scene's volume on the graphics device for a picture of it to be drawn: the
    /// arrangement shown may have no 3D view in it, which is what puts it there otherwise.
    fn hold_the_scene(&mut self, render: Option<&RenderState>, actions: &mut Vec<Action>) {
        let shown = self.session.state().scene.series.as_ref();
        let volume = shown.and_then(|series| self.session.volume(series));
        if let Some(halved) = self.scene.hold(render, volume.as_ref()) {
            actions.push(Action::SceneUploaded { halved });
        }
    }
}
