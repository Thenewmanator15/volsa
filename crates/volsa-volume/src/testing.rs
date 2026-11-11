//! Volumes for this crate's tests.

use voxa_core::{Affine, Volume};

/// A volume of `dims` (columns, rows, slices) with 1 mm voxels, its first voxel's centre at
/// the patient origin, each value given by its column, row and slice.
pub(crate) fn filled(dims: [u16; 3], value: impl Fn(u16, u16, u16) -> f32) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            for col in 0..cols {
                values.push(value(col, row, slice));
            }
        }
    }
    Volume::new(dims.map(u32::from), values, Affine::identity()).expect("adds up")
}

/// 21 voxels a side, all zero but for a ball of value 100: every voxel within 3 of the one at
/// (10, 10, 10).
pub(crate) fn ball() -> Volume {
    filled([21, 21, 21], |col, row, slice| {
        let from_centre = |at: u16| (i32::from(at) - 10).pow(2);
        if from_centre(col) + from_centre(row) + from_centre(slice) <= 9 {
            100.0
        } else {
            0.0
        }
    })
}
