//! Where the slices of a stack lie: their order along the slice normal, the gaps between them,
//! and the transform from the image grid to the patient.

use volsa_core::{Affine, Patient, Voxel};

/// Slices closer together than this along the normal are at the same position.
pub(crate) const SAME_POSITION_MM: f64 = 0.001;
/// How far a direction's length may be from 1, or two directions from a right angle.
const DIRECTION_SLACK: f64 = 0.01;
/// Direction cosines closer than this are the same direction written with different rounding.
const SAME_DIRECTION: f64 = 0.001;

/// Order, spacing and placement of a stack of slices.
#[derive(Debug, Clone, PartialEq)]
pub struct StackGeometry {
    /// Indices into the positions given, from the first slice along the normal to the last.
    pub order: Vec<usize>,
    /// The typical distance between neighbouring slices: the median gap. 0.0 for one slice.
    pub step_mm: f64,
    /// The distance from each slice to the next, in that order; one fewer than the slices.
    pub steps_mm: Vec<f64>,
    /// Where the voxel at (column, row, slice) lies in the patient.
    pub voxel_to_patient: Affine<Voxel, Patient>,
}

/// Work out the geometry of a stack from each slice's `ImagePositionPatient`, the shared
/// `ImageOrientationPatient` (direction of increasing column, then of increasing row) and
/// `PixelSpacing` (distance between rows, then between columns).
///
/// Slices are ordered along the slice normal, which is the direction of increasing column
/// crossed with the direction of increasing row. Returns `None` when there are no positions.
pub fn stack_geometry(
    positions: &[[f64; 3]],
    orientation: [f64; 6],
    pixel_spacing: [f64; 2],
) -> Option<StackGeometry> {
    let [ax, ay, az, dx, dy, dz] = orientation;
    let across = [ax, ay, az];
    let down = [dx, dy, dz];
    let normal = cross(across, down);
    let along = |index: &usize| positions.get(*index).map_or(0.0, |p| dot(*p, normal));

    let mut order: Vec<usize> = (0..positions.len()).collect();
    order.sort_by(|a, b| along(a).total_cmp(&along(b)));
    let first = *positions.get(*order.first()?)?;
    let last = *positions.get(*order.last()?)?;
    let steps_mm: Vec<f64> = order
        .windows(2)
        .filter_map(|pair| match pair {
            [here, next] => Some(along(next) - along(here)),
            _ => None,
        })
        .collect();

    // The slice axis runs from the first slice to the last, so a tilted or sheared stack is
    // placed correctly. A stack with no extent (one slice, or all at one position) borrows the
    // normal at 1 mm so the transform can still be undone.
    let span = [last[0] - first[0], last[1] - first[1], last[2] - first[2]];
    let gaps = steps_mm.len() as f64;
    let slice_axis = if steps_mm.is_empty() || dot(span, span).sqrt() < SAME_POSITION_MM {
        normal
    } else {
        span.map(|v| v / gaps)
    };
    let [between_rows, between_cols] = pixel_spacing;
    Some(StackGeometry {
        order,
        step_mm: median(&steps_mm),
        steps_mm,
        voxel_to_patient: Affine::from_columns(
            across.map(|v| v * between_cols),
            down.map(|v| v * between_rows),
            slice_axis,
            first,
        ),
    })
}

/// The middle value, or the mean of the two middle values; 0.0 for no values.
pub(crate) fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let lower = sorted.len().saturating_sub(1) / 2;
    let upper = sorted.len() / 2;
    match (sorted.get(lower), sorted.get(upper)) {
        (Some(low), Some(high)) => (low + high) / 2.0,
        _ => 0.0,
    }
}

/// Whether an `ImageOrientationPatient` is two directions of unit length at right angles, give
/// or take the rounding scanners write them with. Zeros, or two directions along one line,
/// describe no image plane.
pub(crate) fn is_image_plane(orientation: [f64; 6]) -> bool {
    let [ax, ay, az, dx, dy, dz] = orientation;
    let across = [ax, ay, az];
    let down = [dx, dy, dz];
    (dot(across, across) - 1.0).abs() < DIRECTION_SLACK
        && (dot(down, down) - 1.0).abs() < DIRECTION_SLACK
        && dot(across, down).abs() < DIRECTION_SLACK
}

/// Whether two orientations face the same way, give or take rounding.
pub(crate) fn same_plane(a: [f64; 6], b: [f64; 6]) -> bool {
    a.iter()
        .zip(b)
        .all(|(here, there)| (here - there).abs() < SAME_DIRECTION)
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use volsa_core::Point;

    const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

    #[test]
    fn slices_are_ordered_along_the_normal_whatever_order_they_arrive_in() {
        let positions = [
            [-100.0, -120.0, 10.0],
            [-100.0, -120.0, 0.0],
            [-100.0, -120.0, 15.0],
            [-100.0, -120.0, 5.0],
        ];
        let g = stack_geometry(&positions, AXIAL, [0.9, 0.9]).expect("has positions");
        assert_eq!(g.order, vec![1, 3, 0, 2]);
        assert_eq!(g.step_mm, 5.0);
        assert_eq!(g.steps_mm, vec![5.0, 5.0, 5.0]);
        let p = g.voxel_to_patient.apply(Point::new(10.0, 20.0, 3.0));
        assert_eq!((p.x, p.y, p.z), (-91.0, -102.0, 15.0));
    }

    #[test]
    fn pixel_spacing_is_rows_then_columns() {
        // 2 mm between rows (down the image), 0.5 mm between columns (across it).
        let g = stack_geometry(&[[0.0, 0.0, 0.0], [0.0, 0.0, 3.0]], AXIAL, [2.0, 0.5])
            .expect("has positions");
        let across = g.voxel_to_patient.apply(Point::new(1.0, 0.0, 0.0));
        let down = g.voxel_to_patient.apply(Point::new(0.0, 1.0, 0.0));
        assert_eq!((across.x, across.y), (0.5, 0.0));
        assert_eq!((down.x, down.y), (0.0, 2.0));
    }

    #[test]
    fn a_tilted_stack_steps_along_its_own_slice_direction() {
        // Slices 4 mm apart along a normal tilted about the x axis, as with a tilted head.
        let orientation = [1.0, 0.0, 0.0, 0.0, 0.8, 0.6];
        let normal = [0.0, -0.6, 0.8];
        let positions: Vec<[f64; 3]> = (0..5)
            .map(|i| normal.map(|n| n * 4.0 * f64::from(i)))
            .collect();
        let g = stack_geometry(&positions, orientation, [1.0, 1.0]).expect("has positions");
        assert_eq!(g.order, vec![0, 1, 2, 3, 4]);
        assert!((g.step_mm - 4.0).abs() < 1e-9);
        let top = g.voxel_to_patient.apply(Point::new(0.0, 0.0, 4.0));
        assert!((top.y + 9.6).abs() < 1e-9 && (top.z - 12.8).abs() < 1e-9);
    }

    #[test]
    fn an_uneven_stack_reports_every_gap_and_the_median_as_its_step() {
        let positions = [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 5.0],
            [0.0, 0.0, 10.0],
            [0.0, 0.0, 20.0],
            [0.0, 0.0, 25.0],
        ];
        let g = stack_geometry(&positions, AXIAL, [1.0, 1.0]).expect("has positions");
        assert_eq!(g.steps_mm, vec![5.0, 5.0, 10.0, 5.0]);
        assert_eq!(g.step_mm, 5.0);
    }

    #[test]
    fn one_slice_has_no_step_but_still_has_a_transform() {
        let g = stack_geometry(&[[1.0, 2.0, 3.0]], AXIAL, [0.5, 0.5]).expect("has a position");
        assert_eq!(g.order, vec![0]);
        assert_eq!(g.step_mm, 0.0);
        assert_eq!(g.steps_mm, Vec::<f64>::new());
        assert!(
            g.voxel_to_patient.inverse().is_some(),
            "one slice still maps both ways"
        );
        let p = g.voxel_to_patient.apply(Point::new(2.0, 0.0, 0.0));
        assert_eq!((p.x, p.y, p.z), (2.0, 2.0, 3.0));
    }

    #[test]
    fn no_positions_means_no_geometry() {
        assert!(stack_geometry(&[], AXIAL, [1.0, 1.0]).is_none());
    }

    #[test]
    fn slices_at_one_position_keep_a_usable_transform() {
        let g = stack_geometry(&[[0.0, 0.0, 7.0], [0.0, 0.0, 7.0]], AXIAL, [1.0, 1.0])
            .expect("has positions");
        assert_eq!(g.steps_mm, vec![0.0]);
        assert!(g.voxel_to_patient.inverse().is_some());
    }
}
