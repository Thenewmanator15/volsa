//! Decoding pixels that are stored compressed, each frame in its own fragment.

use std::io::Cursor;

use dicom_core::value::PixelFragmentSequence;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::FileMetaTableBuilder;

use super::*;
use crate::testkit::{MemorySource, SliceOptions, classic_slice};

const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";

/// One frame of 16-bit pixels in DICOM run-length encoding: a 64-byte table of where each
/// byte plane starts, then the high bytes and the low bytes, each written as literal runs.
fn rle_frame(stored: &[u16]) -> Vec<u8> {
    let plane = |which: usize| {
        let mut segment = Vec::new();
        let bytes: Vec<u8> = stored
            .iter()
            .map(|value| value.to_be_bytes()[which])
            .collect();
        for run in bytes.chunks(128) {
            segment.push(u8::try_from(run.len() - 1).expect("at most 128 bytes in a run"));
            segment.extend_from_slice(run);
        }
        if segment.len() % 2 == 1 {
            segment.push(0); // segments are padded to an even length
        }
        segment
    };
    let (high, low) = (plane(0), plane(1));
    let second_starts = 64 + u32::try_from(high.len()).expect("a small segment");
    let mut frame = Vec::new();
    for entry in [2, 64, second_starts].into_iter().chain([0; 13]) {
        frame.extend_from_slice(&u32::to_le_bytes(entry));
    }
    frame.extend_from_slice(&high);
    frame.extend_from_slice(&low);
    frame
}

#[test]
fn decode_reads_run_length_compressed_pixels() {
    let stored: Vec<u16> = (0..16).map(|i| i * 257 + 300).collect();
    let plain = classic_slice(&SliceOptions {
        slope: 2.0,
        ..SliceOptions::default()
    });
    let mut obj = dicom_object::from_reader(Cursor::new(plain))
        .expect("parses")
        .into_inner();
    let fragments = PixelFragmentSequence::new_fragments(vec![rle_frame(&stored)]);
    obj.put(DataElement::new(tags::PIXEL_DATA, VR::OB, fragments));
    let file = obj
        .with_meta(FileMetaTableBuilder::new().transfer_syntax(RLE_LOSSLESS))
        .expect("the object has its identifiers");
    let mut bytes = Vec::new();
    file.write_all(&mut bytes).expect("writes to memory");

    let mut source = MemorySource::new(vec![("rle.dcm".to_owned(), bytes)]);
    let slice = SliceRef {
        entry: "rle.dcm".to_owned(),
        frame: None,
    };
    let frame = decode_slice(&mut source, &slice).expect("decodes");
    let expected: Vec<f32> = stored.iter().map(|value| 2.0 * f32::from(*value)).collect();
    assert_eq!(frame.values, expected);
}

const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
const JPEG_2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
const DEFLATED: &str = "1.2.840.10008.1.2.1.99";

/// A made-up square slice whose pixels are one compressed fragment, in a file that names the
/// way they are compressed. `bits` is how many bits each stored value has.
fn compressed(side: u16, bits: u16, syntax: &str, mut fragment: Vec<u8>) -> Vec<u8> {
    let plain = classic_slice(&SliceOptions {
        rows: side,
        cols: side,
        ..SliceOptions::default()
    });
    let mut obj = dicom_object::from_reader(Cursor::new(plain))
        .expect("parses")
        .into_inner();
    let stored = [
        (tags::BITS_ALLOCATED, bits),
        (tags::BITS_STORED, bits),
        (tags::HIGH_BIT, bits - 1),
    ];
    for (tag, value) in stored {
        obj.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
    }
    if fragment.len() % 2 == 1 {
        fragment.push(0); // a fragment is padded to an even length
    }
    let fragments = PixelFragmentSequence::new_fragments(vec![fragment]);
    obj.put(DataElement::new(tags::PIXEL_DATA, VR::OB, fragments));
    let file = obj
        .with_meta(FileMetaTableBuilder::new().transfer_syntax(syntax))
        .expect("the object has its identifiers");
    let mut bytes = Vec::new();
    file.write_all(&mut bytes).expect("writes to memory");
    bytes
}

fn decoded(bytes: Vec<u8>) -> Frame {
    let mut source = MemorySource::new(vec![("one.dcm".to_owned(), bytes)]);
    let slice = SliceRef {
        entry: "one.dcm".to_owned(),
        frame: None,
    };
    decode_slice(&mut source, &slice).expect("decodes")
}

#[test]
fn decode_reads_a_jpeg_picture() {
    // A made-up picture that climbs gently across and down: JPEG keeps such a picture nearly
    // as it was.
    let picture: Vec<u8> = (0..256u16)
        .map(|place| u8::try_from(40 + (place % 16) * 6 + (place / 16) * 4).expect("below 256"))
        .collect();
    let mut jpeg = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut jpeg, 95);
    encoder
        .encode(&picture, 16, 16, jpeg_encoder::ColorType::Luma)
        .expect("encodes");
    let frame = decoded(compressed(16, 8, JPEG_BASELINE, jpeg));
    assert_eq!((frame.rows, frame.cols), (16, 16));
    for (value, wanted) in frame.values.iter().zip(&picture) {
        assert!(
            (value - f32::from(*wanted)).abs() <= 6.0,
            "{value} for {wanted}"
        );
    }
}

#[test]
fn decode_reads_a_jpeg_2000_picture_exactly() {
    // Written by tools/fixtures/make_ramp_j2k.py: 16 by 16 values climbing by 37 from 1000.
    let fragment = include_bytes!("../../tests/fixtures/ramp.j2k").to_vec();
    let frame = decoded(compressed(16, 16, JPEG_2000_LOSSLESS, fragment));
    let ramp: Vec<f32> = (0..256u16)
        .map(|place| f32::from(1000 + 37 * place))
        .collect();
    assert_eq!(frame.values, ramp);
}

#[test]
fn decode_reads_a_file_whose_data_set_is_deflated() {
    use std::io::Write;

    // The made-up file names the deflated way of writing and holds its data set as it stood.
    // The data set starts after the 128 empty bytes, the four letters, and the group of
    // elements about the file, whose first element (12 bytes) says how long the rest is.
    let whole = classic_slice(&SliceOptions {
        transfer_syntax: DEFLATED.to_owned(),
        ..SliceOptions::default()
    });
    let group = <[u8; 4]>::try_from(&whole[140..144]).expect("four bytes");
    let (head, data_set) = whole.split_at(132 + 12 + u32::from_le_bytes(group) as usize);
    let mut deflater =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    deflater.write_all(data_set).expect("deflates");
    let bytes = [head, &deflater.finish().expect("deflates")].concat();
    assert_eq!(
        unreadable(DEFLATED),
        None,
        "a series of such files is not marked"
    );
    let plain = decoded(classic_slice(&SliceOptions::default()));
    assert_eq!(decoded(bytes).values, plain.values);
}
