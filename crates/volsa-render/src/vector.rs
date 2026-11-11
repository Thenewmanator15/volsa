//! Arithmetic on directions and positions in the patient, as three numbers.

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// `from` moved `by` times along `step`.
pub(crate) fn moved(from: [f64; 3], by: f64, step: [f64; 3]) -> [f64; 3] {
    let ([x, y, z], [dx, dy, dz]) = (from, step);
    [x + by * dx, y + by * dy, z + by * dz]
}

/// The same direction with a length of 1. `None` for one that has no direction: of no
/// length, or not made of finite numbers.
pub(crate) fn unit(direction: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(direction, direction).sqrt();
    (length.is_finite() && length > 0.0).then(|| direction.map(|part| part / length))
}

/// `direction` turned by `angle` radians about `axis`, which has a length of 1: anticlockwise
/// as seen looking back along the axis.
pub(crate) fn turned_about(direction: [f64; 3], axis: [f64; 3], angle: f64) -> [f64; 3] {
    let (sin, cos) = angle.sin_cos();
    let along = dot(axis, direction) * (1.0 - cos);
    moved(
        moved(
            direction.map(|part| part * cos),
            sin,
            cross(axis, direction),
        ),
        along,
        axis,
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn close(found: [f64; 3], expected: [f64; 3]) -> bool {
        found
            .into_iter()
            .zip(expected)
            .all(|(found, expected)| (found - expected).abs() < 1e-12)
    }

    #[test]
    fn the_cross_of_left_and_back_is_up() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
        assert_eq!(
            moved([1.0, 2.0, 3.0], 2.0, [1.0, 0.0, -1.0]),
            [3.0, 2.0, 1.0]
        );
    }

    #[test]
    fn a_direction_is_given_a_length_of_one_if_it_has_a_direction() {
        assert_eq!(unit([0.0, -4.0, 0.0]), Some([0.0, -1.0, 0.0]));
        assert_eq!(unit([0.0; 3]), None);
        assert_eq!(unit([f64::NAN, 1.0, 0.0]), None);
        assert_eq!(unit([f64::INFINITY, 1.0, 0.0]), None);
    }

    #[test]
    fn a_quarter_turn_about_an_axis_takes_each_other_axis_to_the_next() {
        let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(close(turned_about(x, z, FRAC_PI_2), y));
        assert!(close(turned_about(y, z, FRAC_PI_2), [-1.0, 0.0, 0.0]));
        assert!(
            close(turned_about(z, z, FRAC_PI_2), z),
            "the axis itself stays put"
        );
        assert!(close(
            turned_about([1.0, 0.0, 1.0], z, FRAC_PI_2),
            [0.0, 1.0, 1.0]
        ));
    }
}
