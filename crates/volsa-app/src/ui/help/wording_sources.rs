//! What the test of the wording reads: the states each action is asked its reason in, the
//! stages of a labelling run, and the sentences kept in constants.

use volsa_segment::Stage;

use super::super::{about, document, parts};
use crate::state::AppState;
use crate::testing::{labelled_head, showing_a_scan_of, thick_scan_ready};
use crate::ui::theme::DISCLAIMER;
use crate::{labels, measuring};

/// States to ask each action why it cannot be used in: nothing open, a scan with its 3D view
/// ready, labels open on it, and a CT scan whose values are not in Hounsfield units.
pub(super) fn states() -> Vec<AppState> {
    vec![
        AppState::default(),
        thick_scan_ready(),
        labelled_head().0,
        showing_a_scan_of("CT", Some("OTHER")),
    ]
}

/// The stages a labelling run is in.
pub(super) fn stages() -> [Stage; 6] {
    [
        Stage::Preparing,
        Stage::Finding { step: 1, of: 2 },
        Stage::Finding { step: 3, of: 3 },
        Stage::Cortex,
        Stage::Checking,
        Stage::Tidying,
    ]
}

/// The sentences the measuring, the labels, the reports and the windows keep in constants.
pub(super) fn other_sentences() -> Vec<&'static str> {
    vec![
        DISCLAIMER,
        about::INTENDED_USE,
        measuring::NO_SCALE,
        measuring::NO_ONE_SCALE,
        measuring::NOT_SHOWN,
        measuring::IN_COLOUR,
        measuring::PLAIN_STACK,
        labels::NOWHERE_NEAR,
        labels::NOTHING_TO_LABEL,
        labels::LABELS_WAIT,
        parts::WHAT_LABELS_ARE,
        parts::MADE_ELSEWHERE,
        parts::NO_VOLUME,
        document::HEADING,
        document::CUT_SHORT,
        document::IS_A_PDF,
        document::CANNOT_SHOW,
        document::LEFT_OUT,
        crate::ui::welcome::BEGIN,
        crate::ui::confirm::CLOSE_LABELS,
        crate::ui::confirm::CLOSE_MADE,
        crate::ui::confirm::DELETE_KEPT,
        crate::ui::scene::UPLOADING,
        crate::planes::SMOOTHING_NEEDS_MORE,
    ]
}
