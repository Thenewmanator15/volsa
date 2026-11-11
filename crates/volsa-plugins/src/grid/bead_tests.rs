//! A bead off the centre of a made-up scan, sampled into the plugin's grid for every
//! orientation code and mapped back as labels, lands where it was in the patient; and each
//! axis of the grid steps the way its letter says.

use volsa_core::{Affine, Patient, Point, Voxel};
use volsa_segment::Toward;

use super::tests::{input, placed};
use super::{Grid, label_grid, sample};

/// Where the bead is, in the scan's voxels.
const BEAD: [u32; 3] = [2, 3, 7];

/// Every orientation code: each order of the three axes, each way along each.
fn every_code() -> Vec<String> {
    let pairs = [['L', 'R'], ['P', 'A'], ['S', 'I']];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut codes = Vec::new();
    for order in orders {
        for signs in 0..8_usize {
            let letter = |place: usize| pairs[order[place]][(signs >> place) & 1];
            codes.push([letter(0), letter(1), letter(2)].iter().collect());
        }
    }
    codes
}

/// The distance between two points, in millimetres.
fn apart(a: Point<Patient>, b: Point<Patient>) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Samples the bead into the grid of `code`, labels it, maps it back, and checks that the
/// labelled voxel lies on the bead and that each axis steps towards its letter.
fn bead_comes_back(placement: Affine<Voxel, Patient>, code: &str) {
    let volume = placed([9, 10, 11], placement, |col, row, slice| {
        f32::from(u8::from([col, row, slice] == BEAD))
    });
    let grid = Grid::of(&volume, &input(code, [1.0; 3]));
    let region = grid.whole();
    let values = sample(&volume, &grid, region, 0.0).expect("sampled");
    let ids: Vec<u16> = values.iter().map(|value| u16::from(*value > 0.5)).collect();
    let labels = label_grid(&grid, region, &ids, &volume);
    let at = labels
        .ids
        .iter()
        .position(|id| *id == 1)
        .expect("the bead is labelled");
    let [cols, rows, _] = labels.dims.map(|side| side as usize);
    let voxel = Point::new(
        (at % cols) as f64,
        ((at / cols) % rows) as f64,
        (at / (cols * rows)) as f64,
    );
    let bead = volume.voxel_to_patient().apply(Point::new(
        f64::from(BEAD[0]),
        f64::from(BEAD[1]),
        f64::from(BEAD[2]),
    ));
    let landed = labels.placement.apply(voxel);
    assert!(
        apart(landed, bead) < 1e-6,
        "{code}: landed {landed:?}, bead {bead:?}"
    );

    let towards = Toward::code(code).expect("a code");
    for (axis, toward) in towards.into_iter().enumerate() {
        let mut one = [0.0; 3];
        one[axis] = 1.0;
        let step = grid.patient_of(&volume, one);
        let start = grid.patient_of(&volume, [0.0; 3]);
        let moved = [step.x - start.x, step.y - start.y, step.z - start.z];
        let (patient, negative) = toward.axis();
        let along = if negative {
            -moved[patient]
        } else {
            moved[patient]
        };
        assert!(
            along > 0.5,
            "{code}: axis {axis} does not step towards {toward:?}"
        );
    }
}

#[test]
fn every_axis_code_puts_the_bead_back_where_it_was() {
    let as_stored = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [4.0, -2.0, 9.0],
    );
    let codes = every_code();
    assert_eq!(codes.len(), 48);
    for code in &codes {
        bead_comes_back(as_stored, code);
    }
}

#[test]
fn a_scan_stored_another_way_round_keeps_the_bead() {
    let swapped = Affine::from_columns(
        [0.0, 0.0, -1.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [10.0, 5.0, -3.0],
    );
    for code in every_code() {
        bead_comes_back(swapped, &code);
    }
}

#[test]
fn a_tilted_scan_keeps_the_bead() {
    let (cos, sin) = (20_f64.to_radians().cos(), 20_f64.to_radians().sin());
    let tilted = Affine::from_columns(
        [cos, sin, 0.0],
        [-sin * cos, cos * cos, sin],
        [sin * sin, -sin * cos, cos],
        [1.0, 2.0, 3.0],
    );
    for code in every_code() {
        bead_comes_back(tilted, &code);
    }
}
