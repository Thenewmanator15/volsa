use volsa_core::{Affine, Volume};

use super::pieces::drop_small;
use super::smooth::kernel;
use super::tube::{dark_tube, eigenvalues};
use super::{VEINS_PART, VeinsError, veins, veins_table};

const SIDE: u32 = 48;

/// A made-up head: a bright cube of tissue with dark air round it, holding a thin dark tube
/// that runs up through it, a dark sheet across it and a dark ball, each `mm` apart from the
/// next, on voxels 1 mm wide.
fn made_up_head() -> Volume {
    let side = f64::from(SIDE);
    let mut values = Vec::new();
    for slice in 0..SIDE {
        for row in 0..SIDE {
            for column in 0..SIDE {
                let [x, y, z] = [column, row, slice].map(f64::from);
                let tissue = [x, y, z].iter().all(|at| (6.0..side - 6.0).contains(at));
                let tube = (x - 16.0).hypot(y - 16.0) <= 1.2;
                let sheet = (x - 32.0).abs() <= 0.6 && y > 24.0;
                let ball = (x - 18.0).hypot(y - 34.0).hypot(z - 24.0) <= 4.0;
                let value = match (tissue, tube || sheet || ball) {
                    (false, _) => 0.0,
                    (true, true) => 25.0,
                    (true, false) => 100.0,
                };
                values.push(value);
            }
        }
    }
    let dims = [SIDE; 3];
    Volume::new(dims, values, Affine::identity()).expect("a volume")
}

fn at(ids: &[u32], [x, y, z]: [u32; 3]) -> u32 {
    ids[((z * SIDE + y) * SIDE + x) as usize]
}

#[test]
fn finds_the_tube_and_not_the_sheet_the_ball_or_the_air() {
    let grid = veins(&made_up_head(), &|| false).expect("tubes looked for");
    assert_eq!(grid.dims, [SIDE; 3]);
    let ids = &grid.ids;
    let tube: u32 = (12..36).map(|z| at(ids, [16, 16, z])).sum();
    assert!(tube >= 20, "the tube's middle is found: {tube} of 24");
    let sheet: u32 = (12..36).map(|z| at(ids, [32, 36, z])).sum();
    assert_eq!(sheet, 0, "nothing in the middle of the sheet");
    assert_eq!(
        at(ids, [18, 34, 24]),
        0,
        "nothing in the middle of the ball"
    );
    assert_eq!(at(ids, [2, 2, 2]), 0, "nothing in the air");
    let found: u32 = ids.iter().sum();
    assert!(found < 600, "the tube and little else: {found} voxels");
}

#[test]
fn a_scan_of_one_slice_or_nothing_bright_is_refused() {
    let flat = Volume::new([8, 8, 1], vec![1.0; 64], Affine::identity()).expect("a volume");
    assert_eq!(veins(&flat, &|| false), Err(VeinsError::TooThin));
    let dark = Volume::new([8, 8, 8], vec![0.0; 512], Affine::identity()).expect("a volume");
    assert_eq!(veins(&dark, &|| false), Err(VeinsError::Blank));
}

#[test]
fn stopping_ends_the_work() {
    assert_eq!(veins(&made_up_head(), &|| true), Err(VeinsError::Stopped));
}

#[test]
fn the_table_names_one_approximate_part() {
    let table = veins_table();
    assert_eq!(table.entries.len(), 1);
    assert_eq!(table.entries[0].name, VEINS_PART);
    assert_eq!(table.entries[0].ids, vec![1]);
    assert!(matches!(
        table.entries[0].trust,
        volsa_labels::Trust::Approximate(_)
    ));
}

#[test]
fn eigenvalues_of_known_matrices() {
    let diagonal = eigenvalues([3.0, -1.0, 2.0, 0.0, 0.0, 0.0]);
    let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4);
    assert!(close(diagonal, [3.0, 2.0, -1.0]), "{diagonal:?}");
    // [[2,1,0],[1,2,0],[0,0,5]] has eigenvalues 5, 3 and 1.
    let mixed = eigenvalues([2.0, 2.0, 5.0, 1.0, 0.0, 0.0]);
    assert!(close(mixed, [5.0, 3.0, 1.0]), "{mixed:?}");
    assert_eq!(eigenvalues([1.0, 1.0, 1.0, 0.0, 0.0, 0.0]), [1.0; 3]);
}

#[test]
fn only_a_dark_tube_scores() {
    assert!(dark_tube([0.0, 1.0, 1.0], 0.2) > 200, "dark tube");
    assert_eq!(dark_tube([0.0, -1.0, -1.0], 0.2), 0, "bright tube");
    assert!(dark_tube([0.0, 0.05, 1.0], 0.2) < 20, "sheet");
    assert!(dark_tube([1.0, 1.0, 1.0], 0.2) < 50, "blob");
}

#[test]
fn a_kernel_sums_to_one() {
    let weights = kernel(1.5);
    let total = weights[0] + 2.0 * weights[1..].iter().sum::<f32>();
    assert!((total - 1.0).abs() < 1e-5);
    assert_eq!(kernel(0.1), vec![1.0]);
}

#[test]
fn small_pieces_go_and_large_ones_stay() {
    let sides = [6, 6, 6];
    let mut mask = vec![0_u8; 216];
    mask[0] = 1;
    // A line of five voxels, joined only at their corners.
    for step in 1..6 {
        mask[(step * 36) + step * 6 + step] = 1;
    }
    drop_small(&mut mask, sides, 4.0).expect("room");
    assert_eq!(mask.iter().map(|voxel| u32::from(*voxel)).sum::<u32>(), 6);
    drop_small(&mut mask, sides, 7.0).expect("room");
    assert!(mask.iter().all(|voxel| *voxel == 0));
}

#[test]
fn a_scan_with_no_tube_says_none_were_found() {
    let bright =
        Volume::new([12, 12, 12], vec![100.0; 1728], Affine::identity()).expect("a volume");
    assert_eq!(veins(&bright, &|| false), Err(VeinsError::NoneFound));
}

/// Labels of a cube of 4 voxels 2 mm wide, all one part, "brain".
fn coarse_labels() -> volsa_labels::LabelSet {
    let placement =
        Affine::from_columns([2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0], [0.0; 3]);
    volsa_labels::LabelSet {
        dims: [4; 3],
        numbers: vec![1; 64],
        placement,
        parts: vec![volsa_labels::Part {
            name: "brain".to_owned(),
            group: String::new(),
            side: volsa_labels::Side::Midline,
            colour: [200; 3],
            trust: volsa_labels::Trust::NotSaid,
            voxels: 64,
            ml: 0.512,
        }],
        title: "Made-up".to_owned(),
        verdict: None,
        made: None,
    }
}

/// A line of tube 1 voxel thin, on a grid of 1 mm voxels, along x at y = 2, z = 2.
fn fine_tube() -> volsa_labels::LabelGrid {
    let mut ids = vec![0_u32; 512];
    for x in 0..8 {
        ids[(2 * 8 + 2) * 8 + x] = 1;
    }
    volsa_labels::LabelGrid {
        dims: [8; 3],
        ids,
        placement: Affine::identity(),
    }
}

#[test]
fn tubes_are_put_onto_the_labels_open_as_one_more_part_and_counted() {
    let set = super::veins_onto(&coarse_labels(), &fine_tube()).expect("put on");
    let names: Vec<&str> = set.parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, ["brain", VEINS_PART]);
    // The labels' voxel at x 0..4, y 1, z 1 holds the tube's line (at 2 mm), however thin.
    for x in 0..4 {
        assert_eq!(set.numbers[(4 + 1) * 4 + x], 2, "voxel {x} of the line");
    }
    assert_eq!(set.parts[1].voxels, 4);
    assert_eq!(set.parts[0].voxels, 60, "the brain is counted again");
    assert!((set.parts[1].ml - 0.032).abs() < 1e-9);
    let again = super::veins_onto(&set, &fine_tube()).expect("put on again");
    assert_eq!(again.parts.len(), 2, "taken over, not added twice");
    assert_eq!(again.numbers, set.numbers);
}

#[test]
fn tubes_outside_the_labels_box_are_none_found() {
    let mut far = fine_tube();
    far.placement = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [100.0; 3],
    );
    assert_eq!(
        super::veins_onto(&coarse_labels(), &far),
        Err(VeinsError::NoneFound)
    );
}
