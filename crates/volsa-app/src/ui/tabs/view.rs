//! The View tab: brightness, which way round the patient is drawn, playing through the slices
//! and stepping between volumes, mirroring, turning and fitting the picture, and the switches
//! for smoothing the planes, linking the views and showing the patient's details; and last a
//! few plain sentences about the kind of scan in the active viewport.

use egui::Ui;
use volsa_core::WindowLevel;

use super::super::access;
use super::super::commands::{self, Request};
use super::super::overlay::window_number;
use super::super::volumes;
use super::{offer, row, section};
use crate::action::Action;
use crate::naming::{ABOUT_HEADING, about};
use crate::profile::{Unit, unit_of};
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

/// The brightness settings, by their actions, in the order they are shown. Which of them are
/// offered depends on the scan: the four CT windows and Auto for CT, Auto, Soft and Hard for
/// MR, and Auto alone for any other.
const WINDOW_IDS: [&str; 7] = [
    "window.brain",
    "window.soft_tissue",
    "window.lung",
    "window.bone",
    "window.auto",
    "window.soft",
    "window.hard",
];

/// What is said in place of the brightness settings for a picture stored in colour.
const IN_COLOUR: &str = "In its own colours: no brightness to set.";

/// Every sentence the tab keeps in a constant, for the check that none suggests a finding.
#[cfg(test)]
pub(super) const TEXTS: [&str; 1] = [IN_COLOUR];

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
    about_the_scan(ui, state);
    asked
}

/// A few plain sentences about the kind of scan in the active viewport, each on lines of its
/// own that wrap within the panel. Nothing is drawn, not even the heading, with no scan shown.
fn about_the_scan(ui: &mut Ui, state: &AppState) {
    let Some(info) = state.shown_info(state.active) else {
        return;
    };
    section(ui, ABOUT_HEADING);
    for line in about(info) {
        ui.add(egui::Label::new(line).wrap());
    }
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

/// A number of the window as a number to write.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a width or a level of a window is far inside what an f32 holds"
)]
fn narrowed(number: f64) -> f32 {
    number as f32
}

/// Add a number that can be typed or dragged, called `name` for a screen reader and saying
/// `hint` when the pointer rests on it; whether it was changed.
fn field(ui: &mut Ui, number: egui::DragValue<'_>, name: &str, hint: &str) -> bool {
    let response = ui.add(number).on_hover_text(hint);
    access::call(&response, name);
    response.changed()
}

/// The window and level of the active viewport, as numbers that can be typed or dragged, in
/// Hounsfield units where the scan is in them, and the settings its profile offers.
fn brightness(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    // No numbers to set where no picture is in sight to set them on: say so, once.
    let settings = match commands::no_picture(state) {
        Some(why) => {
            ui.weak(why);
            true
        }
        None => window_and_level(ui, state, actions),
    };
    if settings {
        row(ui, |ui| {
            for id in WINDOW_IDS {
                offer(ui, id, state, None, actions, asked);
            }
        });
    }
}

/// The window and level of the active viewport, as numbers that can be typed or dragged, in
/// Hounsfield units where the scan is in them. Whether the settings it offers are to be offered
/// too: not for a picture stored in colour.
fn window_and_level(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> bool {
    let viewport = state.active;
    let Some(shown) = state.viewports.get(viewport) else {
        return false;
    };
    let info = state.shown_info(viewport);
    if info.is_some_and(|info| info.colour) {
        ui.weak(IN_COLOUR);
        return false;
    }
    let unit = if info.is_some_and(|info| unit_of(info) == Unit::Hu) {
        " HU"
    } else {
        ""
    };
    let (mut width, mut level) = (shown.wl.width, shown.wl.center);
    let step = f64::from((shown.wl.width / 100.0).max(0.1));
    // Written as the corner of the picture writes it, not to two decimals whatever the size.
    let written = |number: f64, _: std::ops::RangeInclusive<usize>| window_number(narrowed(number));
    let width_field = egui::DragValue::new(&mut width)
        .speed(step)
        .prefix("W ")
        .suffix(unit)
        .custom_formatter(written)
        .range(1.0..=f32::MAX);
    let level_field = egui::DragValue::new(&mut level)
        .speed(step)
        .prefix("L ")
        .suffix(unit)
        .custom_formatter(written);
    let (width_changed, level_changed) = row(ui, |ui| {
        let width_hint = "Width: the span of values from black to white";
        let width_changed = field(ui, width_field, "Window width", width_hint);
        let level_hint = "Level: the value shown as mid-grey";
        let level_changed = field(ui, level_field, "Window level", level_hint);
        (width_changed, level_changed)
    });
    let slid = sliders::window(ui, state, shown.wl);
    if width_changed || level_changed {
        actions.push(Action::SetWindow {
            viewport,
            wl: WindowLevel::new(level, width),
        });
    } else if let Some(wl) = slid {
        actions.push(Action::SetWindow { viewport, wl });
    }
    true
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
    let cine = cine.unwrap_or_else(Cine::at_rest);
    let mut fps = cine.fps;
    let typed = row(ui, |ui| {
        let label = if cine.playing { "Pause" } else { "Play" };
        if let Some(does) = commands::button_labelled(ui, "slice.play", label, state, None) {
            asked.extend(does.run(state, actions));
        }
        let speed = egui::DragValue::new(&mut fps)
            .speed(0.5)
            .range(Cine::MIN_FPS..=Cine::MAX_FPS)
            .suffix(" /s");
        field(ui, speed, "Playback speed", "Slices shown each second")
    });
    if let Some(slice) = sliders::slice(ui, state) {
        actions.push(Action::SetSlice { viewport, slice });
    }
    // On a line of its own: beside the button and the number it would not fit a narrow panel.
    let slid = sliders::speed(ui, &mut fps);
    if typed || slid {
        let looping = cine.looping;
        actions.push(Action::CineSet {
            viewport,
            fps,
            looping,
        });
    }
    switch(ui, "slice.loop", state, cine.looping, actions, asked);
    // Nothing is drawn, not even a row, for a scan of one volume.
    asked.extend(volumes::stepper(ui, state, actions));
}

mod sliders;

#[cfg(test)]
mod about_tests;
#[cfg(test)]
mod looping_tests;
#[cfg(test)]
mod tests;
