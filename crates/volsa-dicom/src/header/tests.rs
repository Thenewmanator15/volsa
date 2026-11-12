//! Reading headers: what each kind of file gives.

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
fn a_detectors_spacing_is_not_read_as_the_pixel_spacing() {
    let opts = SliceOptions {
        pixel_spacing: None,
        imager_pixel_spacing: Some([0.2, 0.2]),
        ..SliceOptions::default()
    };
    let header = read_header(&classic_slice(&opts)).expect("is an image");
    assert_eq!(header.pixel_spacing, None);
}

#[test]
fn the_rescale_type_of_a_plain_file_is_read_and_trimmed() {
    let named = |rescale_type: Option<&str>| {
        let opts = SliceOptions {
            rescale_type: rescale_type.map(str::to_owned),
            ..SliceOptions::default()
        };
        read_header(&classic_slice(&opts)).expect("is an image")
    };
    assert_eq!(named(Some("HU ")).rescale_type.as_deref(), Some("HU"));
    assert_eq!(named(None).rescale_type, None);
}

#[test]
fn a_monochrome1_header_is_read_as_inverted() {
    let inverted = |monochrome1: bool| {
        let opts = SliceOptions {
            monochrome1,
            ..SliceOptions::default()
        };
        read_header(&classic_slice(&opts))
            .expect("is an image")
            .inverted
    };
    assert!(inverted(true));
    assert!(!inverted(false));
}

#[test]
fn the_rescale_type_of_an_enhanced_file_is_read_from_its_transformation_group() {
    let frames = (0..2)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 2.5]))
        .collect();
    let bytes = enhanced_series(&EnhancedOptions {
        common: SliceOptions {
            rescale_type: Some("US".to_owned()),
            ..SliceOptions::default()
        },
        frames,
    });
    let header = read_header(&bytes).expect("is an image");
    assert!(header.multi_frame);
    assert_eq!(header.rescale_type.as_deref(), Some("US"));
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
