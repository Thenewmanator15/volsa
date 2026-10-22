//! What a file says its pixels take, held against a limit and against what the file holds,
//! before any of them is decoded. A decoder makes room for a frame from what the header says,
//! so a small file that says it holds a huge image could ask for more memory than the
//! computer has, and a request for memory that fails stops the program outright.

use dicom_core::value::Value;
use dicom_dictionary_std::tags;
use dicom_object::DefaultDicomObject;
use voxa_core::VoxaError;

/// The most bytes one frame may take once decoded: 256 MiB. No MRI image comes near it: a
/// slice of 1024 by 1024 values of 16 bits takes 2 MiB, and a 4K screen picture in colour
/// 24 MiB. Decoding a frame takes up to five times its decoded size again, as values.
pub(crate) const MOST_FRAME_BYTES: u64 = 256 << 20;
/// A mebibyte, which a refusal calls a megabyte.
const MEGABYTE: u64 = 1 << 20;

/// What a file says of its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Declared {
    /// `Rows`.
    pub rows: u64,
    /// `Columns`.
    pub cols: u64,
    /// `SamplesPerPixel`.
    pub samples: u64,
    /// `BitsAllocated`: how many bits each sample is stored in.
    pub bits: u64,
    /// `NumberOfFrames`.
    pub frames: u64,
    /// For pixels stored uncompressed, how many bytes of them the file holds; `None` when
    /// they are compressed, which says nothing of how large they are once decoded.
    pub present: Option<u64>,
}

impl Declared {
    /// What the file says of its pixels. `None` when it does not say how many rows, columns
    /// or bits they have, which the decoder refuses on its own before it makes any room.
    pub(crate) fn of(file: &DefaultDicomObject) -> Option<Self> {
        let number = |tag| {
            file.get(tag)
                .and_then(|element| element.to_int::<u64>().ok())
        };
        let present = file
            .get(tags::PIXEL_DATA)
            .and_then(|pixels| match pixels.value() {
                Value::Primitive(stored) => u64::try_from(stored.calculate_byte_len()).ok(),
                _ => None,
            });
        Some(Self {
            rows: number(tags::ROWS)?,
            cols: number(tags::COLUMNS)?,
            samples: number(tags::SAMPLES_PER_PIXEL).unwrap_or(1),
            bits: number(tags::BITS_ALLOCATED)?,
            frames: number(tags::NUMBER_OF_FRAMES).unwrap_or(1).max(1),
            present,
        })
    }

    /// What one frame takes once decoded: a byte or more for each sample, as a decoder makes
    /// room for it. A sample of one bit is decoded into a byte of its own.
    pub(crate) fn frame_bytes(&self) -> u64 {
        self.samples_in_a_frame()
            .saturating_mul(self.bits.div_ceil(8).max(1))
    }

    /// How many samples one frame has: a header can say anything, so this stops at the
    /// largest number rather than going round.
    fn samples_in_a_frame(&self) -> u64 {
        self.rows
            .saturating_mul(self.cols)
            .saturating_mul(self.samples)
    }

    /// The fewest bytes the frames can be stored in uncompressed: samples of one bit packed
    /// eight to a byte, and wider ones in whole bytes.
    pub(crate) fn stored_bytes(&self) -> u64 {
        let samples = self.samples_in_a_frame().saturating_mul(self.frames);
        if self.bits == 1 {
            samples.div_ceil(8)
        } else {
            samples.saturating_mul(self.bits.div_ceil(8))
        }
    }

    /// Whether the pixels can be decoded without asking for more memory than the limit, or
    /// for more than the file holds.
    ///
    /// # Errors
    ///
    /// [`VoxaError::Decode`], in a sentence, when one frame would take more than
    /// [`MOST_FRAME_BYTES`] once decoded, or when pixels stored uncompressed take fewer bytes
    /// than the header says they do.
    pub(crate) fn check(&self) -> Result<(), VoxaError> {
        let frame = self.frame_bytes();
        if frame > MOST_FRAME_BYTES {
            let (takes, most) = (frame.div_ceil(MEGABYTE), MOST_FRAME_BYTES / MEGABYTE);
            return Err(VoxaError::Decode(format!(
                "its header says each image takes {takes} MB, more than any MRI image and \
                 more than the {most} MB Voxa makes room for"
            )));
        }
        let (stored, present) = (self.stored_bytes(), self.present);
        match present {
            Some(present) if present < stored => Err(VoxaError::Decode(format!(
                "its header says its pixels take {stored} bytes, but the file holds \
                 {present}: it is damaged or cut short"
            ))),
            _ => Ok(()),
        }
    }
}
