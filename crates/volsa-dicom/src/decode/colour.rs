//! Pictures stored in colour: maps, screenshots, pages of a report.

use dicom_dictionary_std::tags;
use dicom_object::DefaultDicomObject;
use dicom_pixeldata::{DecodedPixelData, PhotometricInterpretation, PlanarConfiguration};
use volsa_core::VolsaError;

use super::{Frame, damaged};

fn refused<T>(why: &str) -> Result<T, VolsaError> {
    Err(VolsaError::Decode(why.to_owned()))
}

/// Whether a file's images are of a kind Volsa shows: grey, or colour of 8 bits a sample.
///
/// # Errors
///
/// [`VolsaError::Decode`], saying which, for palette colour, for colour of more than 8 bits a
/// sample, and for any other number of samples a pixel.
pub(super) fn check(file: &DefaultDicomObject) -> Result<(), VolsaError> {
    let number = |tag| {
        file.get(tag)
            .and_then(|element| element.to_int::<u16>().ok())
    };
    let said = file.get(tags::PHOTOMETRIC_INTERPRETATION);
    let said = said.and_then(|element| element.to_str().ok());
    if said.is_some_and(|said| said.trim().starts_with("PALETTE")) {
        return refused("palette colour images are not supported yet");
    }
    match number(tags::SAMPLES_PER_PIXEL) {
        None | Some(1) => Ok(()),
        Some(3) if number(tags::BITS_ALLOCATED) == Some(8) => Ok(()),
        Some(3) => refused("colour images of more than 8 bits a sample are not supported yet"),
        Some(_) => refused("images with this many samples a pixel are not supported"),
    }
}

/// How bright a colour is, as the eye weighs red, green and blue.
fn brightness([red, green, blue]: [u8; 3]) -> f32 {
    0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue)
}

/// A colour given as brightness and two colour differences, as red, green and blue.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn from_ybr([y, cb, cr]: [u8; 3]) -> [u8; 3] {
    let (y, cb, cr) = (f32::from(y), f32::from(cb) - 128.0, f32::from(cr) - 128.0);
    let level = |value: f32| value.round().clamp(0.0, 255.0) as u8;
    [
        level(y + 1.402 * cr),
        level(y - 0.344_136 * cb - 0.714_136 * cr),
        level(y + 1.772 * cb),
    ]
}

/// The one frame of a decoded colour image, as a colour for each pixel and its brightness.
///
/// # Errors
///
/// [`VolsaError::Decode`] when the data are not three samples for every pixel, or are stored
/// in a way of writing colour Volsa does not read; the way is named.
pub(super) fn frame(decoded: &DecodedPixelData<'_>) -> Result<Frame, VolsaError> {
    let (rows, cols) = (decoded.rows(), decoded.columns());
    let count = rows as usize * cols as usize;
    let bytes = decoded.frame_data(0).map_err(|error| damaged(&error))?;
    if bytes.len() != count * 3 {
        return refused("the pixel data do not match the image size");
    }
    let stored: Vec<[u8; 3]> = if decoded.planar_configuration() == PlanarConfiguration::PixelFirst
    {
        let (first, rest) = bytes.split_at(count);
        let (second, third) = rest.split_at(count);
        let pixels = first.iter().zip(second).zip(third);
        pixels.map(|((a, b), c)| [*a, *b, *c]).collect()
    } else {
        let (pixels, _) = bytes.as_chunks::<3>();
        pixels.to_vec()
    };
    let colour: Vec<[u8; 3]> = match decoded.photometric_interpretation() {
        PhotometricInterpretation::Rgb => stored,
        PhotometricInterpretation::YbrFull => stored.into_iter().map(from_ybr).collect(),
        other => {
            return refused(&format!("colour stored as {other} is not supported yet"));
        }
    };
    Ok(Frame {
        rows,
        cols,
        values: colour.iter().copied().map(brightness).collect(),
        colour: Some(colour),
    })
}
