//! How the slices are shown: the brightness presets and the CT windows, which way round the
//! patient is drawn, whether the patient's details are shown, and whether the views are linked
//! and the planes smoothed.

use std::borrow::Cow;

use voxa_volume::PRESETS;

use super::super::{Command, Does, Place, Tab, Why};
use super::PLAIN;
use crate::action::Action;
use crate::profile::{CT_WINDOWS, Profile, Unit, unit_of};
use crate::state::AppState;
use crate::ui::tabs::{LINK_SAYS, SMOOTH_SAYS};

/// The words each brightness preset is found by.
const BRIGHTNESS: &[&str] = &["window", "level", "contrast", "preset", "brightness"];

/// What is said of a CT window while the scan's values are not in Hounsfield units: the
/// window is in them, so it would not fit.
const NOT_IN_HU: &str =
    "This scan's values are not in Hounsfield units, so a CT window does not fit them.";

/// Apply a brightness preset to the active viewport.
fn preset(state: &AppState, preset: usize) -> Action {
    let viewport = state.active;
    Action::ApplyPreset { viewport, preset }
}

/// Set the active viewport to one of the CT windows.
fn named(state: &AppState, window: usize) -> Option<Action> {
    let viewport = state.active;
    let wl = CT_WINDOWS.get(window)?.wl;
    Some(Action::SetWindow { viewport, wl })
}

/// Why a CT window cannot be used: the scan shown does not say its values are in Hounsfield
/// units, which the window is in. Such a scan can be a derived or secondary CT.
fn needs_hu(state: &AppState) -> Option<Why> {
    let in_hu = state
        .shown_info(state.active)
        .is_some_and(|info| unit_of(info) == Unit::Hu);
    (!in_hu).then_some(Cow::Borrowed(NOT_IN_HU))
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
        profiles: &[Profile::Mr],
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
        profiles: &[Profile::Mr],
        does: Does::Act(|state| Some(preset(state, 2))),
        ..PLAIN
    },
    Command {
        id: "window.brain",
        name: "Brain window",
        label: CT_WINDOWS[0].name,
        explains: "Width 80, level 40 HU",
        words: &["window", "level", "brightness", "ct", "hu", "brain"],
        place: Place::Tab(Tab::View),
        profiles: &[Profile::Ct],
        why_not: needs_hu,
        does: Does::Act(|state| named(state, 0)),
        ..PLAIN
    },
    Command {
        id: "window.soft_tissue",
        name: "Soft tissue window",
        label: CT_WINDOWS[1].name,
        explains: "Width 400, level 40 HU",
        words: &["window", "level", "brightness", "ct", "hu", "soft tissue"],
        place: Place::Tab(Tab::View),
        profiles: &[Profile::Ct],
        why_not: needs_hu,
        does: Does::Act(|state| named(state, 1)),
        ..PLAIN
    },
    Command {
        id: "window.lung",
        name: "Lung window",
        label: CT_WINDOWS[2].name,
        explains: "Width 1500, level -600 HU",
        words: &["window", "level", "brightness", "ct", "hu", "lung"],
        place: Place::Tab(Tab::View),
        profiles: &[Profile::Ct],
        why_not: needs_hu,
        does: Does::Act(|state| named(state, 2)),
        ..PLAIN
    },
    Command {
        id: "window.bone",
        name: "Bone window",
        label: CT_WINDOWS[3].name,
        explains: "Width 1800, level 400 HU",
        words: &["window", "level", "brightness", "ct", "hu", "bone"],
        place: Place::Tab(Tab::View),
        profiles: &[Profile::Ct],
        why_not: needs_hu,
        does: Does::Act(|state| named(state, 3)),
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
