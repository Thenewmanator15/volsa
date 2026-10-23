use std::io::{Cursor, Read};

use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::tags;
use proptest::prelude::*;

use super::*;
use crate::header::{NeedMore, read_header, read_header_start};
use crate::source::MAX_ENTRY_BYTES;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, SliceOptions, classic_slice, enhanced_series,
};

/// A reader of `bytes` that counts how many of them it has given.
struct Counted<'a> {
    bytes: &'a [u8],
    given: usize,
}

impl Read for Counted<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = (&self.bytes[self.given..]).read(out)?;
        self.given += count;
        Ok(count)
    }
}

/// The header read from the start of `bytes`, and how many of them were read for it.
fn from_start(bytes: &[u8]) -> (Option<Header>, usize) {
    let mut reader = Counted { bytes, given: 0 };
    let header = read_header_from(&mut reader, MAX_ENTRY_BYTES).expect("reads");
    (header, reader.given)
}

/// Asserts that the header read from the start of `bytes` is the whole file's, and gives how
/// many bytes were read for it.
fn same_as_whole(bytes: &[u8]) -> usize {
    let (header, given) = from_start(bytes);
    assert_eq!(header, read_header(bytes));
    given
}

/// The file `bytes` with `change` made to it, written out again.
fn altered(bytes: Vec<u8>, change: impl FnOnce(&mut dicom_object::DefaultDicomObject)) -> Vec<u8> {
    let mut file = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    change(&mut file);
    let mut out = Vec::new();
    file.write_all(&mut out).expect("writes to memory");
    out
}

/// A slice of `side` by `side` with a private element of `extra` bytes before its pixels.
fn padded_slice(side: u16, extra: usize) -> Vec<u8> {
    let plain = classic_slice(&SliceOptions {
        rows: side,
        cols: side,
        ..SliceOptions::default()
    });
    if extra == 0 {
        return plain;
    }
    altered(plain, |file| {
        let creator = PrimitiveValue::from("MADE UP");
        file.put(DataElement::new(Tag(0x0029, 0x0010), VR::LO, creator));
        let padding = PrimitiveValue::from(vec![0x5Au8; extra]);
        file.put(DataElement::new(Tag(0x0029, 0x1010), VR::OB, padding));
    })
}

/// A whole series of `count` frames of `side` by `side` in one file.
fn many_frames(count: u16, side: u16) -> Vec<u8> {
    let frames = (0..count)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 1.5]))
        .collect();
    enhanced_series(&EnhancedOptions {
        common: SliceOptions {
            rows: side,
            cols: side,
            ..SliceOptions::default()
        },
        frames,
    })
}

const FIRST: usize = 64 * 1024;

#[test]
fn the_header_of_a_slice_is_read_from_its_first_part_alone() {
    let bytes = padded_slice(512, 0);
    assert!(bytes.len() > 8 * FIRST);
    let given = same_as_whole(&bytes);
    assert!(given <= FIRST, "{given} bytes read");
}

#[test]
fn a_header_longer_than_the_first_part_is_read_on_until_it_is_whole() {
    let bytes = padded_slice(512, 150_000);
    let given = same_as_whole(&bytes);
    assert!(
        given > FIRST && given < bytes.len(),
        "{given} of {} bytes read",
        bytes.len()
    );
}

#[test]
fn the_long_header_of_a_file_of_many_frames_is_read_and_its_pixels_are_not() {
    let bytes = many_frames(700, 48);
    let given = same_as_whole(&bytes);
    let header = read_header(&bytes).expect("an image");
    assert_eq!(header.positions.len(), 700);
    assert!(
        given > FIRST && given < bytes.len() / 4,
        "{given} of {} bytes read",
        bytes.len()
    );
}

#[test]
fn a_header_claiming_more_frames_than_the_start_holds_gives_what_the_whole_file_gives() {
    for claimed in ["100000", "1000000"] {
        let bytes = altered(padded_slice(512, 0), |file| {
            let frames = PrimitiveValue::from(claimed);
            file.put(DataElement::new(tags::NUMBER_OF_FRAMES, VR::IS, frames));
        });
        same_as_whole(&bytes);
    }
}

#[test]
fn what_is_not_an_image_gives_nothing_reading_only_its_start() {
    let noise: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let mut magic_then_noise = vec![0u8; 128];
    magic_then_noise.extend_from_slice(b"DICM");
    magic_then_noise.extend_from_slice(&noise);
    let short: &[u8] = b"%PDF-1.7 a report";
    for bytes in [&[][..], short, &noise, &magic_then_noise] {
        let (header, given) = from_start(bytes);
        assert_eq!(header, None);
        assert!(given <= FIRST, "{given} bytes read");
    }
}

#[test]
fn every_start_of_a_small_file_says_it_needs_more_or_gives_the_whole_files_header() {
    let files = [
        padded_slice(4, 0),
        padded_slice(4, 600),
        many_frames(3, 2),
        b"%PDF-1.7 a report".to_vec(),
    ];
    for bytes in files {
        let whole = read_header(&bytes);
        for cut in 0..=bytes.len() {
            let reading = read_header_start(&bytes[..cut], false);
            let fine = reading == Err(NeedMore) || reading == Ok(whole.clone());
            assert!(
                fine,
                "the first {cut} of {} bytes gave {reading:?}",
                bytes.len()
            );
        }
        assert_eq!(read_header_start(&bytes, true), Ok(whole));
    }
}

#[test]
fn reading_more_than_the_limit_for_a_header_is_refused() {
    let bytes = padded_slice(16, 150_000);
    let refused = read_header_from(&mut bytes.as_slice(), 100_000);
    assert!(matches!(refused, Err(VoxaError::Archive(_))), "{refused:?}");
    let header = read_header_from(&mut bytes.as_slice(), 1_000_000).expect("within the limit");
    assert_eq!(header, read_header(&bytes));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn a_header_of_any_length_is_read_as_the_whole_file_reads_it(
        side in 1u16..96,
        half_extra in 0usize..90_000,
    ) {
        let bytes = padded_slice(side, half_extra * 2);
        let (header, _) = from_start(&bytes);
        prop_assert_eq!(header, read_header(&bytes));
    }
}
