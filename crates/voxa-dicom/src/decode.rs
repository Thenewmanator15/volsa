//! Turning the stored pixels of one slice into values.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dicom_dictionary_std::tags;
use dicom_object::{DefaultDicomObject, ReadError};
use dicom_pixeldata::{PhotometricInterpretation, PixelDecoder};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
use voxa_core::VoxaError;

use crate::index::SliceRef;
use crate::source::SeriesSource;

/// The pixels of one slice as values, row by row.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Rows in the slice.
    pub rows: u32,
    /// Columns in the slice.
    pub cols: u32,
    /// `rows × cols` values: the one at row `r`, column `c` is `values[r * cols + c]`.
    pub values: Vec<f32>,
}

/// Decode one slice.
///
/// Each value is `stored × slope + intercept`, with the slope and intercept of that frame.
/// Where the file says bright stored values are dark (`MONOCHROME1`), each value becomes
/// `max + min − value` over the slice, so a larger value is always brighter.
///
/// # Errors
///
/// [`VoxaError::UnsupportedTransferSyntax`], naming the compression, when Voxa cannot read
/// it; [`VoxaError::NotDicom`] when the entry is not a DICOM file; [`VoxaError::Decode`] when
/// the file is damaged or cut short, is in colour, or lacks the frame asked for; and the
/// source's own error when the entry cannot be read from it.
pub fn decode_slice(source: &mut dyn SeriesSource, slice: &SliceRef) -> Result<Frame, VoxaError> {
    let bytes = source.read(&slice.entry)?;
    guarded(|| decode_bytes(&bytes, slice.frame.unwrap_or(0)))
}

fn decode_bytes(bytes: &[u8], frame: u32) -> Result<Frame, VoxaError> {
    let file = open(bytes)?;
    if let Some(name) = unreadable(file.meta().transfer_syntax()) {
        return Err(VoxaError::UnsupportedTransferSyntax(name));
    }
    let samples = file
        .get(tags::SAMPLES_PER_PIXEL)
        .and_then(|element| element.to_int::<u16>().ok());
    if samples.is_some_and(|per_pixel| per_pixel != 1) {
        return Err(VoxaError::Decode(
            "colour images are not supported yet".to_owned(),
        ));
    }
    let decoded = file
        .decode_pixel_data_frame(frame)
        .map_err(|error| damaged(&error))?;
    let mut values: Vec<f32> = decoded.to_vec_frame(0).map_err(|error| damaged(&error))?;
    let (rows, cols) = (decoded.rows(), decoded.columns());
    if values.len() != rows as usize * cols as usize {
        return Err(VoxaError::Decode(
            "the pixel data do not match the image size".to_owned(),
        ));
    }
    if *decoded.photometric_interpretation() == PhotometricInterpretation::Monochrome1 {
        invert(&mut values);
    }
    Ok(Frame { rows, cols, values })
}

/// Parse a whole file, naming the compression when that is what stops it being read.
pub(crate) fn open(bytes: &[u8]) -> Result<DefaultDicomObject, VoxaError> {
    dicom_object::from_reader(Cursor::new(bytes)).map_err(|error| match error {
        ReadError::ReadUnrecognizedTransferSyntax { uid, .. } => {
            VoxaError::UnsupportedTransferSyntax(uid.trim_end_matches(['\0', ' ']).to_owned())
        }
        ReadError::ReadUnsupportedTransferSyntax { name, .. }
        | ReadError::ReadUnsupportedTransferSyntaxWithSuggestion { name, .. } => {
            VoxaError::UnsupportedTransferSyntax(name.to_owned())
        }
        ReadError::ReadPreambleBytes { .. } | ReadError::ParseMetaDataSet { .. } => {
            VoxaError::NotDicom
        }
        _ => VoxaError::Decode("the file is damaged or cut short".to_owned()),
    })
}

/// The name of a transfer syntax whose images Voxa cannot decode; `None` when it can. A
/// syntax unknown to the DICOM library is named by its identifier.
pub(crate) fn unreadable(uid: &str) -> Option<String> {
    let uid = uid.trim_end_matches(['\0', ' ']);
    match TransferSyntaxRegistry.get(uid) {
        Some(syntax) if syntax.can_decode_all() => None,
        Some(syntax) => Some(syntax.name().to_owned()),
        None => Some(uid.to_owned()),
    }
}

fn damaged(error: &dicom_pixeldata::Error) -> VoxaError {
    VoxaError::Decode(format!(
        "the pixel data are damaged or incomplete ({error})"
    ))
}

/// Flip the values about the middle of their range, so the largest becomes the smallest.
fn invert(values: &mut [f32]) {
    let finite = || values.iter().copied().filter(|value| value.is_finite());
    let (Some(low), Some(high)) = (finite().reduce(f32::min), finite().reduce(f32::max)) else {
        return;
    };
    for value in values {
        *value = high + low - *value;
    }
}

/// Run a decoder, turning a panic inside it into an error. The decoders for compressed images
/// are other people's code, fed whatever the file holds; one bad slice must not stop the
/// program.
fn guarded<T>(work: impl FnOnce() -> Result<T, VoxaError>) -> Result<T, VoxaError> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err(VoxaError::Decode(
            "the decoder failed on this image".to_owned(),
        ))
    })
}

#[cfg(test)]
mod compressed_tests;
#[cfg(test)]
mod tests;
