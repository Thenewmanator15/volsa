//! Decoding pixels that are stored compressed, each frame in its own fragment.

use std::io::Cursor;

use dicom_core::value::PixelFragmentSequence;
use dicom_core::{DataElement, VR};
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
