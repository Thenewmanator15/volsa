//! What sampling, lines of sight and regions read of a volume, whichever way it holds its
//! values: as 32-bit numbers ([`crate::Volume`]) or as stored ([`crate::HeldVolume`]).

use crate::space::{Affine, Patient, Point, Voxel};

/// A grid of values with a place in the patient.
pub trait Grid: Sync {
    /// Columns, rows and slices.
    fn dims(&self) -> [u32; 3];

    /// Where the centre of each voxel lies in the patient.
    fn voxel_to_patient(&self) -> Affine<Voxel, Patient>;

    /// Which voxel a point in the patient falls in.
    fn patient_to_voxel(&self) -> Affine<Patient, Voxel>;

    /// The value of one voxel; `None` outside the grid, or where it holds no value.
    fn value(&self, col: u32, row: u32, slice: u32) -> Option<f32>;

    /// The eight outer corners of the grid in the patient: half a voxel outside the centres
    /// of the corner voxels. Corner `i` is at the far end of the columns when `i & 1` is set,
    /// of the rows when `i & 2` is, and of the slices when `i & 4` is.
    fn corners(&self) -> [Point<Patient>; 8] {
        corners_of(self.dims(), self.voxel_to_patient())
    }

    /// The distance between neighbouring voxel centres along columns, rows and slices, in
    /// millimetres.
    fn spacing_mm(&self) -> [f64; 3] {
        spacing_of(self.voxel_to_patient())
    }
}

/// The corners of a grid of `dims` placed by `to_patient`, as [`Grid::corners`] gives them.
pub(crate) fn corners_of(
    dims: [u32; 3],
    to_patient: Affine<Voxel, Patient>,
) -> [Point<Patient>; 8] {
    let [cols, rows, slices] = dims.map(f64::from);
    let end = |far: bool, count: f64| if far { count - 0.5 } else { -0.5 };
    std::array::from_fn(|i| {
        let voxel = Point::new(
            end(i & 1 != 0, cols),
            end(i & 2 != 0, rows),
            end(i & 4 != 0, slices),
        );
        to_patient.apply(voxel)
    })
}

/// The spacing of a grid placed by `to_patient`, as [`Grid::spacing_mm`] gives it.
pub(crate) fn spacing_of(to_patient: Affine<Voxel, Patient>) -> [f64; 3] {
    to_patient
        .columns()
        .map(|[x, y, z]| (x * x + y * y + z * z).sqrt())
}
