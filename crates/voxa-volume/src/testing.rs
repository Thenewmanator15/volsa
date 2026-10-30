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

/// The same values as `volume`, held as 32-bit numbers in a held volume of slabs of three
/// slices, as the application holds a series it cannot keep in 16 bits.
pub(crate) fn held_from(volume: &Volume) -> voxa_core::HeldVolume {
    use voxa_core::{HeldVolume, Rescale, SliceIn, Stored};
    let [cols, rows, slices] = volume.dims();
    let slab = 3 * cols as usize * rows as usize * 4;
    let held = HeldVolume::new(
        volume.dims(),
        volume.voxel_to_patient(),
        Stored::Float32,
        Rescale::default(),
        slab,
    )
    .expect("the same placement");
    for slice in 0..slices {
        let values = volume.slice_values(slice).expect("a slice");
        held.put_slice(slice, SliceIn::Float(values)).expect("fits");
    }
    held
}
