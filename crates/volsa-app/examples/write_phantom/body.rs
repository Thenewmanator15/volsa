//! A made-up body from the thighs to the top of the head, for the large studies: what lies at
//! each point of the patient, in millimetres. It is nobody's body: a few ellipses and balls
//! put where a body's parts roughly are, so that a large scan of it has air, fat, soft tissue,
//! lungs and bone in plausible places and in plausible amounts.
//!
//! Points are in the patient's own coordinates: `x` towards the patient's left, `y` towards
//! the back, `z` towards the head, from 0 at the thighs to [`HEIGHT`] at the top of the head.

/// How far the body reaches from the lowest point drawn to the top of the head.
pub const HEIGHT: f64 = 1250.0;

/// What lies at a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tissue {
    Air,
    Fat,
    Soft,
    Lung,
    Bone,
    Marrow,
    Brain,
    Blood,
    Liver,
    Kidney,
    /// The table the patient lies on, behind them.
    Table,
}

/// What lies at `[x, y, z]`.
pub fn at([x, y, z]: [f64; 3]) -> Tissue {
    if (140.0..150.0).contains(&y) && x.abs() < 200.0 {
        return Tissue::Table;
    }
    if z >= 1010.0 {
        head(x, y, z)
    } else if z >= 960.0 {
        neck(x, y, z)
    } else if z >= 390.0 {
        trunk(x, y, z)
    } else {
        legs(x, y, z)
    }
}

/// `(x / a)² + (y / b)²`: below 1 inside an ellipse of half-widths `a` and `b`.
fn ellipse(x: f64, y: f64, a: f64, b: f64) -> f64 {
    (x / a).powi(2) + (y / b).powi(2)
}

/// As [`ellipse`], for a ball of half-widths `halves` about `centre`.
fn ball(point: [f64; 3], centre: [f64; 3], halves: [f64; 3]) -> f64 {
    (0..3)
        .map(|axis| ((point[axis] - centre[axis]) / halves[axis]).powi(2))
        .sum()
}

fn head(x: f64, y: f64, z: f64) -> Tissue {
    let centre = [0.0, 0.0, 1130.0];
    let outer = [79.0, 99.0, 119.0];
    let shrunk = |by: f64| outer.map(|half| half - by);
    let p = [x, y, z];
    if ball(p, centre, outer) > 1.0 {
        Tissue::Air
    } else if ball(p, centre, shrunk(4.0)) > 1.0 {
        Tissue::Soft
    } else if ball(p, centre, shrunk(11.0)) > 1.0 {
        Tissue::Bone
    } else {
        Tissue::Brain
    }
}

fn neck(x: f64, y: f64, z: f64) -> Tissue {
    if ellipse(x, y - 10.0, 55.0, 55.0) > 1.0 {
        Tissue::Air
    } else {
        spine(x, y - 25.0, z, 12.0).unwrap_or(Tissue::Soft)
    }
}

/// The bone of the spine about `(x, y)`, a column of radius `r` with a disc every 30 mm;
/// `None` away from it.
fn spine(x: f64, y: f64, z: f64, r: f64) -> Option<Tissue> {
    let from_middle = x.hypot(y);
    if from_middle > r || z.rem_euclid(30.0) < 6.0 {
        return (from_middle <= r).then_some(Tissue::Soft);
    }
    Some(if from_middle > r - 3.0 {
        Tissue::Bone
    } else {
        Tissue::Marrow
    })
}

/// How wide the trunk is at height `z`: half-widths across and from front to back.
fn trunk_size(z: f64) -> (f64, f64) {
    let across = match z {
        z if z > 900.0 => 170.0 - (z - 900.0) * 1.4,
        z if z > 680.0 => 170.0,
        z if z > 480.0 => 160.0,
        _ => 180.0,
    };
    (across, if z > 480.0 { 115.0 } else { 120.0 })
}

fn trunk(x: f64, y: f64, z: f64) -> Tissue {
    let (across, deep) = trunk_size(z);
    let edge = ellipse(x, y, across, deep);
    if edge > 1.0 {
        return Tissue::Air;
    }
    if ellipse(x, y, across - 12.0, deep - 12.0) > 1.0 {
        return Tissue::Fat;
    }
    if let Some(found) = spine(x, y - 60.0, z, 18.0) {
        return found;
    }
    let ribs = (700.0..940.0).contains(&z)
        && z.rem_euclid(25.0) < 9.0
        && ellipse(x, y, across - 18.0, deep - 18.0) > 1.0;
    if ribs {
        return Tissue::Bone;
    }
    organs([x, y, z]).unwrap_or(Tissue::Soft)
}

/// The organs of the trunk; `None` between them.
fn organs(p: [f64; 3]) -> Option<Tissue> {
    let [x, y, z] = p;
    if ball(p, [20.0, -20.0, 760.0], [55.0, 50.0, 50.0]) < 1.0 {
        return Some(Tissue::Blood);
    }
    let lung = |side: f64| ball(p, [side * 70.0, 10.0, 830.0], [55.0, 75.0, 100.0]) < 1.0;
    if lung(1.0) || lung(-1.0) {
        return Some(Tissue::Lung);
    }
    // The liver is on the patient's right, which is towards -x.
    if ball(p, [-70.0, 0.0, 640.0], [85.0, 75.0, 70.0]) < 1.0 {
        return Some(Tissue::Liver);
    }
    let kidney = |side: f64| ball(p, [side * 70.0, 50.0, 560.0], [25.0, 25.0, 55.0]) < 1.0;
    if kidney(1.0) || kidney(-1.0) {
        return Some(Tissue::Kidney);
    }
    let hip = |side: f64| {
        let from = (x - side * 95.0).hypot(y - 30.0);
        z < 480.0 && (37.0..45.0).contains(&from)
    };
    (hip(1.0) || hip(-1.0)).then_some(Tissue::Bone)
}

fn legs(x: f64, y: f64, z: f64) -> Tissue {
    let radius = 70.0 + 15.0 * (z / 390.0);
    for side in [-1.0, 1.0] {
        let from = (x - side * 95.0).hypot(y - 10.0);
        if from > radius {
            continue;
        }
        let bone = (x - side * 95.0).hypot(y - 15.0);
        return match bone {
            b if b < 9.0 => Tissue::Marrow,
            b if b < 14.0 => Tissue::Bone,
            _ if from > radius - 10.0 => Tissue::Fat,
            _ => Tissue::Soft,
        };
    }
    Tissue::Air
}

/// A number from -1 to 1 that looks random but is the same every time for the same pixel:
/// the noise of a scan.
pub fn noise(col: u32, row: u32, slice: u32) -> f64 {
    let mut h = u64::from(col) | (u64::from(row) << 20) | (u64::from(slice) << 40);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_has_its_parts_where_a_body_has_them() {
        assert_eq!(at([0.0, 0.0, 1130.0]), Tissue::Brain);
        assert_eq!(at([70.0, 10.0, 830.0]), Tissue::Lung);
        assert_eq!(at([-70.0, 0.0, 640.0]), Tissue::Liver);
        assert_eq!(at([95.0, 15.0, 200.0]), Tissue::Marrow);
        assert_eq!(at([0.0, -200.0, 600.0]), Tissue::Air);
        assert_eq!(at([0.0, 145.0, 10.0]), Tissue::Table);
    }

    #[test]
    fn noise_stays_between_minus_one_and_one() {
        let all = (0..1000).map(|i| noise(i, i * 7, i * 13));
        assert!(all.clone().all(|n| (-1.0..1.0).contains(&n)));
        let mean = all.sum::<f64>() / 1000.0;
        assert!(mean.abs() < 0.1, "{mean}");
    }
}
