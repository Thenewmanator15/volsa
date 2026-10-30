//! How the slices are shown: the brightness presets, which way round the patient is drawn,
//! whether the patient's details are shown, and whether the views are linked and the planes
//! smoothed.

use voxa_volume::PRESETS;

use super::super::{Command, Does, Place, Tab};
use super::PLAIN;
use crate::action::Action;
use crate::state::AppState;
use crate::ui::toolbar::{LINK_SAYS, SMOOTH_SAYS};

/// The words each brightness preset is found by.
const BRIGHTNESS: &[&str] = &["window", "level", "contrast", "preset", "brightness"];

/// Apply a brightness preset to the active viewport.
fn preset(state: &AppState, preset: usize) -> Action {
    let viewport = state.active;
    Action::ApplyPreset { viewport, preset }
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "window.auto",
        name: "Auto brightness",
        label: PRESETS[0].name,
        explains: "From the 1 to the 99.5 percentile of the slice shown",
        words: BRIGHTNESS,
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| Some(preset(state, 0))),
        ..PLAIN
    },
    Command {
        id: "window.soft",
        name: "Soft brightness",
        label: PRESETS[1].name,
        explains: "From the 0.5 to the 99.9 percentile of the slice shown",
        words: BRIGHTNESS,
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| Some(preset(state, 1))),
        ..PLAIN
    },
    Command {
        id: "window.hard",
        name: "Hard brightness",
        label: PRESETS[2].name,
        explains: "From the 5 to the 98 percentile of the slice shown",
        words: BRIGHTNESS,
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| Some(preset(state, 2))),
        ..PLAIN
    },
    Command {
        id: "view.radiological",
        name: "Patient's left on the right",
        label: "Radiological",
        explains: "On: the patient's left is on the right of the screen, as a radiologist reads \
                   it.\nOff: neurological, the patient's left on the left.",
        words: &[
            "radiological",
            "neurological",
            "convention",
            "orientation",
            "left",
            "right",
        ],
        place: Place::Tab(Tab::View),
        does: Does::Act(|_| Some(Action::ToggleRadiological)),
        ..PLAIN
    },
    Command {
        id: "view.patient",
        name: "Show patient details",
        label: "Patient details",
        explains: "Show the patient's name and identifier in the viewports and the tag browser",
        words: &["patient", "name", "identifier", "privacy", "anonymous"],
        place: Place::Tab(Tab::View),
        does: Does::Act(|_| Some(Action::TogglePatient)),
        ..PLAIN
    },
    Command {
        id: "planes.link",
        name: "Link the views",
        label: "Link",
        explains: LINK_SAYS,
        words: &["link", "sync", "synchronise", "follow", "position"],
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| Some(Action::SetLinked(!state.planes.linked))),
        ..PLAIN
    },
    Command {
        id: "planes.smooth",
        name: "Smooth the planes",
        label: "Smooth",
        explains: SMOOTH_SAYS,
        words: &["smooth", "interpolate", "cubic", "curve"],
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| Some(Action::SetSmooth(!state.planes.smooth))),
        ..PLAIN
    },
];
