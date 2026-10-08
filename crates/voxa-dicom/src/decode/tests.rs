use std::io::Cursor;

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use proptest::prelude::*;
use voxa_core::VoxaError;

use super::*;
use crate::index::SliceRef;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, MemorySource, SliceOptions, classic_slice, enhanced_series,
};

const NAME: &str = "slice.dcm";
const JPEG_LS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";

fn decode(bytes: Vec<u8>, frame: Option<u32>) -> Result<Frame, VoxaError> {
    let mut source = MemorySource::new(vec![(NAME.to_owned(), bytes)]);
    let slice = SliceRef {
        entry: NAME.to_owned(),
        frame,
    };
    decode_slice(&mut source, &slice)
}

/// A valid slice with some of its elements replaced.
fn altered(change: impl FnOnce(&mut InMemDicomObject)) -> Vec<u8> {
    let valid = classic_slice(&SliceOptions::default());
    let mut file = dicom_object::from_reader(Cursor::new(valid)).expect("parses");
    change(&mut file);
    let mut bytes = Vec::new();
    file.write_all(&mut bytes).expect("writes to memory");
    bytes
}

fn element(
    tag: dicom_core::Tag,
    vr: VR,
    value: impl Into<PrimitiveValue>,
) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, vr, value.into())
}

#[test]
fn decode_scales_stored_values_by_slope_and_intercept() {
    let stored: Vec<u16> = vec![
        0, 1, 2, 3, 10, 100, 1000, 4095, 4096, 20000, 32767, 32768, 40000, 65534, 65535, 7,
    ];
    let opts = SliceOptions {
        stored: stored.clone(),
        slope: 2.0,
        intercept: 10.0,
        ..SliceOptions::default()
    };
    let frame = decode(classic_slice(&opts), None).expect("decodes");
    assert_eq!((frame.rows, frame.cols), (4, 4));
    let expected: Vec<f32> = stored.iter().map(|s| 2.0 * f32::from(*s) + 10.0).collect();
    assert_eq!(frame.values, expected);
}

#[test]
fn decode_keeps_negative_stored_values_exact() {
    // Review focus 4: signed pixels below zero, with a negative intercept on top.
    let signed: [i16; 16] = [
        -32768, -2000, -1024, -1, 0, 1, 2, 1023, 1024, 1025, 3000, 32767, -5, 5, -100, 100,
    ];
    let stored = signed.iter().map(|value| value.cast_unsigned()).collect();
    let opts = SliceOptions {
        stored,
        signed: true,
        intercept: -1024.0,
        ..SliceOptions::default()
    };
    let frame = decode(classic_slice(&opts), None).expect("decodes");
    let expected: Vec<f32> = signed
        .iter()
        .map(|value| f32::from(*value) - 1024.0)
        .collect();
    assert_eq!(frame.values, expected);
}

#[test]
fn decode_inverts_monochrome1_so_the_brightest_stored_pixel_has_the_smallest_value() {
    let stored: Vec<u16> = (0..16).map(|i| i * 100 + 50).collect(); // 50 up to 1550
    let opts = SliceOptions {
        stored: stored.clone(),
        monochrome1: true,
        ..SliceOptions::default()
    };
    let frame = decode(classic_slice(&opts), None).expect("decodes");
    let expected: Vec<f32> = stored
        .iter()
        .map(|s| 1550.0 + 50.0 - f32::from(*s))
        .collect();
    assert_eq!(frame.values, expected);
    assert_eq!(frame.values.last().copied(), Some(50.0));
}

#[test]
fn decode_reads_one_frame_of_a_multi_frame_file_with_that_frames_own_rescale() {
    let frames: Vec<EnhancedFrame> = (0..8u16)
        .map(|i| EnhancedFrame {
            stored: (0..16).map(|pixel| i * 100 + pixel).collect(),
            slope: f64::from(i) + 1.0,
            intercept: f64::from(i) * 10.0,
            ..EnhancedFrame::at([0.0, 0.0, f64::from(i)])
        })
        .collect();
    let file = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let frame = decode(file, Some(5)).expect("decodes");
    let expected: Vec<f32> = (0..16u16)
        .map(|pixel| f32::from(500 + pixel) * 6.0 + 50.0)
        .collect();
    assert_eq!(frame.values, expected);
}

#[test]
fn decode_reads_eight_bit_pixels() {
    let bytes = altered(|obj| {
        obj.put(element(tags::BITS_ALLOCATED, VR::US, 8u16));
        obj.put(element(tags::BITS_STORED, VR::US, 8u16));
        obj.put(element(tags::HIGH_BIT, VR::US, 7u16));
        let pixels: Vec<u8> = (0..16).map(|i| i * 16).collect();
        obj.put(element(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(pixels),
        ));
    });
    let frame = decode(bytes, None).expect("decodes");
    let expected: Vec<f32> = (0..16u8).map(|i| f32::from(i * 16)).collect();
    assert_eq!(frame.values, expected);
}

#[test]
fn decode_names_a_compression_it_has_never_heard_of() {
    let made_up = "1.2.840.10008.9.9.9";
    let opts = SliceOptions {
        transfer_syntax: made_up.to_owned(),
        ..SliceOptions::default()
    };
    let error = decode(classic_slice(&opts), None).expect_err("cannot be read");
    assert!(
        matches!(&error, VoxaError::UnsupportedTransferSyntax(name) if name == made_up),
        "{error:?}"
    );
}

#[test]
fn decode_names_a_compression_it_knows_of_but_cannot_read() {
    let opts = SliceOptions {
        transfer_syntax: JPEG_LS_LOSSLESS.to_owned(),
        ..SliceOptions::default()
    };
    let error = decode(classic_slice(&opts), None).expect_err("cannot be read");
    assert!(
        matches!(&error, VoxaError::UnsupportedTransferSyntax(name) if name.contains("JPEG-LS")),
        "{error:?}"
    );
}

#[test]
fn decode_reports_truncated_pixels_instead_of_panicking() {
    let opts = SliceOptions {
        drop_pixels: 5,
        ..SliceOptions::default()
    };
    let result = decode(classic_slice(&opts), None);
    assert!(matches!(result, Err(VoxaError::Decode(_))), "{result:?}");
}

#[test]
fn decode_reports_a_frame_the_file_does_not_have() {
    let frames = (0..3)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i)]))
        .collect();
    let file = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let result = decode(file, Some(3));
    assert!(matches!(result, Err(VoxaError::Decode(_))), "{result:?}");
}

#[test]
fn decode_declines_colour_images_in_plain_words() {
    let bytes = altered(|obj| {
        obj.put(element(tags::SAMPLES_PER_PIXEL, VR::US, 3u16));
        obj.put(element(tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "RGB"));
    });
    let error = decode(bytes, None).expect_err("colour is out of scope for this milestone");
    assert!(
        matches!(&error, VoxaError::Decode(why) if why.contains("colour")),
        "{error:?}"
    );
}

#[test]
fn decode_of_bytes_that_are_not_dicom_says_so() {
    let result = decode(b"%PDF-1.7 a report".to_vec(), None);
    assert!(matches!(result, Err(VoxaError::NotDicom)), "{result:?}");
}

#[test]
fn decode_of_an_entry_the_source_lacks_is_an_error() {
    let mut source = MemorySource::new(Vec::new());
    let slice = SliceRef {
        entry: "gone.dcm".to_owned(),
        frame: None,
    };
    let result = decode_slice(&mut source, &slice);
    assert!(
        matches!(result, Err(VoxaError::MissingEntry(_))),
        "{result:?}"
    );
}

#[test]
fn a_panic_inside_a_decoder_becomes_a_decode_error() {
    let result: Result<Frame, VoxaError> = guarded(|| panic!("a codec gave up"));
    assert!(matches!(result, Err(VoxaError::Decode(_))), "{result:?}");
}

#[test]
fn unreadable_names_only_what_cannot_be_decoded() {
    assert_eq!(
        unreadable("1.2.840.10008.1.2"),
        None,
        "implicit VR little endian"
    );
    assert_eq!(
        unreadable("1.2.840.10008.1.2.1"),
        None,
        "explicit VR little endian"
    );
    assert_eq!(unreadable("1.2.840.10008.1.2.5"), None, "RLE lossless");
    assert_eq!(unreadable("1.2.840.10008.1.2.4.50"), None, "JPEG baseline");
    assert_eq!(unreadable("1.2.840.10008.1.2.4.70"), None, "JPEG lossless");
    assert_eq!(
        unreadable("1.2.840.10008.1.2.4.90"),
        None,
        "JPEG 2000 lossless"
    );
    assert_eq!(unreadable("1.2.840.10008.1.2.4.91"), None, "JPEG 2000");
    assert_eq!(
        unreadable("1.2.840.10008.1.2.1\0"),
        None,
        "padding is not part of the name"
    );
    assert!(unreadable(JPEG_LS_LOSSLESS).is_some_and(|name| name.contains("JPEG-LS")));
    assert_eq!(unreadable("1.2.3.4"), Some("1.2.3.4".to_owned()));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn decode_never_panics_on_arbitrary_bytes(
        noise in proptest::collection::vec(any::<u8>(), 0..600),
        with_magic in any::<bool>(),
    ) {
        let bytes = if with_magic { [vec![0u8; 128], b"DICM".to_vec(), noise].concat() } else { noise };
        prop_assert!(decode(bytes, None).is_err());
    }

    #[test]
    fn decode_never_panics_on_a_damaged_file(
        damage in proptest::collection::vec((any::<proptest::sample::Index>(), any::<u8>()), 1..8),
        frame in proptest::option::of(0u32..4),
    ) {
        let mut bytes = classic_slice(&SliceOptions::default());
        for (at, value) in damage {
            let at = at.index(bytes.len());
            bytes[at] = value;
        }
        // Either outcome is fine; what matters is that it comes back.
        if let Ok(frame) = decode(bytes, frame) {
            prop_assert_eq!(frame.values.len(), frame.rows as usize * frame.cols as usize);
        }
    }
}
