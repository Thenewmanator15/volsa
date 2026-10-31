//! Builders of synthetic DICOM files, for tests in this crate and the crates above it.
//!
//! Real scans never enter the repository, so every test builds the files it needs here: a
//! single-frame MR slice ([`classic_slice`]), a whole series in one multi-frame file
//! ([`enhanced_series`]), and a source held in memory ([`MemorySource`]).
//!
//! This is test support code: a mistake in a builder should stop the test at once, so unlike
//! the rest of the crate it is allowed to panic.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use voxa_core::VoxaError;

use crate::source::{Entry, SeriesSource};

mod build;
mod nested;
#[cfg(test)]
mod tests;

pub use build::{classic_slice, enhanced_series};
pub use nested::{IMPLICIT_VR_LITTLE_ENDIAN, Nesting, nested_slice};

/// Transfer syntax the builders write: explicit VR little endian.
pub const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";

/// Everything about one single-frame slice. Start from [`SliceOptions::default`] and change
/// what the test is about.
#[derive(Debug, Clone)]
pub struct SliceOptions {
    /// `StudyInstanceUID`.
    pub study_uid: String,
    /// `StudyDate` as `YYYYMMDD`, or absent.
    pub study_date: Option<String>,
    /// `StudyDescription`.
    pub study_description: String,
    /// `PatientName`; the default is an obviously made-up phantom.
    pub patient_name: String,
    /// `PatientID`.
    pub patient_id: String,
    /// `SeriesInstanceUID`.
    pub series_uid: String,
    /// `SeriesNumber`.
    pub series_number: i32,
    /// `SeriesDescription`.
    pub series_description: String,
    /// `Modality`.
    pub modality: String,
    /// `InstanceNumber`; also the last part of the `SOPInstanceUID`.
    pub instance_number: i32,
    /// `Rows`.
    pub rows: u16,
    /// `Columns`.
    pub cols: u16,
    /// `ImagePositionPatient`, or absent.
    pub position: Option<[f64; 3]>,
    /// `ImageOrientationPatient`, or absent.
    pub orientation: Option<[f64; 6]>,
    /// `PixelSpacing` (between rows, between columns), or absent.
    pub pixel_spacing: Option<[f64; 2]>,
    /// `SliceThickness`, or absent.
    pub slice_thickness: Option<f64>,
    /// Stored 16-bit values, row by row. Empty means a ramp 0, 1, 2, … For signed pixels give
    /// the two's-complement bit pattern (`-5i16 as u16`).
    pub stored: Vec<u16>,
    /// `PixelRepresentation`: whether stored values are signed.
    pub signed: bool,
    /// `RescaleSlope`.
    pub slope: f64,
    /// `RescaleIntercept`.
    pub intercept: f64,
    /// Write `MONOCHROME1` (bright stored values are dark) instead of `MONOCHROME2`.
    pub monochrome1: bool,
    /// Transfer syntax named in the file header. The data are always written as explicit VR
    /// little endian; any other value only relabels the file, which is how a
    /// test makes a file claiming a syntax Voxa cannot read.
    pub transfer_syntax: String,
    /// Leave this many values off the end of the pixel data, as a truncated file would.
    pub drop_pixels: usize,
    /// What the file says about how it was acquired; nothing unless given.
    pub scan: ScanTags,
    /// Make the image a picture in colour, with these pixels in place of `stored`.
    pub colour: Option<Colour>,
}

/// A made-up picture stored in colour.
#[derive(Debug, Clone)]
pub struct Colour {
    /// The three samples of each pixel, row by row.
    pub pixels: Vec<[u8; 3]>,
    /// Store all the first samples, then all the second, then all the third.
    pub planar: bool,
    /// `PhotometricInterpretation`: what the three samples are.
    pub photometric: String,
    /// `BitsAllocated` for each sample.
    pub bits: u16,
    /// `SamplesPerPixel`; only this many of each pixel's samples are written.
    pub samples: u16,
}

impl Colour {
    /// These pixels as red, green and blue, 8 bits each, pixel by pixel.
    pub fn of(pixels: Vec<[u8; 3]>) -> Self {
        Self {
            pixels,
            planar: false,
            photometric: "RGB".to_owned(),
            bits: 8,
            samples: 3,
        }
    }
}

/// What a made-up file says about how it was acquired, and what sets it apart from the
/// other images at its position. Whatever is left empty or `None` is not written.
#[derive(Debug, Clone, Default)]
pub struct ScanTags {
    /// `ScanningSequence`: codes such as `SE`, `GR`, `IR`, `EP`, separated by a backslash.
    pub sequence: String,
    /// `MRAcquisitionType`: `2D` or `3D`.
    pub acquired: String,
    /// `RepetitionTime`, in milliseconds.
    pub repetition_ms: Option<f64>,
    /// `EchoTime`, in milliseconds.
    pub echo_ms: Option<f64>,
    /// `InversionTime`, in milliseconds.
    pub inversion_ms: Option<f64>,
    /// `FlipAngle`, in degrees.
    pub flip_deg: Option<f64>,
    /// `MagneticFieldStrength`, in tesla.
    pub field_t: Option<f64>,
    /// `BodyPartExamined`.
    pub body_part: String,
    /// `TemporalPositionIdentifier`.
    pub temporal: Option<i32>,
    /// `DiffusionBValue`.
    pub b_value: Option<f64>,
    /// `TriggerTime`, in milliseconds.
    pub trigger_ms: Option<f64>,
    /// `AcquisitionNumber`.
    pub acquisition: Option<i32>,
}

impl Default for SliceOptions {
    fn default() -> Self {
        Self {
            study_uid: "1.2.826.0.1.3680043.9.9999.1".to_owned(),
            study_date: Some("20200102".to_owned()),
            study_description: "Synthetic study".to_owned(),
            patient_name: "Phantom^Test".to_owned(),
            patient_id: "PHANTOM-001".to_owned(),
            series_uid: "1.2.826.0.1.3680043.9.9999.1.1".to_owned(),
            series_number: 1,
            series_description: "synthetic".to_owned(),
            modality: "MR".to_owned(),
            instance_number: 1,
            rows: 4,
            cols: 4,
            position: Some([-100.0, -120.0, 0.0]),
            orientation: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            pixel_spacing: Some([0.9, 0.9]),
            slice_thickness: Some(5.0),
            stored: Vec::new(),
            signed: false,
            slope: 1.0,
            intercept: 0.0,
            monochrome1: false,
            transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN.to_owned(),
            drop_pixels: 0,
            scan: ScanTags::default(),
            colour: None,
        }
    }
}

/// One frame of an enhanced (multi-frame) series.
#[derive(Debug, Clone)]
pub struct EnhancedFrame {
    /// `ImagePositionPatient` of this frame.
    pub position: [f64; 3],
    /// Stored 16-bit values, row by row. Empty means every pixel holds the frame's index.
    pub stored: Vec<u16>,
    /// `RescaleSlope` of this frame.
    pub slope: f64,
    /// `RescaleIntercept` of this frame.
    pub intercept: f64,
    /// The frame's diffusion weighting, or absent.
    pub b_value: Option<f64>,
    /// The frame's echo time in milliseconds, or absent.
    pub echo_ms: Option<f64>,
    /// Which time point the frame is, or absent.
    pub temporal: Option<i32>,
}

impl EnhancedFrame {
    /// A frame at `position` with unit slope, zero intercept and default pixels.
    pub fn at(position: [f64; 3]) -> Self {
        Self {
            position,
            stored: Vec::new(),
            slope: 1.0,
            intercept: 0.0,
            b_value: None,
            echo_ms: None,
            temporal: None,
        }
    }
}

/// Everything about a whole series stored in one multi-frame file.
#[derive(Debug, Clone)]
pub struct EnhancedOptions {
    /// The values shared with single-frame files: study, patient, series, size, orientation,
    /// pixel spacing and thickness. Its `position`, `stored`, `slope` and `intercept` are
    /// ignored in favour of each frame's own.
    pub common: SliceOptions,
    /// The frames, in the order they are stored in the file.
    pub frames: Vec<EnhancedFrame>,
}

/// A source held in memory: named byte strings.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    files: Vec<(String, Vec<u8>)>,
}

impl MemorySource {
    /// A source of the given `(name, contents)` pairs, yielded in name order.
    pub fn new(mut files: Vec<(String, Vec<u8>)>) -> Self {
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Self { files }
    }
}

impl SeriesSource for MemorySource {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        Box::new(self.files.iter().map(|(name, bytes)| {
            Ok(Entry {
                name: name.clone(),
                bytes: bytes.clone(),
            })
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
        self.files
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| VoxaError::MissingEntry(name.to_owned()))
    }
}
