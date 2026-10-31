//! Turning the stored pixels of one slice into values.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dicom_object::{DefaultDicomObject, ReadError};
use dicom_pixeldata::{PhotometricInterpretation, PixelDecoder};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
use voxa_core::VoxaError;

use crate::index::SliceRef;
use crate::nesting::{TOO_DEEP, TooDeep, check_nesting};
use crate::source::SeriesSource;

mod colour;
mod declared;

use declared::Declared;

/// The pixels of one slice as values, row by row.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Rows in the slice.
    pub rows: u32,
    /// Columns in the slice.
    pub cols: u32,
    /// `rows × cols` values: the one at row `r`, column `c` is `values[r * cols + c]`.
    pub values: Vec<f32>,
    /// For a picture stored in colour, the red, green and blue of each pixel, in the same
    /// order; `values` then holds how bright each pixel is. `None` for a grey image.
    pub colour: Option<Vec<[u8; 3]>>,
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
/// the file is damaged or cut short, says its images are larger than Voxa makes room for (see
/// [`ImageFile::open`]), is in a kind of colour Voxa does not show, or lacks the frame asked
/// for; and the
/// source's own error when the entry cannot be read from it.
pub fn decode_slice(source: &mut dyn SeriesSource, slice: &SliceRef) -> Result<Frame, VoxaError> {
    let bytes = source.read(&slice.entry)?;
    ImageFile::open(&bytes)?.decode(slice.frame)
}

/// One file opened for decoding. The file is parsed once; any of its frames can then be
/// decoded without reading it again, which matters for a file that holds a whole series.
pub struct ImageFile {
    file: DefaultDicomObject,
}

impl ImageFile {
    /// Parse a file and check that its images are of a kind Voxa can decode.
    ///
    /// # Errors
    ///
    /// [`VoxaError::NotDicom`] when the bytes are not a DICOM file,
    /// [`VoxaError::UnsupportedTransferSyntax`], naming the compression, when Voxa cannot read
    /// it, and [`VoxaError::Decode`] when the file is damaged or its images are in a kind of
    /// colour Voxa does not show. A file is refused before any room is made for its pixels
    /// when it says one of its images takes more than 256 MiB once decoded, larger than any
    /// MRI image, or when its pixels are stored uncompressed and it holds fewer bytes of them
    /// than it says.
    pub fn open(bytes: &[u8]) -> Result<Self, VoxaError> {
        guarded(|| {
            let file = open(bytes)?;
            if let Some(name) = unreadable(file.meta().transfer_syntax()) {
                return Err(VoxaError::UnsupportedTransferSyntax(name));
            }
            colour::check(&file)?;
            if let Some(declared) = Declared::of(&file) {
                declared.check()?;
            }
            Ok(Self { file })
        })
    }

    /// Decode one frame, numbered as in [`SliceRef::frame`]: `None` for a file holding a
    /// single slice. The values are as [`decode_slice`] describes.
    ///
    /// # Errors
    ///
    /// [`VoxaError::Decode`] when the pixel data are damaged or cut short, or the file has no
    /// such frame.
    pub fn decode(&self, frame: Option<u32>) -> Result<Frame, VoxaError> {
        guarded(|| self.values_of(frame.unwrap_or(0)))
    }

    fn values_of(&self, frame: u32) -> Result<Frame, VoxaError> {
        let decoded = self
            .file
            .decode_pixel_data_frame(frame)
            .map_err(|error| damaged(&error))?;
        if decoded.samples_per_pixel() == 3 {
            return colour::frame(&decoded);
        }
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
        Ok(Frame {
            rows,
            cols,
            values,
            colour: None,
        })
    }
}

/// Parse a whole file, naming the compression when that is what stops it being read. A file
/// nested too deeply is refused before the library is given it.
pub(crate) fn open(bytes: &[u8]) -> Result<DefaultDicomObject, VoxaError> {
    check_nesting(bytes, None)
        .map_err(|TooDeep| VoxaError::Decode(format!("the file is {TOO_DEEP}")))?;
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

/// Run a reader or decoder, turning a panic inside it into an error. They are other people's
/// code, fed whatever the file holds; one bad file must not stop the program.
pub(crate) fn guarded<T>(work: impl FnOnce() -> Result<T, VoxaError>) -> Result<T, VoxaError> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err(VoxaError::Decode(
            "the reader failed on this file".to_owned(),
        ))
    })
}

#[cfg(test)]
mod colour_tests;
#[cfg(test)]
mod compressed_tests;
#[cfg(test)]
mod declared_tests;
#[cfg(test)]
mod tests;
