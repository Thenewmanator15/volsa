//! Saving the 3D scene to a file from the window: a picture, or a turntable drawn a picture
//! a frame.

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
            actions.push(Action::Post(making.finish()));
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
}
