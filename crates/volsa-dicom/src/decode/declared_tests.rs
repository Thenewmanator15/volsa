//! A file that says its images are larger than any MRI image, or that holds fewer bytes of
//! pixels than it says, is refused in a sentence before any room is made for its pixels.

use std::io::Cursor;

use dicom_core::value::PixelFragmentSequence;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use voxa_core::VoxaError;

use super::*;
use crate::testkit::{
    EXPLICIT_VR_LITTLE_ENDIAN, EnhancedFrame, EnhancedOptions, MemorySource, SliceOptions,
    classic_slice, enhanced_series,
};

const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
const JPEG_2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";

fn decode(bytes: Vec<u8>, frame: Option<u32>) -> Result<Frame, VoxaError> {
    let mut source = MemorySource::new(vec![("one.dcm".to_owned(), bytes)]);
    let slice = SliceRef {
        entry: "one.dcm".to_owned(),
        frame,
    };
    decode_slice(&mut source, &slice)
}

/// What a refusal says, whole.
fn said(bytes: Vec<u8>, frame: Option<u32>) -> String {
    match decode(bytes, frame) {
        Err(error @ VoxaError::Decode(_)) => error.to_string(),
        other => panic!("not refused as it should be: {other:?}"),
    }
}

/// A made-up file of these elements and pixels, written in `syntax`.
fn written(obj: InMemDicomObject, syntax: &str) -> Vec<u8> {
    let file = obj
        .with_meta(FileMetaTableBuilder::new().transfer_syntax(syntax))
        .expect("the object has its identifiers");
    let mut bytes = Vec::new();
    file.write_all(&mut bytes).expect("writes to memory");
    bytes
}

/// The elements of a made-up slice of 4 by 4 pixels, as the testkit writes it.
fn small_slice() -> InMemDicomObject {
    let plain = classic_slice(&SliceOptions::default());
    dicom_object::from_reader(Cursor::new(plain))
        .expect("parses")
        .into_inner()
}

/// A made-up slice that says it is `rows` by `cols` pixels of `samples` samples of `bits`
/// bits each, in pixels stored the way `syntax` names: its own 32 bytes when they are
/// stored plainly, and when compressed one small fragment that a decoder of run-length
/// encoding would begin to read.
fn declaring(rows: u16, cols: u16, samples: u16, bits: u16, syntax: &str) -> Vec<u8> {
    let mut obj = small_slice();
    let us = |tag, value: u16| DataElement::new(tag, VR::US, PrimitiveValue::from(value));
    let size = [
        (tags::ROWS, rows),
        (tags::COLUMNS, cols),
        (tags::SAMPLES_PER_PIXEL, samples),
        (tags::BITS_ALLOCATED, bits),
        (tags::BITS_STORED, bits),
        (tags::HIGH_BIT, bits - 1),
    ];
    for (tag, value) in size {
        obj.put(us(tag, value));
    }
    if syntax != EXPLICIT_VR_LITTLE_ENDIAN {
        let mut fragment: Vec<u8> = [1, 64].into_iter().flat_map(u32::to_le_bytes).collect();
        fragment.resize(64, 0);
        fragment.extend_from_slice(&[3, 1, 2, 3, 4, 0]);
        let fragments = PixelFragmentSequence::new_fragments(vec![fragment]);
        obj.put(DataElement::new(tags::PIXEL_DATA, VR::OB, fragments));
    }
    written(obj, syntax)
}

#[test]
fn a_compressed_image_said_to_be_larger_than_any_mri_image_is_refused_in_a_sentence() {
    // 12,000 by 12,000 values of 16 bits take 275 MB: a decoder would make room for them all
    // before it found that the fragment holds six bytes.
    let file = declaring(12_000, 12_000, 1, 16, RLE_LOSSLESS);
    assert!(file.len() < 1000, "a small file: {} bytes", file.len());
    assert_eq!(
        said(file, None),
        "The image could not be decoded: its header says each image takes 275 MB, more than \
         any MRI image and more than the 256 MB Voxa makes room for."
    );
}

#[test]
fn a_plain_file_that_holds_fewer_bytes_of_pixels_than_it_says_is_refused_in_a_sentence() {
    let cut_short = classic_slice(&SliceOptions {
        drop_pixels: 5,
        ..SliceOptions::default()
    });
    assert_eq!(
        said(cut_short, None),
        "The image could not be decoded: its header says its pixels take 32 bytes, but the \
         file holds 22: it is damaged or cut short."
    );
    let mut obj = small_slice();
    obj.put(DataElement::new(
        tags::ROWS,
        VR::US,
        PrimitiveValue::from(1000_u16),
    ));
    assert_eq!(
        said(written(obj, EXPLICIT_VR_LITTLE_ENDIAN), None),
        "The image could not be decoded: its header says its pixels take 8000 bytes, but the \
         file holds 32: it is damaged or cut short."
    );
}

#[test]
fn a_file_of_many_frames_that_holds_fewer_than_it_says_is_refused_as_it_is_opened() {
    let frames = (0..3)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i)]))
        .collect();
    let whole = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    assert!(decode(whole.clone(), Some(2)).is_ok(), "the file as made");
    let mut obj = dicom_object::from_reader(Cursor::new(whole))
        .expect("parses")
        .into_inner();
    obj.put(DataElement::new(
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        PrimitiveValue::from("300"),
    ));
    let claims_more = written(obj, EXPLICIT_VR_LITTLE_ENDIAN);
    let refusal = "The image could not be decoded: its header says its pixels take 9600 bytes, \
                   but the file holds 96: it is damaged or cut short.";
    assert_eq!(said(claims_more.clone(), Some(0)), refusal);
    let opened = ImageFile::open(&claims_more).map(|_| ());
    assert!(
        matches!(&opened, Err(VoxaError::Decode(why)) if refusal.contains(why.as_str())),
        "{opened:?}"
    );
}

#[test]
fn a_tiny_file_saying_it_holds_a_huge_image_is_refused_however_its_pixels_are_stored() {
    for syntax in [
        EXPLICIT_VR_LITTLE_ENDIAN,
        RLE_LOSSLESS,
        JPEG_BASELINE,
        JPEG_2000_LOSSLESS,
    ] {
        // 65,535 by 65,535 pixels of three 8-bit samples, and of one 16-bit sample: 12 GB and 8 GB.
        for (samples, bits, megabytes) in [(3, 8, 12_288), (1, 16, 8192)] {
            let file = declaring(u16::MAX, u16::MAX, samples, bits, syntax);
            assert!(file.len() < 1000, "a small file: {} bytes", file.len());
            let refusal = format!(
                "The image could not be decoded: its header says each image takes {megabytes} \
                 MB, more than any MRI image and more than the 256 MB Voxa makes room for."
            );
            assert_eq!(said(file, None), refusal, "{syntax}");
        }
    }
}

/// A file's statements about its pixels, for checking without a file.
fn said_of(rows: u64, cols: u64, bits: u64, frames: u64, present: Option<u64>) -> Declared {
    Declared {
        rows,
        cols,
        samples: 1,
        bits,
        frames,
        present,
    }
}

#[test]
fn an_image_of_256_mib_is_let_through_and_one_a_row_larger_is_not() {
    assert_eq!(declared::MOST_FRAME_BYTES, 256 * 1024 * 1024);
    let at_the_limit = said_of(8192, 16_384, 16, 1, None);
    assert_eq!(at_the_limit.frame_bytes(), declared::MOST_FRAME_BYTES);
    assert!(at_the_limit.check().is_ok());
    let over = said_of(8193, 16_384, 16, 1, None);
    assert!(matches!(over.check(), Err(VoxaError::Decode(why)) if why.contains("257 MB")));
}

#[test]
fn samples_of_one_bit_are_counted_packed_as_stored_and_a_byte_each_as_decoded() {
    // Three frames of 3 by 5 single bits: 45 bits, which fit in 6 bytes.
    let packed = said_of(3, 5, 1, 3, Some(6));
    assert_eq!((packed.stored_bytes(), packed.frame_bytes()), (6, 15));
    assert!(packed.check().is_ok());
    assert!(
        said_of(3, 5, 1, 3, Some(5)).check().is_err(),
        "a byte short"
    );
}

#[test]
fn numbers_no_header_could_mean_stop_at_the_largest_rather_than_go_round() {
    let wild = said_of(u64::MAX, u64::MAX, 64, u64::MAX, Some(10));
    assert_eq!(
        (wild.frame_bytes(), wild.stored_bytes()),
        (u64::MAX, u64::MAX)
    );
    assert!(wild.check().is_err());
}
