//! Writing the synthetic files.

use dicom_core::value::DataSetSequence;
use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::tags;
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    Colour, EXPLICIT_VR_LITTLE_ENDIAN, EnhancedFrame, EnhancedOptions, ScanTags, SliceOptions,
};

mod colour;
mod scan;

const MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
const ENHANCED_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4.1";
const IMPLEMENTATION_CLASS: &str = "1.2.826.0.1.3680043.9.9999.0";

type Element = DataElement<InMemDicomObject>;

/// One valid single-frame MR file.
pub fn classic_slice(opts: &SliceOptions) -> Vec<u8> {
    let mut obj = common_elements(opts, MR_IMAGE_STORAGE);
    if let Some(position) = opts.position {
        obj.put(decimal(tags::IMAGE_POSITION_PATIENT, &position));
    }
    if let Some(orientation) = opts.orientation {
        obj.put(decimal(tags::IMAGE_ORIENTATION_PATIENT, &orientation));
    }
    if let Some(spacing) = opts.pixel_spacing {
        obj.put(decimal(tags::PIXEL_SPACING, &spacing));
    }
    if let Some(thickness) = opts.slice_thickness {
        obj.put(decimal(tags::SLICE_THICKNESS, &[thickness]));
    }
    obj.put(decimal(tags::RESCALE_INTERCEPT, &[opts.intercept]));
    obj.put(decimal(tags::RESCALE_SLOPE, &[opts.slope]));
    let count = usize::from(opts.rows) * usize::from(opts.cols);
    let mut stored = if opts.stored.is_empty() {
        ramp(count)
    } else {
        opts.stored.clone()
    };
    stored.truncate(stored.len().saturating_sub(opts.drop_pixels));
    obj.put(pixels(stored));
    if let Some(colour) = &opts.colour {
        colour::put(&mut obj, colour, opts.drop_pixels);
    }
    write(obj, opts)
}

/// One valid multi-frame MR file holding a whole series.
pub fn enhanced_series(opts: &EnhancedOptions) -> Vec<u8> {
    let common = &opts.common;
    let mut obj = common_elements(common, ENHANCED_MR_IMAGE_STORAGE);
    let frame_count = opts.frames.len().to_string();
    obj.put(element(
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        frame_count.as_str(),
    ));
    let shared = vec![shared_groups(common)];
    obj.put(sequence(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, shared));
    let per_frame = opts.frames.iter().map(frame_groups).collect();
    obj.put(sequence(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        per_frame,
    ));

    let count = usize::from(common.rows) * usize::from(common.cols);
    let mut stored = Vec::with_capacity(count * opts.frames.len());
    for (number, frame) in opts.frames.iter().enumerate() {
        if frame.stored.is_empty() {
            let fill = u16::try_from(number).expect("fewer than 65536 frames");
            stored.extend(std::iter::repeat_n(fill, count));
        } else {
            stored.extend_from_slice(&frame.stored);
        }
    }
    obj.put(pixels(stored));
    write(obj, common)
}

/// The functional groups every frame shares: orientation, pixel spacing and thickness.
fn shared_groups(common: &SliceOptions) -> InMemDicomObject {
    let mut shared = InMemDicomObject::new_empty();
    if let Some(orientation) = common.orientation {
        let item = one(decimal(tags::IMAGE_ORIENTATION_PATIENT, &orientation));
        shared.put(sequence(tags::PLANE_ORIENTATION_SEQUENCE, vec![item]));
    }
    scan::put_shared(&mut shared, &common.scan);
    let mut measures = InMemDicomObject::new_empty();
    if let Some(spacing) = common.pixel_spacing {
        measures.put(decimal(tags::PIXEL_SPACING, &spacing));
    }
    if let Some(thickness) = common.slice_thickness {
        measures.put(decimal(tags::SLICE_THICKNESS, &[thickness]));
    }
    shared.put(sequence(tags::PIXEL_MEASURES_SEQUENCE, vec![measures]));
    shared
}

/// The functional groups of one frame: its position and its rescale.
fn frame_groups(frame: &EnhancedFrame) -> InMemDicomObject {
    let mut groups = InMemDicomObject::new_empty();
    let position = one(decimal(tags::IMAGE_POSITION_PATIENT, &frame.position));
    groups.put(sequence(tags::PLANE_POSITION_SEQUENCE, vec![position]));
    let mut rescale = InMemDicomObject::new_empty();
    rescale.put(decimal(tags::RESCALE_INTERCEPT, &[frame.intercept]));
    rescale.put(decimal(tags::RESCALE_SLOPE, &[frame.slope]));
    let rescale = vec![rescale];
    groups.put(sequence(tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE, rescale));
    scan::put_frame(&mut groups, frame);
    groups
}

/// The elements a single-frame and a multi-frame file have in common.
fn common_elements(opts: &SliceOptions, sop_class: &str) -> InMemDicomObject {
    let sop_instance = format!("{}.{}", opts.series_uid, opts.instance_number);
    let series_number = opts.series_number.to_string();
    let instance_number = opts.instance_number.to_string();
    let photometric = if opts.monochrome1 {
        "MONOCHROME1"
    } else {
        "MONOCHROME2"
    };
    let text = [
        (tags::SOP_CLASS_UID, VR::UI, sop_class),
        (tags::SOP_INSTANCE_UID, VR::UI, sop_instance.as_str()),
        (tags::MODALITY, VR::CS, opts.modality.as_str()),
        (
            tags::STUDY_DESCRIPTION,
            VR::LO,
            opts.study_description.as_str(),
        ),
        (
            tags::SERIES_DESCRIPTION,
            VR::LO,
            opts.series_description.as_str(),
        ),
        (tags::PATIENT_NAME, VR::PN, opts.patient_name.as_str()),
        (tags::PATIENT_ID, VR::LO, opts.patient_id.as_str()),
        (tags::STUDY_INSTANCE_UID, VR::UI, opts.study_uid.as_str()),
        (tags::SERIES_INSTANCE_UID, VR::UI, opts.series_uid.as_str()),
        (tags::SERIES_NUMBER, VR::IS, series_number.as_str()),
        (tags::INSTANCE_NUMBER, VR::IS, instance_number.as_str()),
        (tags::PHOTOMETRIC_INTERPRETATION, VR::CS, photometric),
    ];
    let numbers = [
        (tags::SAMPLES_PER_PIXEL, 1),
        (tags::ROWS, opts.rows),
        (tags::COLUMNS, opts.cols),
        (tags::BITS_ALLOCATED, 16),
        (tags::BITS_STORED, opts.bits_stored),
        (tags::HIGH_BIT, opts.bits_stored.saturating_sub(1)),
        (tags::PIXEL_REPRESENTATION, u16::from(opts.signed)),
    ];
    let mut obj = InMemDicomObject::new_empty();
    for (tag, vr, value) in text {
        obj.put(element(tag, vr, value));
    }
    for (tag, value) in numbers {
        obj.put(element(tag, VR::US, PrimitiveValue::from(value)));
    }
    if let Some(date) = &opts.study_date {
        obj.put(element(tags::STUDY_DATE, VR::DA, date.as_str()));
    }
    scan::put(&mut obj, &opts.scan, sop_class == ENHANCED_MR_IMAGE_STORAGE);
    obj
}

fn element(tag: Tag, vr: VR, value: impl Into<PrimitiveValue>) -> Element {
    DataElement::new(tag, vr, value.into())
}

/// Numbers as a DICOM decimal string.
fn decimal(tag: Tag, values: &[f64]) -> Element {
    let text = PrimitiveValue::Strs(values.iter().map(ToString::to_string).collect());
    DataElement::new(tag, VR::DS, text)
}

fn sequence(tag: Tag, items: Vec<InMemDicomObject>) -> Element {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}

fn pixels(stored: Vec<u16>) -> Element {
    DataElement::new(tags::PIXEL_DATA, VR::OW, PrimitiveValue::U16(stored.into()))
}

/// A data set holding just one element, as an item of a sequence.
fn one(only: Element) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    item.put(only);
    item
}

fn ramp(count: usize) -> Vec<u16> {
    (0..count)
        .map(|i| u16::try_from(i % 65536).expect("below 65536"))
        .collect()
}

/// Serialise as a file. The data are always explicit VR little endian; when the options name
/// another transfer syntax, only the file header says so.
fn write(obj: InMemDicomObject, opts: &SliceOptions) -> Vec<u8> {
    let header = |syntax: &str| {
        FileMetaTableBuilder::new()
            .transfer_syntax(syntax)
            .implementation_class_uid(IMPLEMENTATION_CLASS)
    };
    let labelled = obj
        .clone()
        .with_meta(header(&opts.transfer_syntax))
        .expect("the synthetic object has its identifiers");
    let honest = obj
        .with_meta(header(EXPLICIT_VR_LITTLE_ENDIAN))
        .expect("the synthetic object has its identifiers");
    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    labelled
        .write_meta(&mut bytes)
        .expect("writing to memory does not fail");
    honest
        .write_dataset(&mut bytes)
        .expect("writing to memory does not fail");
    bytes
}
