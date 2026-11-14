//! Segmentations stored in DICOM (SEG): the parts another program marked on a scan, each a
//! numbered segment with a name and a colour, stored as one mask per frame. They are read here
//! into one grid of segment numbers, placed in the patient, for the labels of a scan to be
//! made of.
//!
//! Masks of two kinds are read: BINARY, one bit a pixel, and FRACTIONAL, one byte a pixel,
//! which is taken as in the segment where it is at least half of the most it can be. Only
//! pixels stored uncompressed are read.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dicom_dictionary_std::{tags, uids};
use dicom_object::{DefaultDicomObject, FileMetaTable, InMemDicomObject};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
use volsa_core::{Affine, Patient, Voxel};

use crate::decode::open;
use crate::document::sop_class_of;
use crate::header::{first_item, int, items, text};
use crate::nesting::preamble;

mod colour;
mod frames;
mod grid;
mod masks;

/// The most voxels a segmentation may have once its frames are put into one grid, as for a
/// label file: 256 Mi, a quarter of a gibibyte of 16-bit numbers.
pub const MAX_SEGMENTATION_VOXELS: u64 = 256 << 20;

/// What is said of a file that is not a segmentation at all.
const NOT_A_SEGMENTATION: &str = "This file is not a segmentation that Volsa can read.";
/// What is said of a segmentation that the DICOM library cannot read.
pub(crate) const DAMAGED: &str = "This segmentation could not be read. The file may be damaged.";

/// One segment: a part another program marked, by its number in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The `SegmentNumber`, which the grid holds where the segment is.
    pub number: u16,
    /// The `SegmentLabel`; empty when the file gives none.
    pub label: String,
    /// The colour the file recommends, as red, green and blue; `None` when it gives none.
    pub colour: Option<[u8; 3]>,
}

/// A segmentation as one grid: the number of the segment at each voxel, 0 where there is
/// none, and where the voxels lie in the patient.
///
/// Where two segments cover one voxel, the one with the larger segment number is kept: a grid
/// holds one number a voxel, and this way the outcome does not hang on the order of the frames.
#[derive(Debug, Clone, PartialEq)]
pub struct Segmentation {
    /// Columns, rows and slices.
    pub dims: [u32; 3],
    /// The segment number at each voxel, columns fastest, then rows, then slices.
    pub numbers: Vec<u16>,
    /// Where the voxel at (column, row, slice) lies in the patient.
    pub placement: Affine<Voxel, Patient>,
    /// The segments the file describes, in its order.
    pub segments: Vec<Segment>,
    /// The `ContentLabel`, or failing that the description of its series; empty when neither
    /// is given.
    pub title: String,
    /// Whether the masks were FRACTIONAL and so were cut at half their most.
    pub fractional: bool,
    /// The `SeriesInstanceUID` of the first series the file says it was made on, if it says.
    pub referenced_series: Option<String>,
}

/// Why a segmentation is not read, in a sentence for the person, with no file name in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentationError(pub String);

impl std::fmt::Display for SegmentationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SegmentationError {}

/// Shorthand for a refusal in one of the sentences above.
pub(crate) fn refused(sentence: &str) -> SegmentationError {
    SegmentationError(sentence.to_owned())
}

/// Read a whole DICOM SEG file into one grid of segment numbers.
///
/// # Errors
///
/// A [`SegmentationError`] saying why, when the file is not a segmentation, its pixels are
/// compressed, its masks are of a kind Volsa does not read, its frames do not lie evenly
/// spaced on one grid or do not say where they lie, it is larger than
/// [`MAX_SEGMENTATION_VOXELS`], or it is damaged or cut short.
pub fn read_segmentation(bytes: &[u8]) -> Result<Segmentation, SegmentationError> {
    catch_unwind(AssertUnwindSafe(|| read(bytes))).unwrap_or_else(|_| Err(refused(DAMAGED)))
}

fn read(bytes: &[u8]) -> Result<Segmentation, SegmentationError> {
    if let Some(name) = compressed(bytes) {
        return Err(SegmentationError(format!(
            "This segmentation is stored compressed ({name}), which Volsa does not read."
        )));
    }
    let file = open(bytes).map_err(|_| refused(DAMAGED))?;
    if !is_segmentation(&file) {
        return Err(refused(NOT_A_SEGMENTATION));
    }
    let obj: &InMemDicomObject = &file;
    let stored = masks::Stored::of(obj)?;
    let frames = frames::of(obj)?;
    let placed = grid::place(&frames, stored.rows, stored.cols)?;
    let numbers = masks::fill(obj, &stored, &frames, &placed)?;
    Ok(Segmentation {
        dims: [stored.cols, stored.rows, placed.slices],
        numbers,
        placement: placed.placement,
        segments: segments_of(obj),
        title: text(obj, tags::CONTENT_LABEL)
            .or_else(|| text(obj, tags::SERIES_DESCRIPTION))
            .unwrap_or_default(),
        fractional: stored.fractional.is_some(),
        referenced_series: first_item(obj, tags::REFERENCED_SERIES_SEQUENCE)
            .and_then(|series| text(series, tags::SERIES_INSTANCE_UID)),
    })
}

/// Whether a file says it is a segmentation of the kind read here.
pub(crate) fn is_segmentation(file: &DefaultDicomObject) -> bool {
    sop_class_of(file).as_deref() == Some(uids::SEGMENTATION_STORAGE)
}

/// The name of the transfer syntax of a file whose pixels are compressed, or that the DICOM
/// library does not know; `None` for one stored as it is, or for bytes with no file header,
/// which opening then refuses.
fn compressed(bytes: &[u8]) -> Option<String> {
    let mut file = Cursor::new(bytes);
    file.set_position(preamble(bytes));
    let meta = FileMetaTable::from_reader(&mut file).ok()?;
    let uid = meta.transfer_syntax().trim_end_matches(['\0', ' ']);
    match TransferSyntaxRegistry.get(uid) {
        Some(syntax) if syntax.is_encapsulated_pixel_data() || syntax.is_unsupported() => {
            Some(syntax.name().to_owned())
        }
        Some(_) => None,
        None => Some(uid.to_owned()),
    }
}

/// The segments a file describes, in its order. An item with no number is passed over.
fn segments_of(obj: &InMemDicomObject) -> Vec<Segment> {
    items(obj, tags::SEGMENT_SEQUENCE)
        .iter()
        .filter_map(|item| {
            let number = u16::try_from(int(item, tags::SEGMENT_NUMBER)?).ok()?;
            Some(Segment {
                number,
                label: text(item, tags::SEGMENT_LABEL).unwrap_or_default(),
                colour: colour::recommended(item),
            })
        })
        .collect()
}

#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod tests;
