//! The slices of the active viewport and how they are laid out: the keys that step through
//! them, play them, mirror, turn and fit the picture, and choose one, two or four viewports.

use egui::Key;

use super::super::{Command, Does, Place, Tab};
use super::{PLAIN, alone};
use crate::action::Action;
use crate::annotations::Tool;
use crate::scene::MainPanel;
use crate::state::{AppState, Layout};

/// How many slices Page Up and Page Down step.
const PAGE: i32 = 10;

/// Why the volume before or after cannot be shown: no scan of several volumes is shown, or
/// the one shown is already the first or the last of them.
fn volume_at_end(state: &AppState, last: bool) -> Option<&'static str> {
    let set = state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.as_ref())
        .and_then(|series| state.series(series))
        .and_then(|listed| listed.info.set.as_ref());
    let Some(set) = set else {
        return Some("The scan shown is not one of several volumes.");
    };
    if last && set.at + 1 >= set.count {
        Some("This is the last volume.")
    } else if !last && set.at == 0 {
        Some("This is the first volume.")
    } else {
        None
    }
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "view.flip_across",
        name: "Mirror the picture left to right",
        label: "Mirror across",
        words: &["flip", "mirror", "horizontal"],
        key: Some(alone(Key::H, "H")),
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| {
            let viewport = state.active;
            let horizontal = true;
            Some(Action::Flip {
                viewport,
                horizontal,
            })
        }),
        ..PLAIN
    },
    Command {
        id: "view.flip_down",
        name: "Mirror the picture top to bottom",
        label: "Mirror down",
        words: &["flip", "mirror", "vertical", "upside down"],
        key: Some(alone(Key::V, "V")),
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| {
            let viewport = state.active;
            let horizontal = false;
            Some(Action::Flip {
                viewport,
                horizontal,
            })
        }),
        ..PLAIN
    },
    Command {
        id: "view.turn",
        name: "Turn the picture a quarter turn clockwise",
        label: "Turn",
        words: &["rotate", "turn", "clockwise"],
        key: Some(alone(Key::R, "R")),
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| {
            let viewport = state.active;
            Some(Action::Rotate { viewport })
        }),
        ..PLAIN
    },
    Command {
        id: "view.fit",
        name: "Fit the whole picture in the viewport, and the whole volume in 3D",
        label: "Fit",
        words: &["fit", "zoom", "reset", "whole"],
        key: Some(alone(Key::F, "F")),
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| {
            let viewport = state.active;
            Some(Action::Fit { viewport })
        }),
        ..PLAIN
    },
    Command {
        id: "layout.one",
        name: "One viewport",
        label: "1",
        words: &["layout", "single", "one"],
        key: Some(alone(Key::Num1, "1")),
        place: Place::Strip,
        does: Does::Act(|_| Some(Action::SetLayout(Layout::One))),
        ..PLAIN
    },
    Command {
        id: "layout.two",
        name: "Two viewports side by side",
        label: "2",
        words: &["layout", "side by side", "compare", "two"],
        key: Some(alone(Key::Num2, "2")),
        place: Place::Strip,
        does: Does::Act(|_| Some(Action::SetLayout(Layout::TwoAcross))),
        ..PLAIN
    },
    Command {
        id: "layout.four",
        name: "Four viewports",
        label: "4",
        words: &["layout", "grid", "quad", "four"],
        key: Some(alone(Key::Num4, "4")),
        place: Place::Strip,
        does: Does::Act(|_| Some(Action::SetLayout(Layout::TwoByTwo))),
        ..PLAIN
    },
    Command {
        id: "slice.play",
        name: "Play or pause through the slices",
        label: "Play",
        words: &["cine", "play", "pause", "movie", "animate"],
        key: Some(alone(Key::Space, "Space")),
        place: Place::Tab(Tab::View),
        does: Does::Act(|state| {
            let viewport = state.active;
            Some(Action::CineToggle { viewport })
        }),
        ..PLAIN
    },
    Command {
        id: "slice.before",
        name: "The slice before",
        label: "Slice before",
        words: &["previous", "back", "up", "scroll"],
        key: Some(alone(Key::ArrowUp, "Up")),
        does: Does::Act(|state| Some(scroll(state, -1))),
        ..PLAIN
    },
    Command {
        id: "slice.after",
        name: "The slice after",
        label: "Slice after",
        words: &["next", "forward", "down", "scroll"],
        key: Some(alone(Key::ArrowDown, "Down")),
        does: Does::Act(|state| Some(scroll(state, 1))),
        ..PLAIN
    },
    Command {
        id: "slice.ten_back",
        name: "Ten slices back",
        label: "Ten back",
        words: &["page", "skip", "previous"],
        key: Some(alone(Key::PageUp, "Page Up")),
        does: Does::Act(|state| Some(scroll(state, -PAGE))),
        ..PLAIN
    },
    Command {
        id: "slice.ten_on",
        name: "Ten slices on",
        label: "Ten on",
        words: &["page", "skip", "next"],
        key: Some(alone(Key::PageDown, "Page Down")),
        does: Does::Act(|state| Some(scroll(state, PAGE))),
        ..PLAIN
    },
    Command {
        id: "slice.first",
        name: "The first slice",
        label: "First slice",
        words: &["start", "beginning", "top"],
        key: Some(alone(Key::Home, "Home")),
        does: Does::Act(|state| Some(go_to(state, 0))),
        ..PLAIN
    },
    Command {
        // Going to a slice stops at the last, so the last is asked for as the largest there is.
        id: "slice.last",
        name: "The last slice",
        label: "Last slice",
        words: &["end", "bottom", "final"],
        key: Some(alone(Key::End, "End")),
        does: Does::Act(|state| Some(go_to(state, usize::MAX))),
        ..PLAIN
    },
    Command {
        id: "volume.before",
        name: "The volume before",
        label: "◀",
        words: &["echo", "phase", "time point", "previous volume"],
        key: Some(alone(Key::OpenBracket, "[")),
        place: Place::Tab(Tab::View),
        why_not: |state| volume_at_end(state, false),
        does: Does::Act(|state| Some(step_volume(state, -1))),
        ..PLAIN
    },
    Command {
        id: "volume.after",
        name: "The volume after",
        label: "▶",
        words: &["echo", "phase", "time point", "next volume"],
        key: Some(alone(Key::CloseBracket, "]")),
        place: Place::Tab(Tab::View),
        why_not: |state| volume_at_end(state, true),
        does: Does::Act(|state| Some(step_volume(state, 1))),
        ..PLAIN
    },
    Command {
        id: "view.escape",
        name: "Put the tool down; with none in hand, give 3D the large part of the window",
        label: "Esc",
        words: &["escape", "cancel", "tool", "3D"],
        key: Some(alone(Key::Escape, "Esc")),
        does: Does::Act(|state| {
            Some(if state.tool == Tool::Navigate {
                Action::SetMain(MainPanel::Scene)
            } else {
                Action::SetTool(Tool::Navigate)
            })
        }),
        ..PLAIN
    },
];

/// Move `delta` slices on in the active viewport: later slices are on, as the wheel turned
/// towards the person moves.
fn scroll(state: &AppState, delta: i32) -> Action {
    let viewport = state.active;
    Action::Scroll { viewport, delta }
}

/// Go to a slice of the active viewport.
fn go_to(state: &AppState, slice: usize) -> Action {
    let viewport = state.active;
    Action::SetSlice { viewport, slice }
}

/// Show the volume `by` on from the one shown in the active viewport.
fn step_volume(state: &AppState, by: i32) -> Action {
    let viewport = state.active;
    Action::StepVolume { viewport, by }
}
