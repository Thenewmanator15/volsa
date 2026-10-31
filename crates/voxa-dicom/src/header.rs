//! Reading what the indexer needs from one file's header, without touching its pixels.

use std::cell::Cell;
use std::io::Cursor;
use std::panic::AssertUnwindSafe;

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::{DefaultDicomObject, InMemDicomObject, OpenFileOptions};
use voxa_core::ScanFacts;

use crate::nesting::{TOO_DEEP, TooDeep, check_nesting};

mod head;
mod scan;
mod told;

pub(crate) use head::read_header_from;
pub(crate) use told::Told;

/// What one image file says about itself.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Header {
    pub study_uid: String,
    pub study_date: Option<String>,
    pub study_description: String,
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub series_uid: String,
    pub series_number: i32,
    pub series_description: String,
    pub modality: String,
    pub sop_uid: String,
    pub instance_number: i32,
    pub rows: u32,
    pub cols: u32,
    /// Direction of increasing column, then of increasing row.
    pub orientation: Option<[f64; 6]>,
    /// Distance between rows, then between columns.
    pub pixel_spacing: Option<[f64; 2]>,
    pub slice_thickness: Option<f64>,
    /// True for a file that holds many frames: a whole series in one file.
    pub multi_frame: bool,
    /// The transfer syntax named in the file header: how the pixels are stored.
    pub transfer_syntax: String,
    /// `ImagePositionPatient` of each frame; one entry for a single-frame file.
    pub positions: Vec<Option<[f64; 3]>>,
    /// True when the images are stored in colour: three samples for every pixel.
    pub colour: bool,
    /// What the file says about how the series was acquired.
    pub scan: ScanFacts,
    /// What each frame says that sets it apart from others at its position; one entry
    /// for a single-frame file.
    pub told: Vec<Told>,
}

/// What the start of a file was found to hold.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Start {
    /// The header of an image.
    Image(Box<Header>),
    /// Not a DICOM image: not DICOM at all, or DICOM without the identifiers and size every
    /// image has.
    NotAnImage,
    /// A file that is not read, for the reason given: one nested too deeply to read safely.
    Refused(&'static str),
}

#[cfg(test)]
impl Start {
    /// The header, when the start was an image's.
    pub(crate) fn image(self) -> Option<Header> {
        match self {
            Self::Image(header) => Some(*header),
            Self::NotAnImage | Self::Refused(_) => None,
        }
    }
}

/// Parse the header of one whole file, as [`read_header_start`] does; `None` when it is not
/// read as an image.
#[cfg(test)]
pub(crate) fn read_header(bytes: &[u8]) -> Option<Header> {
    read_header_start(bytes, true).ok().and_then(Start::image)
}

/// The start of a file was too short to be sure of its header: more of the file is needed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NeedMore;

/// Parse the header at the start of a file; `whole` says whether `start` is all of it.
///
/// A file the DICOM library panics on is [`Start::NotAnImage`], and is skipped like any other
/// file that cannot be read; a file nested too deeply is [`Start::Refused`] before the library
/// is given it. What is given is what the whole file gives; when the parse needed more than a
/// start that is not the whole file holds, [`NeedMore`] is given instead.
pub(crate) fn read_header_start(start: &[u8], whole: bool) -> Result<Start, NeedMore> {
    let short = Cell::new(false);
    // Only `short` outlives a panic, and it holds a plain flag.
    let parsed = std::panic::catch_unwind(AssertUnwindSafe(|| parse(start, whole, &short)));
    if short.get() {
        return Err(NeedMore);
    }
    Ok(parsed.unwrap_or(Start::NotAnImage))
}

/// The file at the start of `bytes`, read up to its pixels: all of the file when `whole`, and
/// otherwise its start, with `short` set when that was too short to be sure. `None` when the
/// library cannot read it; refused, and not given to the library, when it is nested too deeply.
fn up_to_the_pixels(
    bytes: &[u8],
    whole: bool,
    short: &Cell<bool>,
) -> Result<Option<DefaultDicomObject>, TooDeep> {
    check_nesting(bytes, Some(tags::PIXEL_DATA))?;
    let options = OpenFileOptions::new().read_until(tags::PIXEL_DATA);
    let file = if whole {
        options.from_reader(Cursor::new(bytes))
    } else {
        options.from_reader(head::Watched::new(bytes, short))
    };
    Ok(file.ok())
}

/// What the start of a file holds, read up to its pixels.
fn parse(bytes: &[u8], whole: bool, short: &Cell<bool>) -> Start {
    match up_to_the_pixels(bytes, whole, short) {
        Err(TooDeep) => Start::Refused(TOO_DEEP),
        Ok(file) => file
            .and_then(|file| header(&file, bytes, whole, short))
            .map_or(Start::NotAnImage, |header| Start::Image(Box::new(header))),
    }
}

/// The header of a file read up to its pixels; `None` when it is not an image.
fn header(
    file: &DefaultDicomObject,
    bytes: &[u8],
    whole: bool,
    short: &Cell<bool>,
) -> Option<Header> {
    let obj: &InMemDicomObject = file;
    let groups = items(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE).len();
    let stated = int(obj, tags::NUMBER_OF_FRAMES)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n > 0);
    let multi_frame = stated.is_some_and(|n| n > 1) || groups > 0;
    // A file cannot hold more frames than it has bytes, whatever its header claims; of a file
    // read in part, that is known only once that many bytes are in.
    let wanted = stated.unwrap_or(groups.max(1));
    if !whole && wanted > bytes.len() {
        short.set(true);
        return None;
    }
    let frames = wanted.min(bytes.len());
    let orientation = of_the_plane(obj, tags::PLANE_ORIENTATION_SEQUENCE, IMAGE_ORIENTATION)
        .and_then(|v| <[f64; 6]>::try_from(v).ok());
    Some(Header {
        study_uid: text(obj, tags::STUDY_INSTANCE_UID)?,
        study_date: text(obj, tags::STUDY_DATE),
        study_description: text(obj, tags::STUDY_DESCRIPTION).unwrap_or_default(),
        patient_name: text(obj, tags::PATIENT_NAME),
        patient_id: text(obj, tags::PATIENT_ID),
        series_uid: text(obj, tags::SERIES_INSTANCE_UID)?,
        series_number: int(obj, tags::SERIES_NUMBER).unwrap_or(0),
        series_description: text(obj, tags::SERIES_DESCRIPTION).unwrap_or_default(),
        modality: text(obj, tags::MODALITY).unwrap_or_default(),
        sop_uid: text(obj, tags::SOP_INSTANCE_UID).unwrap_or_default(),
        instance_number: int(obj, tags::INSTANCE_NUMBER).unwrap_or(0),
        rows: u32::try_from(int(obj, tags::ROWS)?).ok()?,
        cols: u32::try_from(int(obj, tags::COLUMNS)?).ok()?,
        orientation,
        pixel_spacing: of_the_plane(obj, tags::PIXEL_MEASURES_SEQUENCE, tags::PIXEL_SPACING)
            .and_then(|v| <[f64; 2]>::try_from(v).ok()),
        slice_thickness: of_the_plane(obj, tags::PIXEL_MEASURES_SEQUENCE, tags::SLICE_THICKNESS)
            .and_then(|v| v.first().copied()),
        multi_frame,
        transfer_syntax: file.meta().transfer_syntax().to_owned(),
        positions: if multi_frame {
            frame_positions(obj, frames)
        } else {
            vec![position(obj)]
        },
        colour: int(obj, tags::SAMPLES_PER_PIXEL) == Some(3),
        scan: scan::facts(obj, orientation),
        told: if multi_frame {
            told::of_frames(obj, frames)
        } else {
            vec![told::of_file(obj)]
        },
    })
}

const IMAGE_ORIENTATION: Tag = tags::IMAGE_ORIENTATION_PATIENT;

/// A value describing the image plane. A single-frame file states it directly; a file of many
/// frames keeps it in a functional group, shared by every frame or repeated for each, in which
/// case the first frame's is taken.
fn of_the_plane(obj: &InMemDicomObject, group: Tag, wanted: Tag) -> Option<Vec<f64>> {
    let shared = first_item(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    let first_frame = first_item(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    floats(obj, wanted).or_else(|| {
        [shared, first_frame]
            .into_iter()
            .flatten()
            .find_map(|groups| first_item(groups, group).and_then(|item| floats(item, wanted)))
    })
}

/// `ImagePositionPatient` of each of `frames` frames, from the per-frame functional groups.
fn frame_positions(obj: &InMemDicomObject, frames: usize) -> Vec<Option<[f64; 3]>> {
    let per_frame = items(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    (0..frames)
        .map(|frame| {
            per_frame
                .get(frame)
                .and_then(|groups| first_item(groups, tags::PLANE_POSITION_SEQUENCE))
                .and_then(position)
        })
        .collect()
}

/// A text value with padding removed; `None` when absent or empty.
fn text(obj: &InMemDicomObject, tag: Tag) -> Option<String> {
    let value = obj.get(tag)?.to_str().ok()?;
    let trimmed = value.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn int(obj: &InMemDicomObject, tag: Tag) -> Option<i32> {
    obj.get(tag)?.to_int::<i32>().ok()
}

/// Every number in a value; `None` when absent or when any part is not a finite number.
fn floats(obj: &InMemDicomObject, tag: Tag) -> Option<Vec<f64>> {
    let values = obj.get(tag)?.to_multi_float64().ok()?;
    values.iter().all(|v| v.is_finite()).then_some(values)
}

fn position(obj: &InMemDicomObject) -> Option<[f64; 3]> {
    floats(obj, tags::IMAGE_POSITION_PATIENT).and_then(|v| <[f64; 3]>::try_from(v).ok())
}

/// The items of a sequence; empty when the element is absent or not a sequence.
fn items(obj: &InMemDicomObject, tag: Tag) -> &[InMemDicomObject] {
    obj.get(tag)
        .and_then(|element| element.items())
        .unwrap_or(&[])
}

fn first_item(obj: &InMemDicomObject, tag: Tag) -> Option<&InMemDicomObject> {
    items(obj, tag).first()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{
        EnhancedFrame, EnhancedOptions, SliceOptions, classic_slice, enhanced_series,
    };
    use dicom_core::{DataElement, PrimitiveValue, VR};

    #[test]
    fn a_classic_header_is_read_without_its_pixels() {
        let header = read_header(&classic_slice(&SliceOptions::default())).expect("is an image");
        assert_eq!(header.series_uid, "1.2.826.0.1.3680043.9.9999.1.1");
        assert_eq!((header.rows, header.cols), (4, 4));
        assert_eq!(header.orientation, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
        assert_eq!(header.pixel_spacing, Some([0.9, 0.9]));
        assert_eq!(header.slice_thickness, Some(5.0));
        assert_eq!(header.positions, vec![Some([-100.0, -120.0, 0.0])]);
        assert!(!header.multi_frame);
    }

    #[test]
    fn an_enhanced_header_gives_a_position_for_every_frame() {
        let frames = (0..3)
            .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 2.5]))
            .collect();
        let bytes = enhanced_series(&EnhancedOptions {
            common: SliceOptions::default(),
            frames,
        });
        let header = read_header(&bytes).expect("is an image");
        assert!(header.multi_frame);
        assert_eq!(
            header.positions,
            vec![
                Some([0.0, 0.0, 0.0]),
                Some([0.0, 0.0, 2.5]),
                Some([0.0, 0.0, 5.0])
            ]
        );
        assert_eq!(header.orientation, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
        assert_eq!(header.pixel_spacing, Some([0.9, 0.9]));
        assert_eq!(header.slice_thickness, Some(5.0));
    }

    #[test]
    fn bytes_that_are_not_an_image_give_nothing() {
        assert_eq!(read_header(b""), None);
        assert_eq!(read_header(b"%PDF-1.7 a report"), None);
        let mut magic_then_noise = vec![0u8; 128];
        magic_then_noise.extend_from_slice(b"DICM\xff\xff\xff\xff\xff\xff\xff\xff");
        assert_eq!(read_header(&magic_then_noise), None);
    }

    #[test]
    fn missing_geometry_is_absent_not_an_error() {
        let opts = SliceOptions {
            position: None,
            orientation: None,
            pixel_spacing: None,
            ..SliceOptions::default()
        };
        let header = read_header(&classic_slice(&opts)).expect("is an image");
        assert_eq!(header.positions, vec![None]);
        assert_eq!(header.orientation, None);
        assert_eq!(header.pixel_spacing, None);
    }

    #[test]
    fn a_header_cannot_claim_more_frames_than_the_file_has_bytes() {
        let honest = classic_slice(&SliceOptions::default());
        let mut file = dicom_object::from_reader(Cursor::new(honest)).expect("parses");
        file.put(DataElement::new(
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            PrimitiveValue::from("100000"),
        ));
        let mut bytes = Vec::new();
        file.write_all(&mut bytes).expect("writes to memory");
        let header = read_header(&bytes).expect("is an image");
        assert!(header.multi_frame);
        assert!(
            header.positions.len() <= bytes.len(),
            "{} frames claimed by a file of {} bytes",
            header.positions.len(),
            bytes.len()
        );
    }
}
