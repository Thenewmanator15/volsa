//! The flat pictures a volume is looked at in: its axial, coronal and sagittal planes through
//! a point, where they lie in the patient, and what they hold.

use voxa_core::{Affine, Patient, Point, Volume, Voxel};

use crate::sample::sample;

/// Which of the three anatomical planes a picture shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneKind {
    /// Across the body: seen from the feet.
    Axial,
    /// Front to back: seen from the front.
    Coronal,
    /// Side to side: seen from the patient's left.
    Sagittal,
}

impl PlaneKind {
    /// The three kinds, in the order they are kept and shown in.
    pub const ALL: [Self; 3] = [Self::Axial, Self::Coronal, Self::Sagittal];

    /// The name radiology gives it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Axial => "Axial",
            Self::Coronal => "Coronal",
            Self::Sagittal => "Sagittal",
        }
    }
}

/// A flat picture's place in the patient: where its middle is, and which way its sides run.
/// `across` and `down` have a length of 1 and are square to each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// The middle of the picture.
    pub centre: Point<Patient>,
    /// The way the picture's rows run, left to right on the screen.
    pub across: [f64; 3],
    /// The way its columns run, top to bottom.
    pub down: [f64; 3],
}

impl Plane {
    /// The way the plane faces: away from the person looking at the picture.
    pub fn normal(&self) -> [f64; 3] {
        cross(self.across, self.down)
    }

    /// The point of the patient `mm` across and down from the middle of the picture.
    pub fn at(&self, [across, down]: [f64; 2]) -> Point<Patient> {
        let along = moved(self.centre, self.across, across);
        moved(along, self.down, down)
    }

    /// Where a point falls on the plane, in millimetres across and down from its middle, and
    /// how far the point is from the plane along its normal.
    pub fn of(&self, point: Point<Patient>) -> ([f64; 2], f64) {
        let from = [
            point.x - self.centre.x,
            point.y - self.centre.y,
            point.z - self.centre.z,
        ];
        let on = [dot(from, self.across), dot(from, self.down)];
        (on, dot(from, self.normal()))
    }
}

/// One of the three planes of a volume through a point. The planes follow the volume's own
/// axes, each named for the anatomical plane it is nearest, so the scan's own slices are
/// shown as they were taken and a tilted scan has tilted planes.
///
/// Axial and coronal pictures have the patient's left on the right while `radiological` is
/// true, and on the left otherwise; a sagittal picture has the front of the head on the left
/// either way. Up is the front in an axial picture and the head in the other two. The middle
/// of the picture is the middle of the volume, moved along the normal until the plane holds
/// `through`. `None` when the placement cannot be turned round.
pub fn plane_of(
    placement: &Affine<Voxel, Patient>,
    dims: [u32; 3],
    kind: PlaneKind,
    through: Point<Patient>,
    radiological: bool,
) -> Option<Plane> {
    placement.inverse()?;
    let [to_left, to_back, to_head] = by_anatomy(placement)?;
    let left = if radiological { 1.0 } else { -1.0 };
    let (across, down) = match kind {
        PlaneKind::Axial => (
            towards(to_left, [left, 0.0, 0.0]),
            towards(to_back, [0.0, 1.0, 0.0]),
        ),
        PlaneKind::Coronal => (
            towards(to_left, [left, 0.0, 0.0]),
            towards(to_head, [0.0, 0.0, -1.0]),
        ),
        PlaneKind::Sagittal => (
            towards(to_back, [0.0, 1.0, 0.0]),
            towards(to_head, [0.0, 0.0, -1.0]),
        ),
    };
    // A sheared grid's axes are not square to each other; the picture's sides have to be.
    let lean = dot(down, across);
    let down = unit([0, 1, 2].map(|part| part_of(down, part) - lean * part_of(across, part)))?;
    let middle = placement.apply(middle_voxel(dims));
    let flat = Plane {
        centre: middle,
        across,
        down,
    };
    let (_, off) = flat.of(through);
    Some(Plane {
        centre: moved(middle, flat.normal(), off),
        ..flat
    })
}

/// How far apart the layers of voxels are that a line in this direction passes through, in
/// millimetres: along a volume's slice axis, the distance between its slices. `None` for a
/// direction of no length or a placement that cannot be turned round.
pub fn pitch_along(placement: &Affine<Voxel, Patient>, direction: [f64; 3]) -> Option<f64> {
    let to_voxel = placement.inverse()?;
    let direction = unit(direction)?;
    // How many voxels the line crosses in a millimetre along columns, rows and slices.
    let crossed = [0, 1, 2].map(|axis| {
        let along_axis = to_voxel.columns().map(|column| part_of(column, axis));
        dot(along_axis, direction).abs()
    });
    let fastest = crossed.into_iter().fold(0.0, f64::max);
    (fastest > 0.0 && fastest.is_finite()).then(|| 1.0 / fastest)
}

/// The point of a volume nearest to `point`: itself if it is inside, where inside reaches
/// half a voxel beyond the centres of the edge voxels. A point that is nowhere becomes the
/// middle of the volume.
pub fn clamped_into(
    placement: &Affine<Voxel, Patient>,
    dims: [u32; 3],
    point: Point<Patient>,
) -> Point<Patient> {
    let Some(to_voxel) = placement.inverse() else {
        return point;
    };
    let voxel = to_voxel.apply(point);
    let middle = middle_voxel(dims);
    let [cols, rows, slices] = dims.map(f64::from);
    let held = |at: f64, count: f64, middle: f64| {
        if at.is_nan() {
            middle
        } else {
            at.clamp(-0.5, (count - 0.5).max(-0.5))
        }
    };
    placement.apply(Point::new(
        held(voxel.x, cols, middle.x),
        held(voxel.y, rows, middle.y),
        held(voxel.z, slices, middle.z),
    ))
}

/// The centre of the voxel in the middle of a volume. With an even number of voxels along an
/// axis it is the one just past the middle, so that a plane through this point shows a layer
/// of the scan as it was taken, not a blend of two.
pub fn central_voxel(placement: &Affine<Voxel, Patient>, dims: [u32; 3]) -> Point<Patient> {
    let [cols, rows, slices] = dims.map(|count| f64::from(count / 2));
    placement.apply(Point::new(cols, rows, slices))
}

/// A point moved `steps` whole layers of voxels along whichever of the volume's axes is
/// nearest `normal`, counting the way the voxel numbers rise: one step from a layer is the
/// next layer, and from between two layers it is the layer ahead. It stops at the first and
/// last layers. `None` for a normal of no length or a placement that cannot be turned round.
pub fn scrolled(
    placement: &Affine<Voxel, Patient>,
    dims: [u32; 3],
    point: Point<Patient>,
    normal: [f64; 3],
    steps: i32,
) -> Option<Point<Patient>> {
    let to_voxel = placement.inverse()?;
    let normal = unit(normal)?;
    if steps == 0 {
        return Some(point);
    }
    let along = placement
        .columns()
        .map(|axis| unit(axis).map_or(0.0, |axis| dot(axis, normal).abs()));
    let [by_columns, by_rows, by_slices] = along;
    let voxel = to_voxel.apply(point);
    let [cols, rows, slices] = dims;
    let stepped = |at: f64, count: u32| {
        // A point is on a layer if it is within a millionth of a voxel of it.
        let on = at.round();
        let from = match (at - on).abs() < 1.0e-6 {
            true => on,
            false if steps > 0 => at.floor(),
            false => at.ceil(),
        };
        let last = f64::from(count.saturating_sub(1));
        (from + f64::from(steps)).clamp(0.0, last)
    };
    let moved = if by_slices >= by_columns && by_slices >= by_rows {
        Point::new(voxel.x, voxel.y, stepped(voxel.z, slices))
    } else if by_rows >= by_columns {
        Point::new(voxel.x, stepped(voxel.y, rows), voxel.z)
    } else {
        Point::new(stepped(voxel.x, cols), voxel.y, voxel.z)
    };
    Some(placement.apply(moved))
}

/// Which slice of a stack of `slices` a point is nearest, counting from 0 and stopping at
/// either end.
pub fn nearest_slice(
    placement: &Affine<Voxel, Patient>,
    slices: usize,
    point: Point<Patient>,
) -> usize {
    let along = placement.inverse().map(|to_voxel| to_voxel.apply(point).z);
    let last = slices.saturating_sub(1);
    match along {
        Some(slice) if slice.is_finite() => whole(slice.round()).min(last),
        _ => 0,
    }
}

/// The values of a volume on a plane: a picture of `size` (width, height) whose pixels are
/// `mm_per_px` apart, its middle at the plane's centre, row by row from the top. Each value
/// is blended from the eight voxels round its point; outside the volume it is not a number.
pub fn reslice(volume: &Volume, plane: &Plane, size: [u32; 2], mm_per_px: f64) -> Vec<f32> {
    let [wide, high] = size;
    let from_middle = |at: u32, of: u32| (f64::from(at) + 0.5 - f64::from(of) / 2.0) * mm_per_px;
    let pixels = (0..high).flat_map(|row| (0..wide).map(move |col| (col, row)));
    pixels
        .map(|(col, row)| {
            let at = plane.at([from_middle(col, wide), from_middle(row, high)]);
            sample(volume, at).unwrap_or(f32::NAN)
        })
        .collect()
}

/// The volume's three axes, each of length 1, given out to the patient's left-right,
/// front-back and head-foot directions by whichever way of giving them out is most nearly
/// aligned in total. `None` if an axis has no length.
fn by_anatomy(placement: &Affine<Voxel, Patient>) -> Option<[[f64; 3]; 3]> {
    let [columns, rows, slices] = placement.columns();
    let [a, b, c] = [unit(columns)?, unit(rows)?, unit(slices)?];
    let ways = [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ];
    let aligned = |[[x, _, _], [_, y, _], [_, _, z]]: [[f64; 3]; 3]| x.abs() + y.abs() + z.abs();
    let mut best = [a, b, c];
    for way in ways {
        if aligned(way) > aligned(best) {
            best = way;
        }
    }
    Some(best)
}

/// An axis turned round, if need be, so that it points the same way as `wanted`.
fn towards(axis: [f64; 3], wanted: [f64; 3]) -> [f64; 3] {
    if dot(axis, wanted) < 0.0 {
        axis.map(|part| -part)
    } else {
        axis
    }
}

/// The voxel coordinate of the middle of a volume.
fn middle_voxel(dims: [u32; 3]) -> Point<Voxel> {
    let [cols, rows, slices] = dims.map(|count| (f64::from(count) - 1.0) / 2.0);
    Point::new(cols, rows, slices)
}

/// A point moved `mm` along a direction.
fn moved(from: Point<Patient>, [x, y, z]: [f64; 3], mm: f64) -> Point<Patient> {
    Point::new(from.x + mm * x, from.y + mm * y, from.z + mm * z)
}

fn part_of([x, y, z]: [f64; 3], part: usize) -> f64 {
    match part {
        0 => x,
        1 => y,
        _ => z,
    }
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

fn cross([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// A direction at a length of 1. `None` if it has no length or is not made of numbers.
fn unit(direction: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(direction, direction).sqrt();
    (length.is_finite() && length > 0.0).then(|| direction.map(|part| part / length))
}

/// A slice number that has already been rounded, as a count.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and what a count can hold first"
)]
fn whole(rounded: f64) -> usize {
    rounded.clamp(0.0, 1.0e15) as usize
}

#[cfg(test)]
mod tests;
