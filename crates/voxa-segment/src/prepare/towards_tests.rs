//! A turn towards any orientation code is `SynthSeg`'s turn when the code is its own, and points
//! each axis of the image the way its letter says otherwise.

use voxa_core::{Affine, Volume};

use super::align::Turn;
use crate::Toward::{self, Anterior, Inferior, Left, Posterior, Right, Superior};

/// A volume of ones with this placement.
fn placed(dims: [u32; 3], columns: [[f64; 3]; 3]) -> Volume {
    let [x, y, z] = columns;
    let count = dims.iter().map(|side| *side as usize).product();
    Volume::new(
        dims,
        vec![1.0; count],
        Affine::from_columns(x, y, z, [0.0; 3]),
    )
    .expect("a volume")
}

/// A few placements: as stored, turned round, swapped about and tilted.
fn placements() -> Vec<[[f64; 3]; 3]> {
    let (cos, sin) = (20_f64.to_radians().cos(), 20_f64.to_radians().sin());
    vec![
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[-1.0, 0.0, 0.0], [0.0, 0.0, -2.0], [0.0, 3.0, 0.0]],
        [[0.0, 0.0, 1.5], [0.9, 0.0, 0.0], [0.0, -0.9, 0.0]],
        [
            [cos, sin, 0.0],
            [-sin * cos, cos * cos, sin],
            [sin * sin, -sin * cos, cos],
        ],
    ]
}

#[test]
fn a_turn_towards_ras_is_synthsegs_turn() {
    for columns in placements() {
        let volume = placed([5, 6, 7], columns);
        for steps in [[1.0, 1.0, 1.0], [0.5, 2.0, 3.0]] {
            let ras = Turn::towards(&volume, steps, [Right, Anterior, Superior]);
            assert_eq!(ras, Turn::of(&volume, steps));
        }
    }
}

#[test]
fn a_volume_as_stored_turns_to_lia_by_its_letters() {
    let volume = placed([5, 6, 7], placements()[0]);
    let turn = Turn::towards(&volume, [1.0; 3], [Left, Inferior, Anterior]);
    assert_eq!(turn.order, [0, 2, 1]);
    assert_eq!(turn.turned_round, [false, true, true]);
    let lps = Turn::towards(&volume, [1.0; 3], [Left, Posterior, Superior]);
    assert_eq!(lps.order, [0, 1, 2]);
    assert_eq!(lps.turned_round, [false; 3]);
}

#[test]
fn each_axis_steps_towards_its_letter() {
    let codes = ["RAS", "LPS", "LIA", "SAR", "IPL", "ARS"];
    for columns in placements() {
        let volume = placed([5, 6, 7], columns);
        let steps = volume.voxel_to_patient().columns();
        for code in codes {
            let towards = Toward::code(code).expect("a code");
            let turn = Turn::towards(&volume, [1.0; 3], towards);
            for (axis, toward) in towards.into_iter().enumerate() {
                let (patient, negative) = toward.axis();
                let step = steps[turn.order[axis]][patient];
                let step = if turn.turned_round[axis] { -step } else { step };
                let along = if negative { -step } else { step };
                assert!(
                    along > 0.0,
                    "{code}: axis {axis} steps away from its letter"
                );
            }
        }
    }
}
