//! The tools of the Tools tab that are part of Voxa: so far, labelling a brain scan.

use super::super::{Command, Does, Place, Request, Tab, Why};
use super::PLAIN;
use crate::labelling::can_label;
use crate::ui::tabs::{LABEL_BUTTON, LABEL_TOOL};

/// Why the scan cannot be labelled now, in a sentence; the Tools tab says why in full.
const CANNOT_LABEL: &str = "Voxa cannot label this scan now. The Tools tab says why.";

/// The entries.
pub(super) const COMMANDS: &[Command] = &[Command {
    id: "tools.label",
    name: LABEL_TOOL,
    label: LABEL_BUTTON,
    explains: "Outline about thirty parts of the brain on this scan, on this computer",
    words: &["segment", "parts", "synthseg", "labelling"],
    place: Place::Tab(Tab::Tools),
    why_not: |state| {
        can_label(state)
            .is_err()
            .then_some(Why::Borrowed(CANNOT_LABEL))
    },
    // The window says how much of the graphics card it holds itself, which the run is told.
    does: Does::Ask(Request::LabelScan),
    ..PLAIN
}];
