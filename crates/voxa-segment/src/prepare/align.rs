//! Turning a volume's axes to the order and direction `SynthSeg`'s networks were trained on: x
//! to the patient's right, y to the front, z to the top. No value changes; none is resampled.
//!
//! The rules are those of `SynthSeg`'s `get_ras_axes` and `align_volume_to_ref`. They are
//! written for a transform to right-anterior-superior millimetres. Voxa's is to
//! left-posterior-superior, so its x and y rows change sign before the rules are applied.

use voxa_core::Volume;

/// How a volume's axes are turned for the networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Turn {
    /// For each of the image's axes (x, y, z), the volume's axis it runs along: 0 for
    /// columns, 1 for rows, 2 for slices.
    pub(super) order: [usize; 3],
    /// For each of the image's axes, whether it runs the other way from the volume's axis.
    pub(super) turned_round: [bool; 3],
}

/// A volume's values with its axes turned.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Turned {
    /// How many voxels lie along x, y and z.
    pub(super) size: [usize; 3],
    /// The values, `[x, y, z]` with z fastest.
    pub(super) values: Vec<f32>,
}

impl Turn {
    /// How `SynthSeg` turns this volume.
    ///
    /// For each of right, front and top it takes the volume's axis that lies most along it,
    /// judged as `SynthSeg` judges it: by the transform back from the patient to the voxels,
    /// the largest number, whatever its sign, in the column of that direction. An axis is
    /// turned round when it runs away from its direction.
    pub(super) fn of(volume: &Volume) -> Self {
        // Changing the sign of a row of the transform changes the sign of a column of the
        // transform back, and only the sizes of its numbers are asked for here.
        let mut order = volume.patient_to_voxel().columns().map(largest);
        mend(&mut order);

        let steps = volume.voxel_to_patient().columns().map(right_front_top);
        let mut turned_round = [false; 3];
        for (direction, (round, &axis)) in turned_round.iter_mut().zip(&order).enumerate() {
            let step = steps.get(axis).and_then(|step| step.get(direction));
            *round = step.is_some_and(|&part| part < 0.0);
        }
        Self {
            order,
            turned_round,
        }
    }

    /// The volume's values in the image's order: `[x, y, z]`, z fastest.
    pub(super) fn turned(&self, volume: &Volume) -> Turned {
        let dims = volume.dims().map(|count| count as usize);
        let [cols, rows, _] = dims;
        let strides = [1, cols, cols * rows];
        // Where each step along one of the image's axes lands in the volume's list of values.
        let landings = |axis: usize, round: bool| -> Vec<usize> {
            let length = dims.get(axis).copied().unwrap_or(0);
            let stride = strides.get(axis).copied().unwrap_or(0);
            let place = |step: usize| if round { length - 1 - step } else { step };
            (0..length).map(|step| place(step) * stride).collect()
        };
        let [from_x, from_y, from_z] = self.order;
        let [round_x, round_y, round_z] = self.turned_round;
        let xs = landings(from_x, round_x);
        let ys = landings(from_y, round_y);
        let zs = landings(from_z, round_z);
        let stored = volume.values();
        let mut values = Vec::with_capacity(stored.len());
        for x in &xs {
            for y in &ys {
                let line = zs
                    .iter()
                    .map(|z| stored.get(x + y + z).copied().unwrap_or(0.0));
                values.extend(line);
            }
        }
        Turned {
            size: [xs.len(), ys.len(), zs.len()],
            values,
        }
    }
}

/// A step in Voxa's left-posterior-superior as `SynthSeg` sees it: right, front, top.
fn right_front_top([left, back, top]: [f64; 3]) -> [f64; 3] {
    [-left, -back, top]
}

/// Which of three numbers is the largest, whatever its sign: the first of them on a tie.
fn largest([a, b, c]: [f64; 3]) -> usize {
    let (a, b, c) = (a.abs(), b.abs(), c.abs());
    if a >= b && a >= c {
        0
    } else if b >= c {
        1
    } else {
        2
    }
}

/// `SynthSeg`'s repair for a volume lying so far askew that two directions chose the same
/// axis. Each axis that no direction chose, lowest first, is given to the last direction that
/// holds the axis chosen most often (the lowest such axis when two are chosen as often).
fn mend(order: &mut [usize; 3]) {
    for missing in 0..3 {
        if order.contains(&missing) {
            continue;
        }
        let chosen = |axis: usize| order.iter().filter(|&&held| held == axis).count();
        // `max_by_key` keeps the last of equals, so the axes are offered highest first.
        let most = (0..3).rev().max_by_key(|&axis| chosen(axis)).unwrap_or(0);
        if let Some(held) = order.iter_mut().rev().find(|held| **held == most) {
            *held = missing;
        }
    }
}
