//! A series as one block of values with a place in the patient.

use crate::space::{Affine, Patient, Point, Voxel};

/// The values of a whole series, slice upon slice, with the transform that places each voxel
/// in the patient. A volume always has at least one voxel, exactly as many values as its
/// dimensions say, and a transform that can be undone.
#[derive(Clone, PartialEq)]
pub struct Volume {
    dims: [u32; 3],
    values: Vec<f32>,
    voxel_to_patient: Affine<Voxel, Patient>,
    patient_to_voxel: Affine<Patient, Voxel>,
}

impl Volume {
    /// A volume of `dims` (columns, rows, slices) holding `values`, the value at column `c`,
    /// row `r`, slice `s` being `values[(s * rows + r) * cols + c]`. `None` when a dimension
    /// is zero, the number of values is not the product of the dimensions, or the transform
    /// cannot be undone.
    pub fn new(
        dims: [u32; 3],
        values: Vec<f32>,
        voxel_to_patient: Affine<Voxel, Patient>,
    ) -> Option<Self> {
        let [cols, rows, slices] = dims.map(u64::from);
        let count = cols.checked_mul(rows)?.checked_mul(slices)?;
        if count == 0 || values.len() as u64 != count {
            return None;
        }
        let patient_to_voxel = voxel_to_patient.inverse()?;
        Some(Self {
            dims,
            values,
            voxel_to_patient,
            patient_to_voxel,
        })
    }

    /// Columns, rows and slices.
    pub fn dims(&self) -> [u32; 3] {
        self.dims
    }

    /// Every value, columns fastest, then rows, then slices.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Where the centre of each voxel lies in the patient.
    pub fn voxel_to_patient(&self) -> Affine<Voxel, Patient> {
        self.voxel_to_patient
    }

    /// Which voxel a point in the patient falls in.
    pub fn patient_to_voxel(&self) -> Affine<Patient, Voxel> {
        self.patient_to_voxel
    }

    /// The value of one voxel; `None` outside the volume.
    pub fn get(&self, col: u32, row: u32, slice: u32) -> Option<f32> {
        let [cols, rows, slices] = self.dims;
        if col >= cols || row >= rows || slice >= slices {
            return None;
        }
        let at = (slice as usize * rows as usize + row as usize) * cols as usize + col as usize;
        self.values.get(at).copied()
    }

    /// The eight outer corners of the volume in the patient: half a voxel outside the centres
    /// of the corner voxels. Corner `i` is at the far end of the columns when `i & 1` is set,
    /// of the rows when `i & 2` is, and of the slices when `i & 4` is.
    pub fn corners(&self) -> [Point<Patient>; 8] {
        let [cols, rows, slices] = self.dims.map(f64::from);
        let end = |far: bool, count: f64| if far { count - 0.5 } else { -0.5 };
        std::array::from_fn(|i| {
            let voxel = Point::new(
                end(i & 1 != 0, cols),
                end(i & 2 != 0, rows),
                end(i & 4 != 0, slices),
            );
            self.voxel_to_patient.apply(voxel)
        })
    }

    /// The middle of the volume in the patient.
    pub fn centre(&self) -> Point<Patient> {
        let [cols, rows, slices] = self.dims.map(|count| (f64::from(count) - 1.0) / 2.0);
        self.voxel_to_patient.apply(Point::new(cols, rows, slices))
    }

    /// The distance between neighbouring voxel centres along columns, rows and slices, in
    /// millimetres.
    pub fn spacing_mm(&self) -> [f64; 3] {
        self.voxel_to_patient
            .columns()
            .map(|[x, y, z]| (x * x + y * y + z * z).sqrt())
    }
}

impl std::fmt::Debug for Volume {
    /// Prints the size and placement, not the values: there are millions of them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Volume")
            .field("dims", &self.dims)
            .field("voxel_to_patient", &self.voxel_to_patient)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted() -> Volume {
        let values = (0..12u8).map(f32::from).collect();
        Volume::new([2, 2, 3], values, Affine::identity()).expect("adds up")
    }

    /// Voxels half a millimetre across in slices 7.5 mm apart, away from the origin.
    fn placed() -> Volume {
        let to_patient = Affine::from_columns(
            [0.5, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [0.0, 0.0, 7.5],
            [-100.0, -130.0, 10.0],
        );
        Volume::new([2, 2, 3], vec![0.0; 12], to_patient).expect("adds up")
    }

    #[test]
    fn a_value_is_found_by_its_column_row_and_slice() {
        let volume = counted();
        assert_eq!(volume.dims(), [2, 2, 3]);
        assert_eq!(volume.values().len(), 12);
        assert_eq!(volume.get(1, 0, 2), Some(9.0));
        assert_eq!(volume.get(0, 1, 0), Some(2.0));
        assert_eq!(volume.get(2, 0, 0), None);
        assert_eq!(volume.get(0, 2, 0), None);
        assert_eq!(volume.get(0, 0, 3), None);
    }

    #[test]
    fn a_volume_that_does_not_add_up_is_refused() {
        let identity = Affine::identity();
        assert!(Volume::new([0, 2, 3], Vec::new(), identity).is_none());
        assert!(Volume::new([2, 2, 3], vec![0.0; 11], identity).is_none());
        let flat = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0; 3], [0.0; 3]);
        assert!(
            Volume::new([2, 2, 3], vec![0.0; 12], flat).is_none(),
            "cannot be undone"
        );
        assert!(
            Volume::new([u32::MAX; 3], Vec::new(), identity).is_none(),
            "too many to count"
        );
    }

    #[test]
    fn spacing_corners_and_centre_come_from_the_transform() {
        let volume = placed();
        assert_eq!(volume.spacing_mm(), [0.5, 0.5, 7.5]);
        let corners = volume.corners();
        let at = |p: Point<Patient>| (p.x, p.y, p.z);
        assert_eq!(at(corners[0]), (-100.25, -130.25, 6.25));
        assert_eq!(
            at(corners[1]),
            (-99.25, -130.25, 6.25),
            "the far end of the columns"
        );
        assert_eq!(at(corners[7]), (-99.25, -129.25, 28.75));
        assert_eq!(at(volume.centre()), (-99.75, -129.75, 17.5));
    }

    #[test]
    fn the_two_transforms_undo_each_other() {
        let volume = placed();
        let there = volume.voxel_to_patient().apply(Point::new(1.0, 1.0, 2.0));
        let back = volume.patient_to_voxel().apply(there);
        let off = (back.x - 1.0).abs() + (back.y - 1.0).abs() + (back.z - 2.0).abs();
        assert!(off < 1e-9, "{back:?}");
    }

    #[test]
    fn printing_a_volume_does_not_print_its_values() {
        let printed = format!("{:?}", counted());
        assert!(printed.contains("dims: [2, 2, 3]"), "{printed}");
        assert!(printed.len() < 400, "{} characters", printed.len());
    }
}
