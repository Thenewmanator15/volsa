//! The View tab: brightness, which way round the patient is drawn, playing through the slices
//! and stepping between volumes, mirroring, turning and fitting the picture, and the switches
//! for smoothing the planes, linking the views and showing the patient's details.

use egui::Ui;
use voxa_core::WindowLevel;

use super::super::commands::{self, Request};
use super::super::volumes;
use super::{offer, row, section};
use crate::action::Action;
use crate::state::{AppState, Cine};

/// What the Link switch says when pointed at.
pub const LINK_SAYS: &str = "Keep the slices of this study on the planes' position, and the position on \
                             the slice scrolled to. Linked views take it that the patient did not \
                             move between series.";

/// What the Smooth switch says when pointed at.
pub const SMOOTH_SAYS: &str = "Draw the three planes from a curve through the scan's values, not a blend \
                               of the nearest. Beside a sharp edge the curve overshoots a little, \
                               which shows as a faint rim that is not in the scan. Smoothed: a curve \
                               through the measured values, not more detail.";

/// The brightness presets offered, by their actions, in the order of the presets.
const PRESET_IDS: [&str; 3] = ["window.auto", "window.soft", "window.hard"];

/// The actions that mirror the picture across and down, turn it a quarter, and fit it whole.
const PICTURE_IDS: [&str; 4] = [
    "view.flip_across",
    "view.flip_down",
    "view.turn",
    "view.fit",
];

/// Draw the View tab: the brightness, the orientation, the slices, mirroring, turning and
/// fitting the picture, and the switches. Changes of state go onto `actions`; what its buttons
/// ask of the window is given back.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let mut asked = Vec::new();
    section(ui, "Brightness");
    brightness(ui, state, actions, &mut asked);
    section(ui, "Orientation");
    switch(
        ui,
        "view.radiological",
        state,
        state.radiological,
        actions,
        &mut asked,
    );
    section(ui, "Slices");
    slices(ui, state, actions, &mut asked);
    section(ui, "Picture");
    picture(ui, state, actions, &mut asked);
    section(ui, "Display");
    switch(
        ui,
        "planes.smooth",
        state,
        state.planes.smooth,
        actions,
        &mut asked,
    );
    switch(
        ui,
        "planes.link",
        state,
        state.planes.linked,
        actions,
        &mut asked,
    );
    switch(
        ui,
        "view.patient",
        state,
        state.show_patient,
        actions,
        &mut asked,
    );
    asked
}

/// A switch, in the plain words of its action's name, drawn pressed while it is `on`.
fn switch(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    on: bool,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    let Some(command) = commands::find(id) else {
        return;
    };
    let pressed = commands::button_labelled(ui, id, command.name, state, Some(on));
    if let Some(does) = pressed {
        asked.extend(does.run(state, actions));
    }
}

/// The window and level of the active viewport, as numbers that can be typed or dragged, and
/// the presets.
fn brightness(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    let viewport = state.active;
    let Some(shown) = state.viewports.get(viewport) else {
        return;
    };
    let series = shown.series.as_ref().and_then(|id| state.series(id));
    if series.is_some_and(|listed| listed.info.colour) {
        ui.weak("none: in its own colours");
        return;
    }
    let (mut width, mut level) = (shown.wl.width, shown.wl.center);
    let step = f64::from((shown.wl.width / 100.0).max(0.1));
    let width_field = egui::DragValue::new(&mut width)
        .speed(step)
        .prefix("W ")
        .range(1.0..=f32::MAX);
    let level_field = egui::DragValue::new(&mut level).speed(step).prefix("L ");
    let (width_changed, level_changed) = row(ui, |ui| {
        let width_changed = ui
            .add(width_field)
            .on_hover_text("Width: the span of values from black to white")
            .changed();
        let level_changed = ui
            .add(level_field)
            .on_hover_text("Level: the value shown as mid-grey")
            .changed();
        (width_changed, level_changed)
    });
    if width_changed || level_changed {
        actions.push(Action::SetWindow {
            viewport,
            wl: WindowLevel::new(level, width),
        });
    }
    row(ui, |ui| {
        for id in PRESET_IDS {
            offer(ui, id, state, None, actions, asked);
        }
    });
}

/// The buttons that mirror the picture of the active viewport, turn it, and fit it whole.
fn picture(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    row(ui, |ui| {
        for id in PICTURE_IDS {
            offer(ui, id, state, None, actions, asked);
        }
    });
}

/// Film-like playback through the slices of the active viewport, with its speed and whether
/// it starts again, and the buttons that step between the volumes of a scan of several.
fn slices(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    let viewport = state.active;
    let cine = state.viewports.get(viewport).and_then(|shown| shown.cine);
    let cine = cine.unwrap_or(Cine::new(Cine::DEFAULT_FPS, true, false));
    row(ui, |ui| {
        let label = if cine.playing { "Pause" } else { "Play" };
        if let Some(does) = commands::button_labelled(ui, "slice.play", label, state, None) {
            asked.extend(does.run(state, actions));
        }
        let (mut fps, mut looping) = (cine.fps, cine.looping);
        let speed = egui::DragValue::new(&mut fps)
            .speed(0.5)
            .range(Cine::MIN_FPS..=Cine::MAX_FPS)
            .suffix(" /s");
        let speed_changed = ui
            .add(speed)
            .on_hover_text("Slices shown each second")
            .changed();
        let loop_hint = "Start again from the first slice after the last";
        let loop_changed = ui
            .checkbox(&mut looping, "Loop")
            .on_hover_text(loop_hint)
            .changed();
        if speed_changed || loop_changed {
            actions.push(Action::CineSet {
                viewport,
                fps,
                looping,
            });
        }
    });
    // Nothing is drawn, not even a row, for a scan of one volume.
    asked.extend(volumes::stepper(ui, state, actions));
}

#[cfg(test)]
mod tests;
