//! Builders of made-up label files, for tests. Nothing here reads a file.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;

/// A placement given the second way a NIfTI file can give one: a turn, sizes and a start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quatern {
    /// The quaternion's b, c and d.
    pub turn: [f32; 3],
    /// Where the first voxel is, in the file's world.
    pub first: [f32; 3],
    /// The size of a voxel each way, in millimetres.
    pub sizes: [f32; 3],
    /// 1, or -1 for a third axis that runs the other way.
    pub flip: f32,
}

/// What a made-up NIfTI-1 label file holds.
#[derive(Debug, Clone, PartialEq)]
pub struct NiftiOptions {
    /// Columns, rows and slices.
    pub dims: [u16; 3],
    /// The number at each voxel, columns fastest.
    pub ids: Vec<u32>,
    /// The NIfTI code of the kind of number they are stored as: 2, 256, 4, 512, 8, 768, 16
    /// or 64. Any other code is written into the header with the values stored as for 512.
    pub kind: i16,
    /// The three rows of the sform, if the file is to have one.
    pub srow: Option<[[f32; 4]; 3]>,
    /// The qform, if the file is to have one.
    pub quatern: Option<Quatern>,
    /// Whether the file is written most significant byte first.
    pub big_endian: bool,
    /// What the header says the values are to be multiplied by; 1 or 0 for labels.
    pub slope: f32,
}

/// The bytes of a single-file NIfTI-1 holding these labels, not gzipped.
pub fn nifti(options: &NiftiOptions) -> Vec<u8> {
    let big = options.big_endian;
    let short = |value: i16| ordered(value.to_le_bytes(), big);
    let real = |value: f32| ordered(value.to_le_bytes(), big);
    let mut file = vec![0_u8; 352];
    let mut put = |at: usize, bytes: &[u8]| {
        for (slot, byte) in file.iter_mut().skip(at).zip(bytes) {
            *slot = *byte;
        }
    };
    put(0, &ordered(348_i32.to_le_bytes(), big));
    let [cols, rows, slices] = options
        .dims
        .map(|size| i16::try_from(size).unwrap_or(i16::MAX));
    for (place, size) in [3, cols, rows, slices, 1, 1, 1, 1].into_iter().enumerate() {
        put(40 + 2 * place, &short(size));
    }
    put(70, &short(options.kind));
    put(
        72,
        &short(i16::try_from(8 * width(options.kind)).unwrap_or(16)),
    );
    put(108, &real(352.0));
    put(112, &real(options.slope));
    put(76, &real(1.0));
    if let Some(Quatern {
        turn,
        first,
        sizes,
        flip,
    }) = options.quatern
    {
        put(252, &short(1));
        put(76, &real(flip));
        for (place, part) in sizes.into_iter().enumerate() {
            put(80 + 4 * place, &real(part));
        }
        for (place, part) in turn.into_iter().chain(first).enumerate() {
            put(256 + 4 * place, &real(part));
        }
    }
    if let Some(srow) = options.srow {
        put(254, &short(1));
        for (place, part) in srow.into_iter().flatten().enumerate() {
            put(280 + 4 * place, &real(part));
        }
    }
    put(344, b"n+1\0");
    for id in &options.ids {
        file.extend(stored(*id, options.kind, big));
    }
    file
}

/// The same bytes as a `.gz` file holds them.
pub fn gzipped(bytes: &[u8]) -> Vec<u8> {
    let mut packer = GzEncoder::new(Vec::new(), Compression::fast());
    // Writing into memory does not fail.
    let _ = packer.write_all(bytes);
    packer.finish().unwrap_or_default()
}

/// How many bytes one number of a kind takes.
fn width(kind: i16) -> usize {
    match kind {
        2 | 256 => 1,
        8 | 768 | 16 => 4,
        64 => 8,
        _ => 2,
    }
}

/// One label as its kind stores it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "a test file holds small numbers, and may be asked to hold them in a narrow kind"
)]
fn stored(id: u32, kind: i16, big: bool) -> Vec<u8> {
    match kind {
        2 => vec![id as u8],
        256 => vec![(id as i8).to_le_bytes()[0]],
        4 => ordered((id as i16).to_le_bytes(), big).to_vec(),
        8 => ordered((id as i32).to_le_bytes(), big).to_vec(),
        768 => ordered(id.to_le_bytes(), big).to_vec(),
        16 => ordered((id as f32).to_le_bytes(), big).to_vec(),
        64 => ordered(f64::from(id).to_le_bytes(), big).to_vec(),
        _ => ordered((id as u16).to_le_bytes(), big).to_vec(),
    }
}

/// Bytes written least significant first, turned round if the file is the other way.
fn ordered<const N: usize>(mut bytes: [u8; N], big: bool) -> [u8; N] {
    if big {
        bytes.reverse();
    }
    bytes
}
