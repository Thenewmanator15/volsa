//! What the menu of a slice offers besides what the View tab has: putting the picture back as it
//! was first shown, and saving the slice as a picture.

use super::super::{Command, Does, Place, Request};
use super::{PLAIN, no_picture};
use crate::action::Action;

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "view.reset",
        name: "Put the picture back as it was first shown",
        label: "Reset view",
        explains: "Fitted, not mirrored, not turned, and with Auto brightness",
        words: &["reset view", "restore", "original view", "as first shown"],
        place: Place::Slice,
        why_not: no_picture,
        does: Does::Act(|state| Some(Action::ResetView(state.active))),
        ..PLAIN
    },
    Command {
        id: "view.save_slice",
        name: "Export this slice as a PNG picture",
        label: "Save this slice as a picture…",
        explains: "As it is shown, as a PNG file of its pixels alone: no words, no details",
        words: &["png", "screenshot", "export slice", "slice picture"],
        place: Place::Slice,
        why_not: no_picture,
        does: Does::Ask(Request::SaveSlice),
        ..PLAIN
    },
];
