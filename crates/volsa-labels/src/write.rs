//! Writing a label file and its table of names: the two files this crate reads, so that what
//! Voxa makes can be kept and opened again, here or in another program.
//!
//! The label file is a gzipped single-file NIfTI-1 with an sform; the table is the shape the
//! labelling pipeline writes, with a `voxa` object that says who made the labels.
//!
//! A NIfTI file places its voxels in a world whose x runs to the patient's right and y to the
//! front. Voxa's patient space is DICOM's, with x to the left and y to the back. Reading
//! changes the sign of both on the way in, and writing changes them back.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use voxa_core::{Affine, Patient, Voxel};

use crate::nifti::{LabelGrid, MAX_LABEL_VOXELS};

/// The header of a file with the four bytes after it that say it has no extensions: where
/// the values begin.
const HEADER: usize = 352;
/// How many numbers are turned into bytes at a time.
const PIECE: usize = 1 << 18;
/// The NIfTI code for a position in the scanner's own space.
const SCANNER_SPACE: i16 = 1;
/// The NIfTI code for lengths in millimetres.
const MILLIMETRES: u8 = 2;

/// Why labels or a table cannot be written. Each says so in words a person can read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// More than a label file, or a table, can hold.
    #[error(
        "These labels are too big to be written as a file: a side is too long, there are too \
         many voxels, a number is too large, or the table has too many names or is too long."
    )]
    TooLarge,
    /// A name given twice on one side.
    #[error("The table names one part twice on the same side, so it cannot be written.")]
    NameTwice,
    /// A table that names nothing.
    #[error("The table names no part, so there is nothing to write.")]
    NoNames,
    /// Not a whole volume, or not placed.
    #[error(
        "These labels are not a whole volume of voxels with a place in the patient, so they \
         cannot be written as a file."
    )]
    BadGrid,
}

/// Write a label volume as the bytes of a `.nii.gz` file: a single-file NIfTI-1 with an sform
/// that holds the placement, and a number for every voxel in the order stored, columns
/// fastest. The numbers are unsigned 16-bit when every one fits, and signed 32-bit otherwise.
/// A qform is not written.
///
/// What is written is read back as it was by [`read_labels`](crate::read_labels), with its
/// placement to within a ten-thousandth of a millimetre: the header holds it in 32-bit numbers.
///
/// # Errors
///
/// [`WriteError::TooLarge`] for a volume the reader would refuse for its size: more than
/// [`MAX_LABEL_VOXELS`] voxels, a side past 32 767, or a number past 2 147 483 647.
/// [`WriteError::BadGrid`] for a side of nothing, a number of voxels that is not the volume's
/// size, or a placement that cannot be turned round.
pub fn write_labels(grid: &LabelGrid) -> Result<Vec<u8>, WriteError> {
    // A header gives each side as a signed 16-bit number.
    let across = grid.dims.map(|side| i16::try_from(side).ok());
    let [Some(columns), Some(rows), Some(slices)] = across else {
        return Err(WriteError::TooLarge);
    };
    let voxels: u64 = grid.dims.iter().map(|side| u64::from(*side)).product();
    if voxels > MAX_LABEL_VOXELS {
        return Err(WriteError::TooLarge);
    }
    if voxels == 0 || usize::try_from(voxels) != Ok(grid.ids.len()) {
        return Err(WriteError::BadGrid);
    }
    let largest = grid.ids.iter().copied().max().unwrap_or(0);
    if i32::try_from(largest).is_err() {
        return Err(WriteError::TooLarge);
    }
    let placed = Placed::of(&grid.placement).ok_or(WriteError::BadGrid)?;
    let wide = largest > u32::from(u16::MAX);
    let header = header([columns, rows, slices], wide, &placed);
    pack(&header, &grid.ids, wide).ok_or(WriteError::TooLarge)
}

/// Where the voxels lie, as a header holds it.
struct Placed {
    /// The three rows of the sform.
    rows: [[f32; 4]; 3],
    /// The size of a voxel along each side.
    steps: [f32; 3],
}

impl Placed {
    /// The placement as the file's world has it, or `None` when what is written would not be
    /// a placement the reader can turn round.
    fn of(placement: &Affine<Voxel, Patient>) -> Option<Self> {
        let columns = placement.columns();
        let [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] = columns;
        let [ox, oy, oz] = placement.origin();
        // The reader's change of sign, undone: x and y turn round.
        let rows = [[-ax, -bx, -cx, -ox], [-ay, -by, -cy, -oy], [az, bz, cz, oz]]
            .map(|row| row.map(single));
        // The placement the reader will make of these rows.
        let [[xa, xb, xc, xo], [ya, yb, yc, yo], [za, zb, zc, zo]] =
            rows.map(|row| row.map(f64::from));
        let stored = Affine::<Voxel, Patient>::from_columns(
            [-xa, -ya, za],
            [-xb, -yb, zb],
            [-xc, -yc, zc],
            [-xo, -yo, zo],
        );
        let finite = rows.iter().flatten().all(|part| part.is_finite());
        let size = |[dx, dy, dz]: [f64; 3]| single((dx * dx + dy * dy + dz * dz).sqrt());
        (finite && stored.inverse().is_some()).then(|| Self {
            rows,
            steps: columns.map(size),
        })
    }
}

/// A real number as the 32-bit one a header holds: infinite if it is too large for one.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a header holds 32-bit numbers, and a number too large for one is refused after"
)]
fn single(value: f64) -> f32 {
    value as f32
}

/// The 352 bytes before the values: the header and the four bytes that follow it.
fn header(sides: [i16; 3], wide: bool, placed: &Placed) -> [u8; HEADER] {
    let mut header = [0_u8; HEADER];
    let mut put = |at: usize, bytes: &[u8]| {
        if let Some(slot) = header.get_mut(at..at + bytes.len()) {
            slot.copy_from_slice(bytes);
        }
    };
    put(0, &348_i32.to_le_bytes());
    let [columns, rows, slices] = sides;
    for (place, side) in [3, columns, rows, slices, 1, 1, 1, 1]
        .into_iter()
        .enumerate()
    {
        put(40 + 2 * place, &side.to_le_bytes());
    }
    let (kind, bits): (i16, i16) = if wide { (8, 32) } else { (512, 16) };
    put(70, &kind.to_le_bytes());
    put(72, &bits.to_le_bytes());
    let [x, y, z] = placed.steps;
    for (place, step) in [1.0, x, y, z, 0.0, 0.0, 0.0, 0.0].into_iter().enumerate() {
        put(76 + 4 * place, &f32::to_le_bytes(step));
    }
    put(108, &352.0_f32.to_le_bytes());
    put(112, &1.0_f32.to_le_bytes());
    put(123, &[MILLIMETRES]);
    put(254, &SCANNER_SPACE.to_le_bytes());
    for (place, part) in placed.rows.iter().flatten().enumerate() {
        put(280 + 4 * place, &part.to_le_bytes());
    }
    put(344, b"n+1\0");
    header
}

/// The header and the numbers, gzipped. `None` only if writing into memory fails, which it
/// does for want of room.
fn pack(header: &[u8], ids: &[u32], wide: bool) -> Option<Vec<u8>> {
    let width = if wide { 4 } else { 2 };
    let mut packer = GzEncoder::new(Vec::new(), Compression::default());
    packer.write_all(header).ok()?;
    let mut piece = Vec::with_capacity(PIECE * width);
    for numbers in ids.chunks(PIECE) {
        piece.clear();
        for number in numbers {
            // A number that fits in 16 bits is the first two of its four bytes.
            let bytes = number.to_le_bytes();
            piece.extend_from_slice(bytes.get(..width).unwrap_or_default());
        }
        packer.write_all(&piece).ok()?;
    }
    packer.finish().ok()
}

mod table;
pub use table::write_table;

#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod voxa_tests;
