//! The masks of a segmentation: how they are stored, and putting each frame's mask into its
//! slice of the grid.
//!
//! A BINARY mask is one bit a pixel, the first pixel in the lowest bit of the first byte. The
//! frames follow one another bit after bit, so a frame whose pixels do not fill whole bytes
//! ends part-way through a byte and the next frame starts in the same byte. A FRACTIONAL mask
//! is one byte a pixel, from 0 to its `MaximumFractionalValue`; a pixel at half that or more is
//! taken to be in the segment, and the rest are not.

use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::frames::Frames;
use super::grid::{Placed, TOO_LARGE};
use super::{DAMAGED, SegmentationError, refused};
use crate::header::{int, text};

/// What is said of masks of a kind that is not read.
const OTHER_KIND: &str = "This kind of segmentation is not one Volsa reads.";
/// What is said of a file that holds fewer masks than it says.
const SHORT: &str =
    "This segmentation holds less than its frames need. The file may be damaged or cut short.";

/// How the masks of a segmentation are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stored {
    /// Rows in each frame.
    pub rows: u32,
    /// Columns in each frame.
    pub cols: u32,
    /// For FRACTIONAL masks, the most a pixel can hold; `None` for BINARY masks.
    pub fractional: Option<u16>,
}

impl Stored {
    /// How the masks of `obj` are stored.
    ///
    /// # Errors
    ///
    /// When they are neither BINARY at one bit a pixel nor FRACTIONAL at one byte a pixel with
    /// a most greater than nothing, or the frames have no size.
    pub(super) fn of(obj: &InMemDicomObject) -> Result<Self, SegmentationError> {
        let bits = int(obj, tags::BITS_ALLOCATED);
        let fractional = match text(obj, tags::SEGMENTATION_TYPE).as_deref() {
            Some("BINARY") if bits == Some(1) => None,
            Some("FRACTIONAL") if bits == Some(8) => {
                let most = int(obj, tags::MAXIMUM_FRACTIONAL_VALUE)
                    .and_then(|most| u16::try_from(most).ok())
                    .filter(|most| *most > 0)
                    .ok_or_else(|| refused(OTHER_KIND))?;
                Some(most)
            }
            _ => return Err(refused(OTHER_KIND)),
        };
        let size = |tag| {
            int(obj, tag)
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| refused(DAMAGED))
        };
        Ok(Self {
            rows: size(tags::ROWS)?,
            cols: size(tags::COLUMNS)?,
            fractional,
        })
    }

    /// Whether pixel `at`, counted from the first pixel of the first frame, is in its segment.
    fn on(self, pixels: &[u8], at: usize) -> bool {
        match self.fractional {
            None => pixels
                .get(at / 8)
                .is_some_and(|byte| (byte >> (at % 8)) & 1 == 1),
            Some(most) => pixels
                .get(at)
                .is_some_and(|value| *value > 0 && u16::from(*value) * 2 >= most),
        }
    }

    /// How many bytes `frames` frames of these masks take.
    fn bytes_for(self, frames: usize) -> Option<usize> {
        let pixels = self.pixels()?.checked_mul(frames)?;
        Some(match self.fractional {
            None => pixels.div_ceil(8),
            Some(_) => pixels,
        })
    }

    /// The pixels in one frame.
    fn pixels(self) -> Option<usize> {
        let rows = usize::try_from(self.rows).ok()?;
        rows.checked_mul(usize::try_from(self.cols).ok()?)
    }
}

/// The grid of segment numbers: each frame's mask put into its slice, with its segment number
/// where the mask is on. Where two masks are on at one voxel, the larger number is kept.
///
/// # Errors
///
/// When the file holds no masks or fewer bytes of them than its frames need.
pub(super) fn fill(
    obj: &InMemDicomObject,
    stored: &Stored,
    frames: &Frames,
    placed: &Placed,
) -> Result<Vec<u16>, SegmentationError> {
    let element = obj.get(tags::PIXEL_DATA).ok_or_else(|| refused(SHORT))?;
    let pixels = element.to_bytes().map_err(|_| refused(DAMAGED))?;
    let needed = stored.bytes_for(frames.segments.len());
    if needed.is_none_or(|needed| pixels.len() < needed) {
        return Err(refused(SHORT));
    }
    let each = stored.pixels().ok_or_else(|| refused(TOO_LARGE))?;
    let slices = usize::try_from(placed.slices).map_err(|_| refused(TOO_LARGE))?;
    let mut numbers = vec![0u16; each.checked_mul(slices).ok_or_else(|| refused(TOO_LARGE))?];
    let frame_slices = frames.segments.iter().zip(&placed.slice_of);
    for (frame, (segment, slice)) in frame_slices.enumerate() {
        let slice = usize::try_from(*slice).map_err(|_| refused(TOO_LARGE))?;
        let into = numbers.get_mut(slice * each..(slice + 1) * each);
        let into = into.ok_or_else(|| refused(TOO_LARGE))?;
        let start = frame * each;
        for (pixel, number) in into.iter_mut().enumerate() {
            if stored.on(&pixels, start + pixel) {
                *number = (*number).max(*segment);
            }
        }
    }
    Ok(numbers)
}
