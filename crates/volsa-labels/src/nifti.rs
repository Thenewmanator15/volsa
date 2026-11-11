//! Reading a label volume from a NIfTI-1 file: the 348 bytes of its header, then one whole
//! number for every voxel. The format is described at <https://nifti.nimh.nih.gov/nifti-1>.
//!
//! A NIfTI file places its voxels in a world whose x runs to the patient's right, y to the
//! front and z up. Voxa's patient space is DICOM's: x to the left, y to the back, z up. So x
//! and y change sign on the way in, and nothing else does.

use std::io::Read;

use flate2::read::MultiGzDecoder;
use voxa_core::{Affine, Patient, Voxel};

/// The most voxels a label volume may have: enough for 512 x 512 voxels in 1024 slices.
pub const MAX_LABEL_VOXELS: u64 = 256 << 20;
/// How long the header is, and where a single file's values begin at the earliest.
const HEADER: usize = 348;
const FIRST_VALUE: u64 = 352;
/// The furthest into the file the values may begin: past any notes kept after the header.
const LAST_START: u64 = 64 << 20;
/// How many bytes of values are read and turned into numbers at a time.
const PIECE: usize = 1 << 20;

/// A label volume as its file holds it: a number for every voxel, columns fastest, and where
/// the voxels lie in the patient.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelGrid {
    /// Columns, rows and slices.
    pub dims: [u32; 3],
    /// The number at each voxel, in the order stored.
    pub ids: Vec<u32>,
    /// Where each voxel lies in the patient.
    pub placement: Affine<Voxel, Patient>,
}

/// Why a file cannot be read as labels. Each says so in words a person can act on.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NiftiError {
    /// It is not a NIfTI file at all.
    #[error("This is not a NIfTI file, so it cannot be opened as labels.")]
    NotNifti,
    /// It is the second version of the format.
    #[error("This is a NIfTI-2 file. Voxa reads the first version of NIfTI.")]
    Nifti2,
    /// Its values are in a second file.
    #[error("This NIfTI file keeps its values in a second file. Voxa reads the single-file kind.")]
    TwoFiles,
    /// It holds more than one volume.
    #[error("This file holds more than one volume, so it is not one set of labels.")]
    NotThreeD,
    /// Its values are of a kind labels are not stored as; the number is the format's own.
    #[error("The values in this file are of a kind labels are not stored as (NIfTI kind {0}).")]
    Kind(i16),
    /// Its values are to be scaled, which labels never are.
    #[error(
        "This file says its values are to be scaled, as a scan's are. Labels are plain numbers."
    )]
    Scaled,
    /// It holds a value that is not a whole number from 0 up.
    #[error("This file holds values that are not whole numbers from 0 up, so they are not labels.")]
    NotWhole,
    /// It does not say where its voxels lie.
    #[error(
        "This file does not say where its voxels lie in the patient, so they cannot be placed."
    )]
    NoPlace,
    /// It has more voxels than Voxa holds as labels.
    #[error("This file has more voxels than Voxa will hold as labels.")]
    TooLarge,
    /// It ends before its values do.
    #[error("This file ends before its values do, or is damaged.")]
    CutShort,
}

/// Read a label volume from the bytes of a `.nii` or `.nii.gz` file. Which of the two it is
/// is told from its first bytes, not from its name. No more is unpacked than the header says
/// the volume holds.
///
/// # Errors
///
/// A [`NiftiError`] for anything that is not one placed volume of whole numbers.
pub fn read_labels(bytes: &[u8]) -> Result<LabelGrid, NiftiError> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        read(&mut MultiGzDecoder::new(bytes))
    } else {
        let mut plain = bytes;
        read(&mut plain)
    }
}

fn read(file: &mut impl Read) -> Result<LabelGrid, NiftiError> {
    let mut header = [0_u8; HEADER];
    file.read_exact(&mut header)
        .map_err(|_| NiftiError::NotNifti)?;
    let fields = Fields::of(&header)?;
    let (dims, voxels) = fields.dims()?;
    let kind = Kind::of(fields.i16_at(70))?;
    if !plain_numbers(fields.f32_at(112), fields.f32_at(116)) {
        return Err(NiftiError::Scaled);
    }
    let placement = fields.placement().ok_or(NiftiError::NoPlace)?;
    if voxels > MAX_LABEL_VOXELS {
        return Err(NiftiError::TooLarge);
    }
    // The values begin a little after the header; what lies between is passed over.
    let start = f64::from(fields.f32_at(108));
    let start = if start.is_finite() { start } else { 0.0 };
    let between = whole(start.clamp(FIRST_VALUE as f64, LAST_START as f64)) - HEADER as u64;
    let passed = std::io::copy(&mut file.by_ref().take(between), &mut std::io::sink());
    if passed.ok() != Some(between) {
        return Err(NiftiError::CutShort);
    }
    let ids = values(file, kind, fields.big_endian, voxels)?;
    Ok(LabelGrid {
        dims,
        ids,
        placement,
    })
}

/// Whether a header's slope and intercept leave the values as they are: a slope of 1, or of
/// 0, which the format reads as "none", and no intercept.
#[expect(
    clippy::float_cmp,
    reason = "a header that does not scale says exactly 0 or 1, and exactly 0"
)]
fn plain_numbers(slope: f32, shift: f32) -> bool {
    (slope == 0.0 || slope == 1.0) && shift == 0.0
}

/// A count worked out as a real number, as a whole one.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between two small whole numbers before it comes here"
)]
fn whole(count: f64) -> u64 {
    count as u64
}

/// Read the number at every voxel, a piece of the file at a time.
fn values(
    file: &mut impl Read,
    kind: Kind,
    big_endian: bool,
    voxels: u64,
) -> Result<Vec<u32>, NiftiError> {
    let voxels = usize::try_from(voxels).map_err(|_| NiftiError::TooLarge)?;
    let mut ids = Vec::with_capacity(voxels);
    let mut piece = vec![0_u8; PIECE - PIECE % kind.bytes()];
    while ids.len() < voxels {
        let wanted = ((voxels - ids.len()) * kind.bytes()).min(piece.len());
        let part = piece.get_mut(..wanted).ok_or(NiftiError::CutShort)?;
        file.read_exact(part).map_err(|_| NiftiError::CutShort)?;
        for stored in part.chunks_exact(kind.bytes()) {
            ids.push(
                kind.number(stored, big_endian)
                    .ok_or(NiftiError::NotWhole)?,
            );
        }
    }
    Ok(ids)
}

/// The kinds of number a label volume is stored as.
#[derive(Debug, Clone, Copy)]
enum Kind {
    U8,
    I8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Kind {
    /// The kind a NIfTI datatype code stands for, if labels are ever stored that way.
    fn of(code: i16) -> Result<Self, NiftiError> {
        Ok(match code {
            2 => Self::U8,
            256 => Self::I8,
            4 => Self::I16,
            512 => Self::U16,
            8 => Self::I32,
            768 => Self::U32,
            16 => Self::F32,
            64 => Self::F64,
            other => return Err(NiftiError::Kind(other)),
        })
    }

    fn bytes(self) -> usize {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    /// One stored number as a label: a whole number from 0 up. `None` when it is not one.
    fn number(self, stored: &[u8], big_endian: bool) -> Option<u32> {
        // The bytes in the order this machine's numbers are read from: least first.
        let mut least_first = [0_u8; 8];
        let ordered = least_first.get_mut(..stored.len())?;
        ordered.copy_from_slice(stored);
        if big_endian {
            ordered.reverse();
        }
        let [a, b, c, d, ..] = least_first;
        match self {
            Self::U8 => Some(u32::from(a)),
            Self::I8 => u32::try_from(i8::from_le_bytes([a])).ok(),
            Self::I16 => u32::try_from(i16::from_le_bytes([a, b])).ok(),
            Self::U16 => Some(u32::from(u16::from_le_bytes([a, b]))),
            Self::I32 => u32::try_from(i32::from_le_bytes([a, b, c, d])).ok(),
            Self::U32 => Some(u32::from_le_bytes([a, b, c, d])),
            Self::F32 => counted(f64::from(f32::from_le_bytes([a, b, c, d]))),
            Self::F64 => counted(f64::from_le_bytes(least_first)),
        }
    }
}

/// A real number as a label, if it is a whole number from 0 up that a label can be.
fn counted(value: f64) -> Option<u32> {
    let fits = value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to be a whole number within range"
    )]
    fits.then_some(value as u32)
}

/// The header of a file, read in whichever byte order it was written.
struct Fields<'a> {
    header: &'a [u8; HEADER],
    big_endian: bool,
}

impl<'a> Fields<'a> {
    /// Work out the byte order from the header's own length, which is always written first,
    /// and check that this is a single-file NIfTI-1.
    fn of(header: &'a [u8; HEADER]) -> Result<Self, NiftiError> {
        let [a, b, c, d, ..] = *header;
        let lengths = [
            i32::from_le_bytes([a, b, c, d]),
            i32::from_be_bytes([a, b, c, d]),
        ];
        if lengths.contains(&540) {
            return Err(NiftiError::Nifti2);
        }
        let [little, big] = lengths.map(|length| length == 348);
        if !(little || big) {
            return Err(NiftiError::NotNifti);
        }
        match header.get(344..348) {
            Some(b"n+1\0") => Ok(Self {
                header,
                big_endian: big,
            }),
            Some(b"ni1\0") => Err(NiftiError::TwoFiles),
            _ => Err(NiftiError::NotNifti),
        }
    }

    fn bytes<const N: usize>(&self, at: usize) -> [u8; N] {
        let mut read = [0_u8; N];
        if let Some(stored) = self.header.get(at..at + N) {
            read.copy_from_slice(stored);
        }
        if self.big_endian {
            read.reverse();
        }
        read
    }

    fn i16_at(&self, at: usize) -> i16 {
        i16::from_le_bytes(self.bytes(at))
    }

    fn f32_at(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.bytes(at))
    }

    /// Four numbers one after another, as real numbers.
    fn row(&self, at: usize) -> [f64; 4] {
        [0, 4, 8, 12].map(|step| f64::from(self.f32_at(at + step)))
    }

    /// Columns, rows and slices, and how many voxels that is. A file with more than three
    /// dimensions is one volume only if the others are all 1.
    fn dims(&self) -> Result<([u32; 3], u64), NiftiError> {
        let count = usize::try_from(self.i16_at(40)).unwrap_or(0);
        if !(1..=7).contains(&count) {
            return Err(NiftiError::NotNifti);
        }
        let mut sizes = [1_u32; 7];
        for (place, size) in sizes.iter_mut().enumerate().take(count) {
            let stored = u32::try_from(self.i16_at(42 + 2 * place)).ok();
            *size = stored
                .filter(|size| *size > 0)
                .ok_or(NiftiError::NotNifti)?;
        }
        let [cols, rows, slices, rest @ ..] = sizes;
        if rest.iter().any(|size| *size != 1) {
            return Err(NiftiError::NotThreeD);
        }
        let voxels = u64::from(cols) * u64::from(rows) * u64::from(slices);
        Ok(([cols, rows, slices], voxels))
    }

    /// Where the voxels lie in the patient: by the sform if the file has one, otherwise by
    /// the qform. `None` when it has neither, or the one it has cannot be turned round.
    fn placement(&self) -> Option<Affine<Voxel, Patient>> {
        let rows = if self.i16_at(254) > 0 {
            [self.row(280), self.row(296), self.row(312)]
        } else if self.i16_at(252) > 0 {
            self.by_quaternion()
        } else {
            return None;
        };
        let [[xx, xy, xz, x0], [yx, yy, yz, y0], [zx, zy, zz, z0]] = rows;
        // Right-anterior-superior to left-posterior-superior: x and y change sign.
        let placement = Affine::from_columns(
            [-xx, -yx, zx],
            [-xy, -yy, zy],
            [-xz, -yz, zz],
            [-x0, -y0, z0],
        );
        let finite = rows.iter().flatten().all(|part| part.is_finite());
        placement.inverse().filter(|_| finite).map(|_| placement)
    }

    /// The three rows of the placement a qform stands for: a turn given as a quaternion, the
    /// size of a voxel each way, and where the first voxel is. NIfTI-1, "method 2".
    fn by_quaternion(&self) -> [[f64; 4]; 3] {
        let [mut b, mut c, mut d, x0] = self.row(256);
        let [y0, z0, ..] = self.row(272);
        let [flip, wide, high, deep] = self.row(76);
        let squares = b * b + c * c + d * d;
        // The fourth part of the quaternion is whatever makes its length 1.
        let a = if squares < 1.0 {
            (1.0 - squares).sqrt()
        } else {
            let length = squares.sqrt();
            (b, c, d) = (b / length, c / length, d / length);
            0.0
        };
        let deep = if flip < 0.0 { -deep } else { deep };
        let turn = [
            [
                a * a + b * b - c * c - d * d,
                2.0 * (b * c - a * d),
                2.0 * (b * d + a * c),
            ],
            [
                2.0 * (b * c + a * d),
                a * a + c * c - b * b - d * d,
                2.0 * (c * d - a * b),
            ],
            [
                2.0 * (b * d - a * c),
                2.0 * (c * d + a * b),
                a * a + d * d - c * c - b * b,
            ],
        ];
        let sized = |[x, y, z]: [f64; 3], first: f64| [x * wide, y * high, z * deep, first];
        let [across, down, through] = turn;
        [sized(across, x0), sized(down, y0), sized(through, z0)]
    }
}

#[cfg(test)]
mod tests;
