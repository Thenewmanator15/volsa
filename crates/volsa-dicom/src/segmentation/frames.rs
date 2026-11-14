//! What each frame of a segmentation says of itself: where it lies, which way it faces, and
//! which segment it is a mask of. Each is read from the frame's own functional groups or,
//! failing that, from those every frame shares.

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{SegmentationError, refused};
use crate::geometry::{is_image_plane, same_plane};
use crate::header::{first_item, floats, int, items, position};

/// What is said when a frame does not say where it lies or how far apart its pixels are.
pub(super) const NO_PLACE: &str =
    "This segmentation does not say where each of its frames lies in the patient.";
/// What is said when a frame does not say which segment it shows.
const NO_SEGMENT: &str = "This segmentation does not say which segment each of its frames shows.";
/// What is said when the frames do not lie on one grid.
pub(super) const NOT_ON_A_GRID: &str =
    "The frames of this segmentation do not lie on one grid, so Volsa does not open it as labels.";

/// Every frame of a segmentation, in the order stored, with what they share.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Frames {
    /// Direction of increasing column, then of increasing row: the same for every frame.
    pub orientation: [f64; 6],
    /// Distance between rows, then between columns.
    pub spacing: [f64; 2],
    /// The distance between slices the file states, if it does: `SpacingBetweenSlices`, or
    /// failing that `SliceThickness`.
    pub stated_step: Option<f64>,
    /// `ImagePositionPatient` of each frame.
    pub positions: Vec<[f64; 3]>,
    /// The segment number each frame is a mask of.
    pub segments: Vec<u16>,
}

/// The frames of a segmentation.
///
/// # Errors
///
/// When the file does not say, for every frame, where it lies, which way it faces, how far
/// apart its pixels are and which segment it shows, or when its frames face different ways.
pub(super) fn of(obj: &InMemDicomObject) -> Result<Frames, SegmentationError> {
    let per_frame = items(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    let stated = int(obj, tags::NUMBER_OF_FRAMES).and_then(|n| usize::try_from(n).ok());
    if per_frame.is_empty() || stated.is_some_and(|n| n != per_frame.len()) {
        return Err(refused(NO_PLACE));
    }
    let shared = first_item(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    let grouped = |frame, group| grouped(frame, shared, group);
    let measures = per_frame
        .first()
        .and_then(|frame| grouped(frame, tags::PIXEL_MEASURES_SEQUENCE))
        .ok_or_else(|| refused(NO_PLACE))?;
    let mut frames = Frames {
        orientation: [0.0; 6],
        spacing: spacing_of(measures).ok_or_else(|| refused(NO_PLACE))?,
        stated_step: [tags::SPACING_BETWEEN_SLICES, tags::SLICE_THICKNESS]
            .into_iter()
            .find_map(|tag| positive(measures, tag)),
        positions: Vec::with_capacity(per_frame.len()),
        segments: Vec::with_capacity(per_frame.len()),
    };
    for (at, frame) in per_frame.iter().enumerate() {
        let facing = grouped(frame, tags::PLANE_ORIENTATION_SEQUENCE)
            .and_then(|plane| floats(plane, tags::IMAGE_ORIENTATION_PATIENT))
            .and_then(|values| <[f64; 6]>::try_from(values).ok())
            .ok_or_else(|| refused(NO_PLACE))?;
        if at == 0 {
            frames.orientation = facing;
        }
        if !is_image_plane(facing) || !same_plane(facing, frames.orientation) {
            return Err(refused(NOT_ON_A_GRID));
        }
        let at = grouped(frame, tags::PLANE_POSITION_SEQUENCE)
            .and_then(position)
            .ok_or_else(|| refused(NO_PLACE))?;
        let segment = grouped(frame, tags::SEGMENT_IDENTIFICATION_SEQUENCE)
            .and_then(|which| int(which, tags::REFERENCED_SEGMENT_NUMBER))
            .and_then(|number| u16::try_from(number).ok())
            .filter(|number| *number > 0)
            .ok_or_else(|| refused(NO_SEGMENT))?;
        frames.positions.push(at);
        frames.segments.push(segment);
    }
    Ok(frames)
}

/// The first item of `group` in a frame's functional groups or, failing that, in those every
/// frame shares.
fn grouped<'a>(
    frame: &'a InMemDicomObject,
    shared: Option<&'a InMemDicomObject>,
    group: Tag,
) -> Option<&'a InMemDicomObject> {
    first_item(frame, group).or_else(|| shared.and_then(|shared| first_item(shared, group)))
}

/// `PixelSpacing` when it is two distances greater than nothing.
fn spacing_of(measures: &InMemDicomObject) -> Option<[f64; 2]> {
    let spacing = <[f64; 2]>::try_from(floats(measures, tags::PIXEL_SPACING)?).ok()?;
    spacing.iter().all(|gap| *gap > 0.0).then_some(spacing)
}

/// The first number of a value, when it is greater than nothing.
fn positive(obj: &InMemDicomObject, tag: Tag) -> Option<f64> {
    floats(obj, tag)?
        .first()
        .copied()
        .filter(|value| *value > 0.0)
}
