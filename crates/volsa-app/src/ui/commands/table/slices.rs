//! The slices of the active viewport and how they are laid out: the keys that step through
//! them, play them, mirror, turn and fit the picture, choose one, two or four viewports, and
//! put a tool down.

use egui::Key;

use super::super::{Command, Does, Place, Tab, Why};
use super::{PLAIN, alone, covered_by_a_document, no_picture};
use crate::action::Action;
use crate::annotations::Tool;
use crate::state::{AppState, Cine, Layout};

/// How many slices Page Up and Page Down step.
const PAGE: i32 = 10;

/// What the hints of Up and Down say of the point: with a measuring tool in hand and the viewport
/// holding the keyboard, they move it and leave the slice alone.
const POINT_SAYS: &str = "With a measuring tool in hand and the keyboard on a viewport (Tab to it), \
     Up and Down move the point on the slice and leave the slice alone. Using the mouse there \
     gives them back to the slices. Page Up and Page Down still change the slice";

/// Why the volume before or after cannot be shown: no scan of several volumes is shown, or
/// the one shown is already the first or the last of them.
fn volume_at_end(state: &AppState, last: bool) -> Option<Why> {
    if let Some(why) = no_picture(state) {
        return Some(why);
    }
    let set = state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.as_ref())
        .and_then(|series| state.series(series))
        .and_then(|listed| listed.info.set.as_ref());
    let Some(set) = set else {
        return Some(Why::Borrowed(
            "The scan shown is not one of several volumes.",
        ));
    };
    if last && set.at + 1 >= set.count {
        Some(Why::Borrowed("This is the last volume."))
    } else if !last && set.at == 0 {
        Some(Why::Borrowed("This is the first volume."))
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
        why_not: no_picture,
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
        why_not: no_picture,
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
        why_not: no_picture,
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
        why_not: no_picture,
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
        why_not: covered_by_a_document,
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
        why_not: covered_by_a_document,
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
        why_not: covered_by_a_document,
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
        why_not: no_picture,
        does: Does::Act(|state| {
            let viewport = state.active;
            Some(Action::CineToggle { viewport })
        }),
        ..PLAIN
    },
    Command {
        id: "slice.loop",
        name: "Start again after the last slice",
        label: "Loop",
        explains: "While the slices play, go back to the first slice after the last",
        words: &["loop", "repeat", "cine", "play", "again", "wrap"],
        place: Place::Tab(Tab::View),
        why_not: no_picture,
        does: Does::Act(|state| {
            let viewport = state.active;
            let cine = state
                .viewports
                .get(viewport)
                .and_then(|shown| shown.cine)
                .unwrap_or_else(Cine::at_rest);
            Some(Action::CineSet {
                viewport,
                fps: cine.fps,
                looping: !cine.looping,
            })
        }),
        ..PLAIN
    },
    Command {
        id: "slice.before",
        name: "The slice before, or the point up with a tool in hand",
        label: "Slice before",
        explains: POINT_SAYS,
        words: &["previous", "back", "up", "scroll"],
        key: Some(alone(Key::ArrowUp, "Up")),
        why_not: no_picture,
        does: Does::Act(|state| Some(scroll(state, -1))),
        ..PLAIN
    },
    Command {
        id: "slice.after",
        name: "The slice after, or the point down with a tool in hand",
        label: "Slice after",
        explains: POINT_SAYS,
        words: &["next", "forward", "down", "scroll"],
        key: Some(alone(Key::ArrowDown, "Down")),
        why_not: no_picture,
        does: Does::Act(|state| Some(scroll(state, 1))),
        ..PLAIN
    },
    Command {
        id: "slice.ten_back",
        name: "Ten slices back",
        label: "Ten back",
        words: &["page", "skip", "previous"],
        key: Some(alone(Key::PageUp, "Page Up")),
        why_not: no_picture,
        does: Does::Act(|state| Some(scroll(state, -PAGE))),
        ..PLAIN
    },
    Command {
        id: "slice.ten_on",
        name: "Ten slices on",
        label: "Ten on",
        words: &["page", "skip", "next"],
        key: Some(alone(Key::PageDown, "Page Down")),
        why_not: no_picture,
        does: Does::Act(|state| Some(scroll(state, PAGE))),
        ..PLAIN
    },
    Command {
        id: "slice.first",
        name: "The first slice",
        label: "First slice",
        words: &["start", "beginning", "top"],
        key: Some(alone(Key::Home, "Home")),
        why_not: no_picture,
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
        why_not: no_picture,
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
        name: "Put the tool down",
        label: "Esc",
        words: &["escape", "cancel", "tool", "navigate"],
        key: Some(alone(Key::Escape, "Esc")),
        why_not: |state| {
            let none = state.tool == Tool::Navigate;
            none.then_some(Why::Borrowed("There is no tool in hand to put down."))
        },
        does: Does::Act(|state| {
            (state.tool != Tool::Navigate).then_some(Action::SetTool(Tool::Navigate))
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
