//! Writing made-up segmentations (DICOM SEG).

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::document::common_elements;
use super::{decimal, element, one, sequence, write_labelled};
use crate::testkit::{DocumentOptions, EXPLICIT_VR_LITTLE_ENDIAN};

const SEGMENTATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.4";

/// One segment a made-up segmentation describes.
#[derive(Debug, Clone)]
pub struct SegSegment {
    /// `SegmentNumber`.
    pub number: u16,
    /// `SegmentLabel`; not written when empty.
    pub label: String,
    /// `RecommendedDisplayCIELabValue`, as DICOM scales it to 16 bits; not written when absent.
    pub lab: Option<[u16; 3]>,
}

/// One frame of a made-up segmentation: a mask of one segment at one place.
#[derive(Debug, Clone)]
pub struct SegFrame {
    /// `ImagePositionPatient` of the frame.
    pub position: [f64; 3],
    /// The `ReferencedSegmentNumber` of the frame.
    pub segment: u16,
    /// One value a pixel, row by row: for BINARY masks any value but 0 is on; for FRACTIONAL
    /// masks the value is written as it is.
    pub mask: Vec<u8>,
}

/// Everything about a made-up segmentation. Start from [`SegOptions::default`], which is in the
/// same study as the default slice and has no frames.
#[derive(Debug, Clone)]
pub struct SegOptions {
    /// The study, series, patient and date.
    pub document: DocumentOptions,
    /// `Rows`.
    pub rows: u16,
    /// `Columns`.
    pub cols: u16,
    /// `ImageOrientationPatient`, shared by every frame.
    pub orientation: [f64; 6],
    /// `PixelSpacing` (between rows, between columns).
    pub pixel_spacing: [f64; 2],
    /// `SpacingBetweenSlices`, or absent.
    pub spacing_between: Option<f64>,
    /// The segments.
    pub segments: Vec<SegSegment>,
    /// The frames, in the order they are stored.
    pub frames: Vec<SegFrame>,
    /// For FRACTIONAL masks, the `MaximumFractionalValue`; `None` for BINARY masks.
    pub fractional: Option<u16>,
    /// `ContentLabel`; not written when empty.
    pub content_label: String,
    /// The `SeriesInstanceUID` in the `ReferencedSeriesSequence`, or absent.
    pub referenced_series: Option<String>,
    /// Transfer syntax named in the file header; the data are always explicit VR little endian.
    pub transfer_syntax: String,
    /// Leave this many bytes off the end of the masks, as a truncated file would.
    pub drop_bytes: usize,
}

impl Default for SegOptions {
    fn default() -> Self {
        Self {
            document: DocumentOptions {
                series_uid: "1.2.826.0.1.3680043.9.9999.1.950".to_owned(),
                title: String::new(),
                ..DocumentOptions::default()
            },
            rows: 3,
            cols: 5,
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            pixel_spacing: [0.9, 0.9],
            spacing_between: None,
            segments: Vec::new(),
            frames: Vec::new(),
            fractional: None,
            content_label: "MADE_UP".to_owned(),
            referenced_series: None,
            transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN.to_owned(),
            drop_bytes: 0,
        }
    }
}

/// One segmentation file, its BINARY masks packed bit after bit across the frames.
pub fn segmentation(opts: &SegOptions) -> Vec<u8> {
    let mut obj = common_elements(&opts.document, SEGMENTATION_STORAGE, "SEG");
    put_size(&mut obj, opts);
    let kind = if opts.fractional.is_some() {
        "FRACTIONAL"
    } else {
        "BINARY"
    };
    obj.put(element(tags::SEGMENTATION_TYPE, VR::CS, kind));
    if let Some(most) = opts.fractional {
        let most = PrimitiveValue::from(most);
        obj.put(element(tags::MAXIMUM_FRACTIONAL_VALUE, VR::US, most));
        let fractional = tags::SEGMENTATION_FRACTIONAL_TYPE;
        obj.put(element(fractional, VR::CS, "PROBABILITY"));
    }
    if !opts.content_label.is_empty() {
        obj.put(element(
            tags::CONTENT_LABEL,
            VR::CS,
            opts.content_label.as_str(),
        ));
    }
    if let Some(series) = &opts.referenced_series {
        let item = one(element(tags::SERIES_INSTANCE_UID, VR::UI, series.as_str()));
        obj.put(sequence(tags::REFERENCED_SERIES_SEQUENCE, vec![item]));
    }
    let segments = opts.segments.iter().map(segment_item).collect();
    obj.put(sequence(tags::SEGMENT_SEQUENCE, segments));
    obj.put(sequence(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        vec![shared(opts)],
    ));
    let per_frame = opts.frames.iter().map(frame_groups).collect();
    obj.put(sequence(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        per_frame,
    ));
    let mut masks = packed(opts);
    masks.truncate(masks.len().saturating_sub(opts.drop_bytes));
    if masks.len() % 2 == 1 {
        masks.push(0);
    }
    let masks = PrimitiveValue::U8(masks.into());
    obj.put(DataElement::new(tags::PIXEL_DATA, VR::OB, masks));
    write_labelled(obj, &opts.transfer_syntax)
}

/// The size of the frames and how many there are, and how their pixels are stored.
fn put_size(obj: &mut InMemDicomObject, opts: &SegOptions) {
    let bits: u16 = if opts.fractional.is_some() { 8 } else { 1 };
    for (tag, value) in [
        (tags::SAMPLES_PER_PIXEL, 1),
        (tags::ROWS, opts.rows),
        (tags::COLUMNS, opts.cols),
        (tags::BITS_ALLOCATED, bits),
        (tags::BITS_STORED, bits),
        (tags::HIGH_BIT, bits - 1),
        (tags::PIXEL_REPRESENTATION, 0),
    ] {
        obj.put(element(tag, VR::US, PrimitiveValue::from(value)));
    }
    let photometric = tags::PHOTOMETRIC_INTERPRETATION;
    obj.put(element(photometric, VR::CS, "MONOCHROME2"));
    let frames = opts.frames.len().to_string();
    obj.put(element(tags::NUMBER_OF_FRAMES, VR::IS, frames.as_str()));
}

/// The masks of every frame as stored: a byte a pixel for FRACTIONAL masks, and otherwise a
/// bit a pixel, the first in the lowest bit, running on from one frame into the next.
fn packed(opts: &SegOptions) -> Vec<u8> {
    let values = opts
        .frames
        .iter()
        .flat_map(|frame| frame.mask.iter().copied());
    if opts.fractional.is_some() {
        return values.collect();
    }
    let mut bytes = Vec::new();
    for (at, value) in values.enumerate() {
        if at % 8 == 0 {
            bytes.push(0);
        }
        if value != 0 {
            let last = bytes.last_mut().expect("a byte was pushed for this bit");
            *last |= 1 << (at % 8);
        }
    }
    bytes
}

fn segment_item(segment: &SegSegment) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    let number = PrimitiveValue::from(segment.number);
    item.put(element(tags::SEGMENT_NUMBER, VR::US, number));
    if !segment.label.is_empty() {
        item.put(element(tags::SEGMENT_LABEL, VR::LO, segment.label.as_str()));
    }
    item.put(element(tags::SEGMENT_ALGORITHM_TYPE, VR::CS, "MANUAL"));
    if let Some(lab) = segment.lab {
        let lab = PrimitiveValue::U16(lab.to_vec().into());
        item.put(element(
            tags::RECOMMENDED_DISPLAY_CIE_LAB_VALUE,
            VR::US,
            lab,
        ));
    }
    item
}

/// The functional groups every frame shares: which way they face and how far apart their
/// pixels are.
fn shared(opts: &SegOptions) -> InMemDicomObject {
    let mut shared = InMemDicomObject::new_empty();
    let facing = one(decimal(tags::IMAGE_ORIENTATION_PATIENT, &opts.orientation));
    shared.put(sequence(tags::PLANE_ORIENTATION_SEQUENCE, vec![facing]));
    let mut measures = InMemDicomObject::new_empty();
    measures.put(decimal(tags::PIXEL_SPACING, &opts.pixel_spacing));
    if let Some(between) = opts.spacing_between {
        measures.put(decimal(tags::SPACING_BETWEEN_SLICES, &[between]));
    }
    shared.put(sequence(tags::PIXEL_MEASURES_SEQUENCE, vec![measures]));
    shared
}

/// The functional groups of one frame: where it lies and which segment it shows.
fn frame_groups(frame: &SegFrame) -> InMemDicomObject {
    let mut groups = InMemDicomObject::new_empty();
    let position = one(decimal(tags::IMAGE_POSITION_PATIENT, &frame.position));
    groups.put(sequence(tags::PLANE_POSITION_SEQUENCE, vec![position]));
    let number = PrimitiveValue::from(frame.segment);
    let which = one(element(tags::REFERENCED_SEGMENT_NUMBER, VR::US, number));
    groups.put(sequence(tags::SEGMENT_IDENTIFICATION_SEQUENCE, vec![which]));
    groups
}
