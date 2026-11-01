//! Writing a made-up picture stored in colour.

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{Colour, element};

/// Make `obj` a picture in colour: the elements that say how it is stored, and its pixels,
/// less `drop` bytes off the end.
pub(super) fn put(obj: &mut InMemDicomObject, colour: &Colour, drop: usize) {
    let numbers = [
        (tags::SAMPLES_PER_PIXEL, colour.samples),
        (tags::PLANAR_CONFIGURATION, u16::from(colour.planar)),
        (tags::BITS_ALLOCATED, colour.bits),
        (tags::BITS_STORED, colour.bits),
        (tags::HIGH_BIT, colour.bits.saturating_sub(1)),
        (tags::PIXEL_REPRESENTATION, 0),
    ];
    for (tag, value) in numbers {
        obj.put(element(tag, VR::US, PrimitiveValue::from(value)));
    }
    let said = colour.photometric.as_str();
    obj.put(element(tags::PHOTOMETRIC_INTERPRETATION, VR::CS, said));
    let samples = usize::from(colour.samples).min(3);
    let sample = |pixel: &[u8; 3], at: usize| pixel.get(at).copied().unwrap_or(0);
    let mut bytes: Vec<u8> = if colour.planar {
        let plane = |at: usize| colour.pixels.iter().map(move |pixel| sample(pixel, at));
        (0..samples).flat_map(plane).collect()
    } else {
        let pixels = colour.pixels.iter();
        pixels
            .flat_map(|pixel| (0..samples).map(move |at| sample(pixel, at)))
            .collect()
    };
    bytes.truncate(bytes.len().saturating_sub(drop));
    let data = if colour.bits == 8 {
        // An odd number of bytes is padded to an even one, as the standard asks.
        if bytes.len() % 2 == 1 && drop == 0 {
            bytes.push(0);
        }
        DataElement::new(tags::PIXEL_DATA, VR::OB, PrimitiveValue::U8(bytes.into()))
    } else {
        let words: Vec<u16> = bytes.into_iter().map(u16::from).collect();
        DataElement::new(tags::PIXEL_DATA, VR::OW, PrimitiveValue::U16(words.into()))
    };
    obj.put(data);
}
