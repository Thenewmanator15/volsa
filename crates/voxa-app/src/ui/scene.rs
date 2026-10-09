//! The 3D viewport: the volume of the active viewport's series, turned with the mouse, with
//! a letter at each edge, a scale bar, and what the picture cannot show said beneath it.

use std::sync::Arc;
use std::time::Instant;

use egui::{Align2, Color32, PointerButton, Rect, Sense, Ui, pos2};
use egui_wgpu::RenderState;
use voxa_core::{Patient, Point, Volume};
use voxa_render::{SceneParams, pick};

use super::pen::{self, Pen};
use super::{overlay, theme, viewport};
use crate::action::Action;
use crate::scene::SceneStatus;
use crate::state::AppState;

mod captions;
mod gpu;
mod input;

pub use captions::{
    captions, edge_letters, quality, scene_params, status_text, until_full_quality,
};
pub use input::{SceneInput, scene_input_to_actions};

/// What to say when the volume is ready and this computer cannot draw it.
const CANNOT_DRAW: &str = "The 3D view cannot be drawn on this computer's graphics device.";

/// The most of the scene's width its captions may take, which keeps them clear of the scale
/// bar in the middle, and the least they are given.
const CAPTION_SHARE: f32 = 0.3;
const CAPTION_LEAST: f32 = 200.0;

/// What the 3D viewport keeps between frames.
pub struct SceneView {
    /// Empty when the window has no graphics device to draw a volume with.
    gpu: Option<gpu::SceneGpu>,
    /// What the picture was last drawn from, at full quality, and when that last changed:
    /// the picture is drawn coarsely until it has been still for a moment.
    shown: Option<SceneParams>,
    changed: Instant,
    /// Part of a wheel notch scrolled on a touchpad and not yet acted on.
    part_notch: f32,
}

impl SceneView {
    /// A viewport with nothing drawn yet.
    pub fn new(render: Option<&RenderState>) -> Self {
        Self {
            gpu: render.map(gpu::SceneGpu::new),
            shown: None,
            changed: Instant::now(),
            part_notch: 0.0,
        }
    }

    /// Whether a picture of a volume has been drawn, so there is something to see.
    pub fn has_picture(&self) -> bool {
        self.gpu.as_ref().is_some_and(gpu::SceneGpu::has_picture)
    }

    /// Draw the scene's picture if there is a volume and a device to draw it with, and give
    /// the name the interface shows it by. Reports, through `actions`, how often a volume
    /// just put on the device had to be halved.
    fn picture(
        &mut self,
        ui: &Ui,
        area: Rect,
        state: &AppState,
        volume: Option<&Arc<Volume>>,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) -> Option<egui::TextureId> {
        let ready = scene_params(state, 1.0)
            .zip(self.gpu.as_mut())
            .zip(render.zip(volume));
        let Some(((params, gpu), (render, volume))) = ready else {
            // Whatever was drawn before is of a volume no longer shown.
            self.shown = None;
            if let Some(gpu) = &mut self.gpu {
                gpu.forget();
            }
            return None;
        };
        if let Some(halved) = gpu.hold(render, volume) {
            actions.push(Action::SceneUploaded { halved });
        }
        if self.shown.as_ref() != Some(&params) {
            self.shown = Some(params.clone());
            self.changed = Instant::now();
        }
        let since = self.changed.elapsed();
        if let Some(left) = until_full_quality(since) {
            ui.ctx().request_repaint_after(left);
        }
        let quality = quality(since);
        let dots = quality * ui.ctx().pixels_per_point();
        let pixels = [area.width(), area.height()].map(|side| whole(side * dots));
        let params = SceneParams { quality, ..params };
        Some(gpu.draw(render, &params, [area.width(), area.height()], pixels))
    }
}

/// Draw the scene into `area` and turn what the mouse does over it into actions. `volume` is
/// the volume built for the scene's series, if it is ready.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    volume: Option<&Arc<Volume>>,
    view: &mut SceneView,
    render: Option<&RenderState>,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(area, ui.id().with("scene"), Sense::click_and_drag());
    ui.painter().rect_filled(area, 0, Color32::BLACK);
    let size = [area.width(), area.height()];
    let was = state.scene.viewport;
    // Told when it changes by half a point or more, not every frame.
    if size
        .iter()
        .zip(was)
        .any(|(now, was)| (now - was).abs() >= 0.5)
    {
        actions.push(Action::SceneResized { viewport: size });
    }
    let picture = view.picture(ui, area, state, volume, render, actions);
    if let Some(picture) = picture {
        let whole_picture = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        ui.painter()
            .image(picture, area, whole_picture, Color32::WHITE);
        written_over(ui, area, state);
    } else {
        let ready = state.scene.status == SceneStatus::Ready;
        let text = status_text(&state.scene).or_else(|| ready.then(|| CANNOT_DRAW.to_owned()));
        let font = egui::TextStyle::Body.resolve(ui.style());
        let middle = Align2::CENTER_CENTER;
        let text = text.unwrap_or_default();
        ui.painter()
            .text(area.center(), middle, text, font, theme::QUIET_TEXT);
    }
    // A double-click on something drawn puts that point in the middle.
    let pointed_at = response.double_clicked().then(|| {
        let at = response.interact_pointer_pos()? - area.min;
        pick(volume?, &scene_params(state, 1.0)?, size, [at.x, at.y])
    });
    let window_width = state
        .viewports
        .get(state.active)
        .map_or(1.0, |shown| shown.wl.width);
    let input = gather(
        ui,
        &response,
        area,
        pointed_at.flatten(),
        &mut view.part_notch,
    );
    let input = SceneInput {
        window_width,
        ..input
    };
    actions.extend(scene_input_to_actions(state.active, &input));
}

/// Write over the picture: a letter at each edge, a scale bar, and beneath them what the
/// picture cannot show.
fn written_over(ui: &Ui, area: Rect, state: &AppState) {
    let scene = &state.scene;
    let pen = Pen::over(ui, area);
    let inside = area.shrink(pen::INSET);
    pen::letters(&pen, inside, &edge_letters(&scene.camera));
    let mm_per_point = narrowed(scene.camera.mm_per_px);
    if let Some((length, label)) = overlay::scale_bar(mm_per_point, area.width() * pen::BAR_SHARE) {
        pen::bar(&pen, inside, length, &label);
    }
    let spacing = scene.series.as_ref().and_then(|id| state.series(id));
    let spacing = spacing.map_or([1.0; 3], |series| series.info.spacing_mm);
    // Kept to the left of the letter in the middle of the bottom edge.
    let said = captions(spacing, scene.halved).join(
        "
",
    );
    let room = (area.width() * CAPTION_SHARE).max(CAPTION_LEAST);
    pen.write_in_lines(inside.left_bottom(), &said, room);
}

/// What the mouse did over the scene this frame.
fn gather(
    ui: &Ui,
    response: &egui::Response,
    area: Rect,
    pointed_at: Option<Point<Patient>>,
    part_notch: &mut f32,
) -> SceneInput {
    let dragged = response.drag_delta();
    let dragged_with = |button| {
        if response.dragged_by(button) {
            [dragged.x, dragged.y]
        } else {
            [0.0; 2]
        }
    };
    let mut input = SceneInput {
        left_drag: dragged_with(PointerButton::Primary),
        middle_drag: dragged_with(PointerButton::Middle),
        right_drag: dragged_with(PointerButton::Secondary),
        shift: ui.input(|i| i.modifiers.shift),
        pressed: response.is_pointer_button_down_on() && ui.input(|i| i.pointer.any_pressed()),
        double_clicked_surface: pointed_at,
        ..SceneInput::default()
    };
    if let Some(at) = response.hover_pos() {
        let from_middle = at - area.center();
        input.pointer = [from_middle.x, from_middle.y];
        (input.wheel, _) = viewport::wheel_turned(ui, part_notch);
    }
    input
}

/// A length on the screen as a whole number of pixels, at least one.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 1 and the largest picture first"
)]
fn whole(length: f32) -> u32 {
    length.round().clamp(1.0, 16_384.0) as u32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "millimetres to a pixel is a small number"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests;
