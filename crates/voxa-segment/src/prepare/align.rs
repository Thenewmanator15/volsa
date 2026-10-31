// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `get_ras_axes` and `align_volume_to_ref` in ext/lab2im/edit_volumes.py,
// translated to Rust and changed for Voxa.
//! Turning a volume's axes to the order and direction `SynthSeg`'s networks were trained on: x
//! to the patient's right, y to the front, z to the top. No value changes; none is resampled.
//!
//! The rules are those of `SynthSeg`'s `get_ras_axes` and `align_volume_to_ref`. They are
//! written for a transform to right-anterior-superior millimetres. Voxa's is to
//! left-posterior-superior, so its x and y rows change sign before the rules are applied.

use voxa_core::Volume;

use crate::Toward;

/// How a volume's axes are turned for a network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    /// For each of the image's axes (x, y, z), the volume's axis it runs along: 0 for
    /// columns, 1 for rows, 2 for slices.
    pub order: [usize; 3],
    /// For each of the image's axes, whether it runs the other way from the volume's axis.
    pub turned_round: [bool; 3],
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
    /// How `SynthSeg` turns the grid this volume has at 1 mm, worked out from the volume's
    /// transform alone: no value is read, and no grid made. `steps` is how many voxels of
    /// that grid there are to one of the volume's along its columns, rows and slices: its
    /// spacing in millimetres when it is resampled, 1 when it is left as it is. The two can
    /// give different turns, since an axis with a long step counts for less as it is stored.
    ///
    /// For each of right, front and top `SynthSeg` takes the grid's axis that lies most along
    /// it, judged by the transform back from the patient to the voxels: the largest number,
    /// whatever its sign, in the column of that direction. An axis is turned round when it
    /// runs away from its direction.
    pub(super) fn of(volume: &Volume, steps: [f64; 3]) -> Self {
        Self::towards(
            volume,
            steps,
            [Toward::Right, Toward::Anterior, Toward::Superior],
        )
    }

    /// How a volume's axes are turned so that the image's axes point towards these three
    /// directions, in order: `SynthSeg`'s rule, for any orientation code. For each of the
    /// patient's three axes, in `SynthSeg`'s order, the volume's axis that lies most along it is
    /// chosen and mended as [`Turn::of`] does; then the image's axes take them in the order of
    /// the code, each turned round when the volume's step runs away from its letter. With the
    /// code right, front, top this is exactly [`Turn::of`].
    pub fn towards(volume: &Volume, steps: [f64; 3], towards: [Toward; 3]) -> Self {
        let [across, down, through] = steps;
        // The grid's transform is the volume's with each step made shorter, so its transform
        // back has each number made as many times larger. (Changing the sign of a row of a
        // transform changes the sign of a column of the transform back, and only the sizes
        // of its numbers are asked for here.)
        let at_1_mm = |[col, row, slice]: [f64; 3]| [col * across, row * down, slice * through];
        let back = volume.patient_to_voxel().columns();
        let mut by_patient = back.map(at_1_mm).map(largest);
        mend(&mut by_patient);

        // A step keeps its direction however long it is.
        let steps = volume.voxel_to_patient().columns();
        let mut order = [0; 3];
        let mut turned_round = [false; 3];
        for (axis, toward) in towards.into_iter().enumerate() {
            let (patient, negative) = toward.axis();
            let from = by_patient.get(patient).copied().unwrap_or(0);
            let step = steps.get(from).and_then(|step| step.get(patient)).copied();
            let along = step.map(|step| if negative { -step } else { step });
            if let (Some(slot), Some(round)) = (order.get_mut(axis), turned_round.get_mut(axis)) {
                *slot = from;
                *round = along.is_some_and(|along| along < 0.0);
            }
        }
        Self {
            order,
            turned_round,
        }
    }

    /// How many voxels a grid with these columns, rows and slices has along the image's z,
    /// from the bottom of the patient to the top, once it is turned.
    pub(super) fn tall(&self, sides: [u32; 3]) -> u32 {
        let [_, _, from] = self.order;
        sides.get(from).copied().unwrap_or(0)
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
