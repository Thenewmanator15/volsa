//! Volumes made up for tests, held as the store holds a series' volume.

/// The same values as `volume`, held as 32-bit numbers, as the store holds a series.
pub fn held_of(volume: &voxa_core::Volume) -> voxa_core::HeldVolume {
    use voxa_core::{HeldVolume, Rescale, SliceIn, Stored};
    let [_, _, slices] = volume.dims();
    let held = HeldVolume::new(
        volume.dims(),
        volume.voxel_to_patient(),
        Stored::Float32,
        Rescale::default(),
        1 << 20,
    )
    .expect("the same placement");
    for slice in 0..slices {
        let values = volume.slice_values(slice).expect("a slice");
        held.put_slice(slice, SliceIn::Float(values)).expect("fits");
    }
    held
}

/// A cube `side` voxels of 1 mm each way, every voxel `value`, held as a series' volume is.
pub fn cube(side: u32, value: f32) -> std::sync::Arc<voxa_core::HeldVolume> {
    let values = vec![value; (side * side * side) as usize];
    let placement = voxa_core::Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0; 3],
    );
    let volume = voxa_core::Volume::new([side; 3], values, placement).expect("adds up");
    std::sync::Arc::new(held_of(&volume))
}
