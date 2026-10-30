//! A frame decoded to its numbers as stored, with the rescale that makes them values: half
//! the memory of 32-bit values for scans stored in 16 bits, and the same values to the bit.

use dicom_pixeldata::{
    DecodedPixelData, PhotometricInterpretation, PixelDecoder, PixelRepresentation,
};
use voxa_core::{Rescale, VoxaError};

use super::{ImageFile, damaged, guarded};

/// One frame's numbers as stored, and how they become values.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFrame {
    /// Rows in the frame.
    pub rows: u32,
    /// Columns in the frame.
    pub cols: u32,
    /// `rows × cols` numbers, row by row.
    pub samples: StoredSamples,
}

/// A frame's numbers: whole numbers of at most 16 bits with the frame's rescale, or the
/// values themselves where whole numbers and one rescale cannot give them exactly.
#[derive(Debug, Clone, PartialEq)]
pub enum StoredSamples {
    /// Unsigned whole numbers.
    Unsigned(Vec<u16>, Rescale),
    /// Signed whole numbers.
    Signed(Vec<i16>, Rescale),
    /// The values, as [`ImageFile::decode`] gives them.
    Float(Vec<f32>),
}

impl StoredFrame {
    /// The frame's values, as [`ImageFile::decode`] gives them.
    pub fn values(&self) -> Vec<f32> {
        match &self.samples {
            StoredSamples::Unsigned(samples, rescale) => samples
                .iter()
                .map(|sample| rescale.value(i32::from(*sample)))
                .collect(),
            StoredSamples::Signed(samples, rescale) => samples
                .iter()
                .map(|sample| rescale.value(i32::from(*sample)))
                .collect(),
            StoredSamples::Float(values) => values.clone(),
        }
    }
}

impl ImageFile {
    /// Decode one frame, numbered as in [`ImageFile::decode`], to its numbers as stored. A
    /// grey frame of 8 or 16 bits with a slope above 0 gives whole numbers, each masked to
    /// Bits Stored and, if signed, sign-extended, with the frame's own rescale: the numbers
    /// the DICOM library's own conversion turns into values. Any other frame (inverted grey,
    /// colour, a slope of 0) gives its values.
    ///
    /// # Errors
    ///
    /// As [`ImageFile::decode`].
    pub fn decode_stored(&self, frame: Option<u32>) -> Result<StoredFrame, VoxaError> {
        let at = frame.unwrap_or(0);
        let whole = guarded(|| {
            let decoded = self
                .file
                .decode_pixel_data_frame(at)
                .map_err(|error| damaged(&error))?;
            Ok(whole_numbers(&decoded))
        })?;
        if let Some(stored) = whole {
            return stored;
        }
        let values = self.decode(frame)?;
        Ok(StoredFrame {
            rows: values.rows,
            cols: values.cols,
            samples: StoredSamples::Float(values.values),
        })
    }
}

/// The frame's whole numbers and rescale; `None` when the frame is not one that has them.
fn whole_numbers(decoded: &DecodedPixelData<'_>) -> Option<Result<StoredFrame, VoxaError>> {
    let grey = *decoded.photometric_interpretation() == PhotometricInterpretation::Monochrome2
        && decoded.samples_per_pixel() == 1;
    let bits = decoded.bits_allocated();
    let stored_bits = decoded.bits_stored().clamp(1, bits.max(1));
    let rescale = decoded.rescale().ok()?.first().map(|found| Rescale {
        slope: found.slope,
        intercept: found.intercept,
    })?;
    let usable = rescale.slope.is_finite() && rescale.slope > 0.0 && rescale.intercept.is_finite();
    if !grey || !matches!(bits, 8 | 16) || !usable {
        return None;
    }
    let (rows, cols) = (decoded.rows(), decoded.columns());
    let bytes = match decoded.frame_data(0) {
        Ok(bytes) => bytes,
        Err(error) => return Some(Err(damaged(&error))),
    };
    let raw: Vec<u16> = if bits == 8 {
        bytes.iter().map(|byte| u16::from(*byte)).collect()
    } else {
        let (pairs, _) = bytes.as_chunks::<2>();
        pairs.iter().map(|pair| u16::from_ne_bytes(*pair)).collect()
    };
    if raw.len() != rows as usize * cols as usize {
        return Some(Err(VoxaError::Decode(
            "the pixel data do not match the image size".to_owned(),
        )));
    }
    // For 8-bit data the library's table is always 8 bits wide, whatever Bits Stored says.
    let width = if bits == 8 { 8 } else { stored_bits };
    let samples = if decoded.pixel_representation() == PixelRepresentation::Signed {
        StoredSamples::Signed(raw.iter().map(|n| signed(*n, width)).collect(), rescale)
    } else {
        StoredSamples::Unsigned(raw.iter().map(|n| n & mask(width)).collect(), rescale)
    };
    Some(Ok(StoredFrame {
        rows,
        cols,
        samples,
    }))
}

/// The lowest `width` bits set.
fn mask(width: u16) -> u16 {
    u16::MAX >> (16 - width.clamp(1, 16))
}

/// A stored number of `width` bits read as signed: its lowest `width` bits, with the top one
/// of them the sign.
fn signed(raw: u16, width: u16) -> i16 {
    let width = width.clamp(1, 16);
    let kept = i32::from(raw & mask(width));
    let value = if kept >= 1 << (width - 1) {
        kept - (1 << width)
    } else {
        kept
    };
    i16::try_from(value).unwrap_or_default()
}
