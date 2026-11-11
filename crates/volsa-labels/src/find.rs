//! Finding a part: the one at a point of the patient, and the first one a line of sight
//! meets. Both read the label volume on its own grid; nothing is resampled.

use voxa_core::{Patient, Point, Voxel};

use crate::set::LabelSet;

/// The most steps a line of sight is walked in: far more than any label volume is across.
const MAX_STEPS: u32 = 100_000;

impl LabelSet {
    /// The part at a point of the patient: that of the nearest voxel, counted from 0 in
    /// `parts`. `None` outside the volume, where the voxel holds nothing, and for a point
    /// that is not a number.
    pub fn part_at(&self, point: Point<Patient>) -> Option<usize> {
        let voxel = self.placement.inverse()?.apply(point);
        self.part_in(voxel)
    }

    /// The part of the voxel nearest a place given in voxels.
    fn part_in(&self, voxel: Point<Voxel>) -> Option<usize> {
        let [cols, rows, slices] = self.dims;
        let (col, row, slice) = (
            nearest(voxel.x, cols)?,
            nearest(voxel.y, rows)?,
            nearest(voxel.z, slices)?,
        );
        let (cols, rows) = (as_index(cols), as_index(rows));
        let number = *self.numbers.get((slice * rows + row) * cols + col)?;
        let part = usize::from(number).checked_sub(1)?;
        (part < self.parts.len()).then_some(part)
    }

    /// The eight corners of the volume's box: half a voxel past its outermost voxels' centres.
    pub fn corners(&self) -> [Point<Patient>; 8] {
        let [cols, rows, slices] = self.dims.map(f64::from);
        let end = |far: bool, count: f64| if far { count - 0.5 } else { -0.5 };
        std::array::from_fn(|corner| {
            let at = Point::new(
                end(corner & 1 != 0, cols),
                end(corner & 2 != 0, rows),
                end(corner & 4 != 0, slices),
            );
            self.placement.apply(at)
        })
    }

    /// The least distance between neighbouring voxels, in millimetres.
    pub fn finest_mm(&self) -> f64 {
        let steps = self.placement.columns().map(length);
        steps.into_iter().fold(f64::INFINITY, f64::min)
    }
}

/// The first part a line of sight meets, among those `shown` (by their place in `parts`; a
/// part past the end of `shown` is not shown), and the point of the patient where it meets
/// it. The line starts at `from` and runs the way `towards` points. It is walked in steps of
/// half the finest voxel from where it enters the volume's box to where it leaves. `None`
/// when it meets nothing shown, misses the box, or has no direction.
pub fn first_part(
    set: &LabelSet,
    shown: &[bool],
    from: Point<Patient>,
    towards: [f64; 3],
) -> Option<(usize, Point<Patient>)> {
    let long = length(towards);
    if !(long.is_finite() && long > 0.0) {
        return None;
    }
    let [dx, dy, dz] = towards.map(|part| part / long);
    let along = |mm: f64| Point::new(from.x + dx * mm, from.y + dy * mm, from.z + dz * mm);
    let to_voxel = set.placement.inverse()?;
    let (start, one_mm_on) = (to_voxel.apply(from), to_voxel.apply(along(1.0)));
    let (enters, leaves) = through_box(set.dims, start, one_mm_on)?;
    let step = set.finest_mm() / 2.0;
    if !(step.is_finite() && step > 0.0) {
        return None;
    }
    for walked in 0..MAX_STEPS {
        let mm = enters + f64::from(walked) * step;
        if mm > leaves {
            return None;
        }
        let met = to_voxel.apply(along(mm));
        let part = set.part_in(met);
        if let Some(part) = part.filter(|part| shown.get(*part).copied().unwrap_or(false)) {
            return Some((part, along(mm)));
        }
    }
    None
}

/// How far along a line, in millimetres from its start, it enters and leaves a volume's box.
/// `start` and `one_mm_on` are the line's start and the point a millimetre along it, in
/// voxels. `None` when the line misses the box or the box is behind its start.
fn through_box(dims: [u32; 3], start: Point<Voxel>, one_mm_on: Point<Voxel>) -> Option<(f64, f64)> {
    let ways = [
        (start.x, one_mm_on.x - start.x),
        (start.y, one_mm_on.y - start.y),
        (start.z, one_mm_on.z - start.z),
    ];
    let (mut enters, mut leaves) = (0.0_f64, f64::INFINITY);
    for ((at, per_mm), count) in ways.into_iter().zip(dims) {
        let (low, high) = (-0.5, f64::from(count) - 0.5);
        if per_mm.abs() < f64::EPSILON {
            // Running along this pair of faces: inside them all the way, or never.
            if !(low..high).contains(&at) {
                return None;
            }
            continue;
        }
        let (one, other) = ((low - at) / per_mm, (high - at) / per_mm);
        enters = enters.max(one.min(other));
        leaves = leaves.min(one.max(other));
    }
    (enters <= leaves && leaves.is_finite()).then_some((enters, leaves))
}

/// Whether two boxes, each given by its eight corners, overlap: whether their reaches along
/// the patient's three axes all overlap. Boxes that are tilted may be said to meet when they
/// only come close, which is the safe way round for asking "are these labels of this scan".
pub fn boxes_meet(a: &[Point<Patient>; 8], b: &[Point<Patient>; 8]) -> bool {
    let axes: [fn(&Point<Patient>) -> f64; 3] = [|at| at.x, |at| at.y, |at| at.z];
    axes.into_iter().all(|axis| {
        let reach = |corners: &[Point<Patient>; 8]| {
            let along = corners.iter().map(axis);
            let low = along.clone().fold(f64::INFINITY, f64::min);
            (low, along.fold(f64::NEG_INFINITY, f64::max))
        };
        let ((a_low, a_high), (b_low, b_high)) = (reach(a), reach(b));
        a_low <= b_high && b_low <= a_high
    })
}

/// The voxel along one axis nearest a place, if that place is within the volume.
fn nearest(at: f64, count: u32) -> Option<usize> {
    let within = (-0.5..f64::from(count) - 0.5).contains(&at);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked to lie within the volume, which is no more than 65 535 voxels a side"
    )]
    within.then_some(((at + 0.5).floor() as usize).min(as_index(count).saturating_sub(1)))
}

fn as_index(count: u32) -> usize {
    usize::try_from(count).unwrap_or(usize::MAX)
}

fn length([x, y, z]: [f64; 3]) -> f64 {
    (x * x + y * y + z * z).sqrt()
}
