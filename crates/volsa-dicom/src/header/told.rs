//! What sets one image apart from the others at its position.

use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::scan::number;
use super::{first_item, int, items};

/// What an image says that can tell it from another image at the same position: which time
/// point it is, which diffusion weighting, which echo. Every part may be absent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Told {
    /// Which time point, as the scanner numbers them.
    pub temporal: Option<i32>,
    /// The diffusion weighting.
    pub b_value: Option<f64>,
    /// The echo time, in milliseconds.
    pub echo_ms: Option<f64>,
    /// The delay after the trigger, in milliseconds.
    pub trigger_ms: Option<f64>,
    /// Which acquisition, as the scanner numbers them.
    pub acquisition: Option<i32>,
}

/// What a single-frame file says of its one image.
pub(super) fn of_file(obj: &InMemDicomObject) -> Told {
    Told {
        temporal: int(obj, tags::TEMPORAL_POSITION_IDENTIFIER),
        b_value: number(obj, tags::DIFFUSION_B_VALUE),
        echo_ms: number(obj, tags::ECHO_TIME),
        trigger_ms: number(obj, tags::TRIGGER_TIME),
        acquisition: int(obj, tags::ACQUISITION_NUMBER),
    }
}

/// What each of `frames` frames of a file of many frames says, from its own functional groups.
pub(super) fn of_frames(obj: &InMemDicomObject, frames: usize) -> Vec<Told> {
    let per_frame = items(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    (0..frames)
        .map(|frame| per_frame.get(frame).map(of_frame).unwrap_or_default())
        .collect()
}

fn of_frame(groups: &InMemDicomObject) -> Told {
    let content = first_item(groups, tags::FRAME_CONTENT_SEQUENCE);
    let diffusion = first_item(groups, tags::MR_DIFFUSION_SEQUENCE);
    let echo = first_item(groups, tags::MR_ECHO_SEQUENCE);
    Told {
        temporal: content.and_then(|item| int(item, tags::TEMPORAL_POSITION_INDEX)),
        b_value: diffusion.and_then(|item| number(item, tags::DIFFUSION_B_VALUE)),
        echo_ms: echo.and_then(|item| number(item, tags::EFFECTIVE_ECHO_TIME)),
        ..Told::default()
    }
}
