//! A frame decoded to its values as stored reads, value by value and to the bit, what the
//! ordinary decoding gives.

use std::io::Cursor;

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use volsa_core::Rescale;

use super::*;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, SliceOptions, classic_slice, enhanced_series,
};

/// Decode a file both ways, check every value is the same to the bit, and give the frame
/// decoded as stored.
fn read_the_same(bytes: &[u8], frame: Option<u32>) -> StoredFrame {
    let file = ImageFile::open(bytes).expect("opens");
    let old = file.decode(frame).expect("decodes");
    let new = file.decode_stored(frame).expect("decodes as stored");
    assert_eq!((new.rows, new.cols), (old.rows, old.cols));
    let bits = |values: &[f32]| {
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&new.values()), bits(&old.values));
    new
}

fn slice(change: impl FnOnce(&mut SliceOptions)) -> Vec<u8> {
    let mut options = SliceOptions::default();
    change(&mut options);
    classic_slice(&options)
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

fn eight_bit(signed: u16) -> Vec<u8> {
    altered(|file| {
        let us = |tag, value: u16| DataElement::new(tag, VR::US, PrimitiveValue::from(value));
        file.put(us(tags::BITS_ALLOCATED, 8));
        file.put(us(tags::BITS_STORED, 8));
        file.put(us(tags::HIGH_BIT, 7));
        file.put(us(tags::PIXEL_REPRESENTATION, signed));
        let pixels: Vec<u8> = (0..16).map(|i| i * 17).collect();
        file.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(pixels),
        ));
    })
}

fn rescale_of(frame: &StoredFrame) -> Option<Rescale> {
    match &frame.samples {
        StoredSamples::Unsigned(_, rescale) | StoredSamples::Signed(_, rescale) => Some(*rescale),
        StoredSamples::Float(_) => None,
    }
}

#[test]
fn stored_ct_values_read_as_their_hounsfield_numbers() {
    let bytes = slice(|options| {
        options.modality = "CT".to_owned();
        options.stored = vec![
            0, 24, 924, 1024, 1064, 2124, 4095, 1, 2, 3, 5, 8, 13, 21, 34, 55,
        ];
        options.intercept = -1024.0;
    });
    let frame = read_the_same(&bytes, None);
    let StoredSamples::Unsigned(samples, rescale) = &frame.samples else {
        panic!("held as unsigned 16-bit: {:?}", frame.samples);
    };
    assert_eq!(
        (samples[3], rescale.value(i32::from(samples[3]))),
        (1024, 0.0)
    );
    assert_eq!(frame.values()[0], -1024.0);
}

#[test]
fn signed_values_read_below_zero() {
    let stored = [
        -1024_i16, -1, 0, 1, 3071, -32768, 32767, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    ];
    let bytes = slice(|options| {
        options.signed = true;
        options.stored = stored.iter().map(|value| value.cast_unsigned()).collect();
    });
    let frame = read_the_same(&bytes, None);
    let StoredSamples::Signed(samples, _) = &frame.samples else {
        panic!("held as signed 16-bit: {:?}", frame.samples);
    };
    assert_eq!(samples[..7], stored[..7]);
}

#[test]
fn a_fractional_slope_and_intercept_read_as_the_ordinary_decoding_does() {
    let bytes = slice(|options| {
        options.slope = 2.5;
        options.intercept = 0.25;
        options.stored = (0..16).map(|value| value * 4099).collect();
    });
    let frame = read_the_same(&bytes, None);
    assert_eq!(
        rescale_of(&frame),
        Some(Rescale {
            slope: 2.5,
            intercept: 0.25
        })
    );
}

#[test]
fn bits_above_bits_stored_are_ignored() {
    let bytes = slice(|options| {
        options.bits_stored = 12;
        options.stored = (0..16).map(|value| 0xF000 | (value * 255)).collect();
    });
    let frame = read_the_same(&bytes, None);
    assert_eq!(frame.values()[1], 255.0);
}

#[test]
fn a_signed_twelve_bit_sample_is_sign_extended() {
    let bytes = slice(|options| {
        options.bits_stored = 12;
        options.signed = true;
        options.stored = vec![
            0x0800, 0x0FFF, 0x07FF, 0x0000, 0xF800, 0xAFFF, 0x0001, 0x0002, 3, 4, 5, 6, 7, 8, 9, 10,
        ];
    });
    let frame = read_the_same(&bytes, None);
    assert_eq!(frame.values()[..3], [-2048.0, -1.0, 2047.0]);
}

#[test]
fn eight_bit_samples_are_read_unsigned_and_signed() {
    let unsigned = read_the_same(&eight_bit(0), None);
    assert!(matches!(unsigned.samples, StoredSamples::Unsigned(..)));
    let signed = read_the_same(&eight_bit(1), None);
    assert!(matches!(signed.samples, StoredSamples::Signed(..)));
    assert_eq!(signed.values()[15], -1.0, "255 stored is -1 signed");
}

#[test]
fn each_frame_of_a_many_frame_file_has_its_own_rescale() {
    let frames = (0..8)
        .map(|place| EnhancedFrame {
            stored: (0..16).map(|pixel| 500 + pixel).collect(),
            slope: f64::from(place) + 1.0,
            intercept: 10.0 * f64::from(place),
            ..EnhancedFrame::at([0.0, 0.0, f64::from(place)])
        })
        .collect();
    let file = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let fifth = read_the_same(&file, Some(5));
    assert_eq!(
        rescale_of(&fifth),
        Some(Rescale {
            slope: 6.0,
            intercept: 50.0
        })
    );
}

#[test]
fn monochrome1_and_a_slope_of_zero_are_read_as_numbers() {
    let inverted = read_the_same(&slice(|options| options.monochrome1 = true), None);
    assert!(matches!(inverted.samples, StoredSamples::Float(_)));
    let flat = read_the_same(&slice(|options| options.slope = 0.0), None);
    assert!(matches!(flat.samples, StoredSamples::Float(_)));
}
