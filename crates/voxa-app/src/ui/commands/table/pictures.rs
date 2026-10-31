//! How the pictures in the middle of the window are arranged: the four arrangements the strip
//! above them offers. Each but the slices alone shows the 3D view, which a scan may not have.

use super::super::{Command, Does, Place, Why};
use super::PLAIN;
use crate::action::Action;
use crate::pictures::Pictures;
use crate::scene::SceneStatus;
use crate::state::AppState;

/// Why an arrangement with the 3D view cannot be had: the scan shown has none, and the reason
/// it gives, as a sentence.
pub(in crate::ui) fn no_3d_view(state: &AppState) -> Option<Why> {
    let SceneStatus::Unavailable(said) = &state.scene.status else {
        return None;
    };
    let reason = said
        .iter()
        .map(|reason| reason.trim())
        .find(|reason| !reason.is_empty());
    let Some(reason) = reason else {
        return Some(Why::Borrowed("This scan has no 3D view."));
    };
    let stop = if reason.ends_with('.') { "" } else { "." };
    Some(Why::Owned(format!(
        "This scan has no 3D view: {}{stop}",
        lower_first(reason)
    )))
}

/// A sentence to follow a colon: its first letter lower case, unless the word it begins is
/// written in capitals, as `DICOM` is.
fn lower_first(sentence: &str) -> String {
    let mut letters = sentence.chars();
    match (letters.next(), letters.next()) {
        (Some(first), Some(second)) if !second.is_uppercase() => first
            .to_lowercase()
            .chain(sentence.chars().skip(1))
            .collect(),
        _ => sentence.to_owned(),
    }
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "pictures.slices",
        name: "Show the slices",
        label: "Slices",
        explains: "One, two or four viewports of the scan's own slices",
        words: &["arrangement", "2D", "stack", "viewports"],
        place: Place::Strip,
        does: Does::Act(|_| Some(Action::SetPictures(Pictures::Slices))),
        ..PLAIN
    },
    Command {
        id: "pictures.slices3d",
        name: "Show the slices beside the 3D view",
        label: "Slices + 3D",
        words: &["arrangement", "side by side", "both"],
        place: Place::Strip,
        why_not: no_3d_view,
        does: Does::Act(|_| Some(Action::SetPictures(Pictures::SlicesAndScene))),
        ..PLAIN
    },
    Command {
        id: "pictures.planes",
        name: "Show the three planes and the 3D view",
        label: "Planes",
        explains: "Axial, coronal and sagittal planes through one point, two by two with 3D",
        words: &[
            "arrangement",
            "MPR",
            "axial",
            "coronal",
            "sagittal",
            "orthogonal",
        ],
        place: Place::Strip,
        why_not: no_3d_view,
        does: Does::Act(|_| Some(Action::SetPictures(Pictures::Planes))),
        ..PLAIN
    },
    Command {
        id: "pictures.scene",
        name: "Show the 3D view alone",
        label: "3D",
        words: &["arrangement", "3D", "volume", "whole"],
        place: Place::Strip,
        why_not: no_3d_view,
        does: Does::Act(|_| Some(Action::SetPictures(Pictures::Scene))),
        ..PLAIN
    },
];
