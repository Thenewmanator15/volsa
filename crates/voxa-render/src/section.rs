//! The plane views as they are drawn inside the 3D scene: where each plane cuts the volume's
//! box, and the cross at the position they share.

use voxa_core::{Patient, Point};

use crate::camera::Camera;
use crate::handle::on_screen;
use crate::vector::{cross, dot, unit};

/// Two points of a cut are taken as one when they are this many millimetres apart or less:
/// a plane through a corner of the box meets three of its edges at that one point.
const SAME_POINT_MM: f64 = 1.0e-6;
/// The edges of a box, each as the two corners it joins. Corner `i` is at the far end of the
/// columns when `i & 1` is set, of the rows when `i & 2` is, and of the slices when `i & 4`.
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Where a plane cuts a volume's box: the corners of the cut, in order round it. There are
/// three to six of them, or none when the plane misses the box or `normal` has no length.
/// `corners` are the box's eight, as [`voxa_core::Volume::corners`] gives them.
pub fn box_section(
    corners: &[Point<Patient>; 8],
    point: Point<Patient>,
    normal: [f64; 3],
) -> Vec<Point<Patient>> {
    let Some(normal) = unit(normal) else {
        return Vec::new();
    };
    // How far in front of the plane each corner is.
    let ahead = |corner: &Point<Patient>| dot(from_to(point, *corner), normal);
    let mut met: Vec<Point<Patient>> = Vec::new();
    for (from, to) in EDGES {
        let (Some(from), Some(to)) = (corners.get(from), corners.get(to)) else {
            continue;
        };
        let (before, after) = (ahead(from), ahead(to));
        // Both on one side of the plane, or the whole edge lying in it: no one point is met.
        let span = before - after;
        if before * after > 0.0 || span.abs() <= 0.0 {
            continue;
        }
        let along = before / span;
        let [x, y, z] = from_to(*from, *to).map(|part| part * along);
        let at = Point::new(from.x + x, from.y + y, from.z + z);
        if met
            .iter()
            .all(|other| length(from_to(*other, at)) > SAME_POINT_MM)
        {
            met.push(at);
        }
    }
    if met.len() < 3 {
        return Vec::new();
    }
    in_order_round(met, normal)
}

/// Points of one plane put in order round their middle, turning one way about `normal`.
fn in_order_round(mut points: Vec<Point<Patient>>, normal: [f64; 3]) -> Vec<Point<Patient>> {
    let count = points.len() as f64;
    let sum = points
        .iter()
        .fold([0.0; 3], |[x, y, z], at| [x + at.x, y + at.y, z + at.z]);
    let [x, y, z] = sum.map(|part| part / count);
    let middle = Point::new(x, y, z);
    // Two directions in the plane, square to each other, to measure the turn against.
    let Some(first) = points.first().and_then(|at| unit(from_to(middle, *at))) else {
        return points;
    };
    let second = cross(normal, first);
    let turn = |at: &Point<Patient>| {
        let out = from_to(middle, *at);
        dot(out, second).atan2(dot(out, first))
    };
    points.sort_by(|a, b| turn(a).total_cmp(&turn(b)));
    points
}

/// The cross drawn at the position, as it is on the screen: for each of `axes`, the two ends
/// of an arm reaching `arm_mm` either way along it, first the end against the axis and then
/// the end along it. An arm seen end on has both its ends at the position. Places are in
/// pixels from the top left corner of a viewport of this width and height.
pub fn position_cross(
    camera: &Camera,
    viewport: [f32; 2],
    position: Point<Patient>,
    axes: [[f64; 3]; 3],
    arm_mm: f64,
) -> [[[f32; 2]; 2]; 3] {
    let end = |axis: [f64; 3], reach: f64| {
        let [x, y, z] = unit(axis).unwrap_or([0.0; 3]).map(|part| part * reach);
        let at = Point::new(position.x + x, position.y + y, position.z + z);
        on_screen(camera, viewport, at)
    };
    axes.map(|axis| [end(axis, -arm_mm), end(axis, arm_mm)])
}

fn from_to(from: Point<Patient>, to: Point<Patient>) -> [f64; 3] {
    [to.x - from.x, to.y - from.y, to.z - from.z]
}

fn length(of: [f64; 3]) -> f64 {
    dot(of, of).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::NamedView;

    /// The corners of the box from (0, 0, 0) to (2, 2, 2).
    fn cube() -> [Point<Patient>; 8] {
        let far = |i: usize, bit: usize| if i & bit == 0 { 0.0 } else { 2.0 };
        std::array::from_fn(|i| Point::new(far(i, 1), far(i, 2), far(i, 4)))
    }

    fn middle() -> Point<Patient> {
        Point::new(1.0, 1.0, 1.0)
    }

    /// How far each point of a cut is from the next, going round and back to the first.
    fn sides(cut: &[Point<Patient>]) -> Vec<f64> {
        let next = cut.iter().cycle().skip(1);
        cut.iter()
            .zip(next)
            .map(|(a, b)| length(from_to(*a, *b)))
            .collect()
    }

    #[test]
    fn a_plane_square_to_a_box_cuts_it_in_a_square() {
        let cut = box_section(&cube(), middle(), [0.0, 0.0, 1.0]);
        assert_eq!(cut.len(), 4);
        assert!(cut.iter().all(|at| at.z == 1.0), "{cut:?}");
        let sides = sides(&cut);
        assert!(
            sides.iter().all(|side| (side - 2.0).abs() < 1.0e-12),
            "going round: {sides:?}"
        );
        let upside_down = box_section(&cube(), middle(), [0.0, 0.0, -3.0]);
        assert_eq!(upside_down.len(), 4, "whichever way the plane faces");
    }

    #[test]
    fn a_plane_across_the_corners_of_a_box_cuts_it_in_a_hexagon() {
        let cut = box_section(&cube(), middle(), [1.0, 1.0, 1.0]);
        assert_eq!(cut.len(), 6);
        let sides = sides(&cut);
        let expected = 2.0_f64.sqrt();
        assert!(
            sides.iter().all(|side| (side - expected).abs() < 1.0e-12),
            "{sides:?}"
        );
    }

    #[test]
    fn a_plane_on_a_face_of_a_box_cuts_it_once_at_each_corner_of_the_face() {
        let cut = box_section(&cube(), Point::new(0.0, 0.0, 0.0), [0.0, 0.0, 1.0]);
        assert_eq!(cut.len(), 4, "{cut:?}");
        assert!(cut.iter().all(|at| at.z == 0.0));
    }

    #[test]
    fn a_plane_that_misses_the_box_or_faces_no_way_cuts_nothing() {
        let far = Point::new(9.0, 9.0, 9.0);
        assert_eq!(box_section(&cube(), far, [0.0, 0.0, 1.0]), []);
        assert_eq!(box_section(&cube(), middle(), [0.0; 3]), []);
        assert_eq!(box_section(&cube(), middle(), [f64::NAN, 0.0, 1.0]), []);
        // Through one corner only: a point, not a cut.
        let corner = Point::new(2.0, 2.0, 2.0);
        assert_eq!(box_section(&cube(), corner, [1.0, 1.0, 1.0]), []);
    }

    #[test]
    fn the_cross_at_the_position_is_seen_as_the_camera_sees_its_arms() {
        // From the front at half a millimetre a pixel: the patient's left is on the right.
        let camera = Camera::looking(NamedView::Anterior, middle(), 0.5);
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let [left, back, up] = position_cross(&camera, [200.0, 100.0], middle(), axes, 10.0);
        let near = |arm: [[f32; 2]; 2], expected: [[f32; 2]; 2]| {
            let ends = arm
                .into_iter()
                .flatten()
                .zip(expected.into_iter().flatten());
            ends.into_iter()
                .all(|(found, wanted)| (found - wanted).abs() < 1.0e-3)
        };
        assert!(
            near(left, [[80.0, 50.0], [120.0, 50.0]]),
            "across the screen: {left:?}"
        );
        assert!(near(up, [[100.0, 70.0], [100.0, 30.0]]), "up it: {up:?}");
        assert!(
            near(back, [[100.0, 50.0], [100.0, 50.0]]),
            "end on, a point: {back:?}"
        );
        let longer = [[5.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let [same, none, _] = position_cross(&camera, [200.0, 100.0], middle(), longer, 10.0);
        assert!(near(same, left), "only the way an axis points counts");
        assert!(near(none, back), "an axis with no direction is a point too");
    }
}
