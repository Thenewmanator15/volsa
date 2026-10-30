//! The 3D viewport: the volume of the active viewport's series, turned with the mouse, with
//! a letter at each edge, a scale bar, and what the picture cannot show said beneath it.

use std::sync::Arc;
use std::time::Instant;

use egui::{Align2, Color32, PointerButton, Rect, Sense, Ui, pos2};
use egui_wgpu::RenderState;
use voxa_core::{HeldVolume, Patient, Point};
use voxa_labels::LabelSet;
use voxa_render::{LabelsOnDevice, Prepared, RenderError, SceneParams, VolumePipeline, pick};

use super::pen::{self, Pen};
use super::{overlay, theme, viewport};
use crate::action::Action;
use crate::annotations::Tool;
use crate::labels::name_of;
use crate::scene::SceneStatus;
use crate::state::AppState;

mod captions;
pub mod export;
mod gpu;
mod handles;
mod input;
mod pace;
mod parts;
mod tools;
pub mod views;

pub use captions::{
    captions, edge_letters, quality, scene_params, status_text, until_full_quality,
};
pub use input::{SceneInput, scene_input_to_actions};

/// What to say when the volume is ready and this computer cannot draw it.
pub(super) const CANNOT_DRAW: &str =
    "The 3D view cannot be drawn on this computer's graphics device.";

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
    /// What has been dragged on a handle short of a whole step.
    carried: handles::Carried,
    /// The scene's volume as a worker made it ready for the device, until it is handed over.
    ready: Option<Prepared>,
    /// The first end of a ruler being clicked in the scene, once it has been placed.
    ruler_from: Option<Point<Patient>>,
}

impl SceneView {
    /// A viewport with nothing drawn yet.
    pub fn new(render: Option<&RenderState>) -> Self {
        Self {
            gpu: render.map(gpu::SceneGpu::new),
            shown: None,
            changed: Instant::now(),
            part_notch: 0.0,
            carried: handles::Carried::default(),
            ready: None,
            ruler_from: None,
        }
    }

    /// Whether this volume has still to be put on the graphics device.
    pub fn needs(&self, volume: &Arc<HeldVolume>) -> bool {
        self.gpu.as_ref().is_some_and(|gpu| !gpu.holds(volume))
    }

    /// Give the viewport its volume as a worker made it ready, to hand over when it draws.
    pub fn offer(&mut self, ready: Option<Prepared>) {
        if ready.is_some() {
            self.ready = ready;
        }
    }

    /// Whether a picture of a volume has been drawn, so there is something to see.
    pub fn has_picture(&self) -> bool {
        self.gpu.as_ref().is_some_and(gpu::SceneGpu::has_picture)
    }

    /// The pipeline that holds the scene's volume on the graphics device, for what else
    /// draws from the same volume.
    pub fn volume_pipeline(&self) -> Option<&VolumePipeline> {
        self.gpu.as_ref().map(gpu::SceneGpu::volume_pipeline)
    }

    /// Draw the Parts look from these labels, or have none to draw it from. `stamp` moves
    /// on whenever the labels, or how their parts look, change.
    pub fn label(
        &mut self,
        render: Option<&RenderState>,
        with: Option<&LabelsOnDevice>,
        stamp: u64,
    ) {
        if let Some((gpu, render)) = self.gpu.as_mut().zip(render) {
            gpu.label(render, with, stamp);
        }
    }

    /// Draw the scene into a picture for saving: `pixels` wide and high, laid out as at
    /// `viewport`. `None` when there is no graphics device to draw with.
    pub fn snapshot(
        &mut self,
        render: Option<&RenderState>,
        params: &SceneParams,
        viewport: [f32; 2],
        pixels: [u32; 2],
    ) -> Option<Result<Vec<u8>, RenderError>> {
        let (gpu, render) = (self.gpu.as_mut()?, render?);
        Some(gpu.snapshot(render, params, viewport, pixels))
    }

    /// Draw the scene's picture if there is a volume and a device to draw it with, and give
    /// the name the interface shows it by. Reports, through `actions`, how often a volume
    /// just put on the device had to be halved.
    fn picture(
        &mut self,
        ui: &Ui,
        area: Rect,
        state: &AppState,
        volume: Option<&Arc<HeldVolume>>,
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
        if let Some(halved) = gpu.hold(render, volume, self.ready.take()) {
            actions.push(Action::SceneUploaded { halved });
        }
        if !gpu.has_volume() {
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

/// Draw the scene into `area` and turn what the mouse does over it into actions. `volume` is
/// the volume built for the scene's series, if it is ready, and `labels` the labels read.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    (volume, labels): (Option<&Arc<HeldVolume>>, Option<&LabelSet>),
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
    if state.scene.status == SceneStatus::Ready {
        // Drawn after the scene itself, so a press on a handle takes hold of the handle.
        handles::show(ui, area, state, &mut view.carried, actions);
        tools::draw(ui, area, state, view.ruler_from);
    }
    let sources = (volume, labels);
    let twice = pointed(
        state,
        sources,
        &response,
        area,
        &mut view.ruler_from,
        actions,
    );
    let window_width = state
        .viewports
        .get(state.active)
        .map_or(1.0, |shown| shown.wl.width);
    let input = gather(ui, &response, area, twice, &mut view.part_notch);
    let input = SceneInput {
        window_width,
        ..input
    };
    actions.extend(scene_input_to_actions(state.active, &input));
}

/// Act on a click in the scene and on the pointer resting there. What a click on something
/// drawn does depends on the tool in hand. In the Parts look what is pointed at is a solid,
/// which the click also selects and the resting pointer is told the name of; in the others
/// it is a surface of the scan. Gives the point a double-click landed on while navigating,
/// which is put in the middle as well.
fn pointed(
    state: &AppState,
    (volume, labels): (Option<&Arc<HeldVolume>>, Option<&LabelSet>),
    response: &egui::Response,
    area: Rect,
    ruler_from: &mut Option<Point<Patient>>,
    actions: &mut Vec<Action>,
) -> Option<Point<Patient>> {
    let size = [area.width(), area.height()];
    let seen = |at: egui::Pos2| {
        let at = at - area.min;
        parts::part_seen(state, labels?, size, [at.x, at.y])
    };
    if state.tool != Tool::Ruler {
        *ruler_from = None;
    }
    let mut pointed_at = None;
    if response.clicked() {
        let at = response.interact_pointer_pos();
        let part = at.and_then(seen);
        pointed_at = if parts::drawn(state) {
            part.map(|(_, point)| point)
        } else {
            at.and_then(|at| {
                let at = at - area.min;
                pick(
                    volume?.as_ref(),
                    &scene_params(state, 1.0)?,
                    size,
                    [at.x, at.y],
                )
            })
        };
        let (begun, asked) = tools::clicked(state.tool, *ruler_from, pointed_at);
        *ruler_from = begun;
        actions.extend(asked);
        // The click that moves the position also selects the part clicked.
        actions.extend(parts::selected_by(state, part.map(|(part, _)| part)));
    }
    let rests = response.hover_pos().filter(|_| !response.dragged());
    let named = rests.and_then(seen);
    if let Some(part) = named.and_then(|(part, _)| state.labels.parts.get(part)) {
        response.clone().on_hover_text_at_pointer(name_of(part));
    }
    let navigating = state.tool == Tool::Navigate && response.double_clicked();
    pointed_at.filter(|_| navigating)
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
    // At the top left, which nothing else uses, kept clear of the letter at the top.
    // The solids are not the scan: what is said of the scan's picture does not apply.
    let mut said = if parts::drawn(state) {
        vec![captions::solids(state)]
    } else {
        captions(spacing, scene.halved)
    };
    said.extend(tools::said(state).into_iter().map(str::to_owned));
    let said = said.join("\n");
    let (down, room) = captions::caption_room(inside.width());
    pen.write_wrapped(inside.left_top() + egui::vec2(0.0, down), &said, room);
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

/// The size in pixels of a picture laid out at `viewport` on a screen with this many pixels
/// to the point.
pub fn pixels(viewport: [f32; 2], pixels_per_point: f32) -> [u32; 2] {
    viewport.map(|side| whole(side * pixels_per_point))
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
