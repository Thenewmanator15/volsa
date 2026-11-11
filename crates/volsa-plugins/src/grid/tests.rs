//! The plugin's grid: its size, the values sampled into it, and the boxes cut from it.

use serde_json::json;
use volsa_core::{Affine, Volume};

use super::{Grid, Region, overhang_mm, sample};
use crate::Input;
use crate::describe::tests::{example, read};

/// What lies outside the scan, in these tests.
const OUTSIDE: f32 = -7.0;

/// A volume of `dims` voxels, 1 mm apart along the patient's own axes, valued by `value`.
pub(crate) fn volume_of(dims: [u32; 3], value: impl Fn(u32, u32, u32) -> f32) -> Volume {
    let placement =
        Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    placed(dims, placement, value)
}

/// A volume of `dims` voxels with this placement, valued by `value`.
pub(crate) fn placed(
    dims: [u32; 3],
    placement: Affine<volsa_core::Voxel, volsa_core::Patient>,
    value: impl Fn(u32, u32, u32) -> f32,
) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            for col in 0..cols {
                values.push(value(col, row, slice));
            }
        }
    }
    Volume::new(dims, values, placement).expect("a volume")
}

/// The example's input with these axes and spacing.
pub(crate) fn input(axes: &str, spacing: [f64; 3]) -> Input {
    let mut value = example();
    value["input"]["axes"] = json!(axes);
    value["input"]["spacing_mm"] = json!(spacing);
    read(&value).expect("reads").input
}

#[test]
fn a_one_mm_scan_at_one_mm_samples_its_own_values() {
    let volume = volume_of([4, 5, 6], |col, row, slice| {
        (col + 10 * row + 100 * slice) as f32
    });
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    assert_eq!(grid.size, [4, 5, 6]);
    let values = sample(&volume, &grid, grid.whole(), OUTSIDE).expect("sampled");
    for a in 0..4 {
        for b in 0..5 {
            for c in 0..6 {
                let at = (a * 5 + b) * 6 + c;
                let expected = (a + 10 * b + 100 * c) as f32;
                assert_eq!(values[at], expected, "at {a} {b} {c}");
            }
        }
    }
}

#[test]
fn halving_the_spacing_doubles_the_sides_and_interpolates() {
    let volume = volume_of([4, 4, 4], |col, _, _| col as f32);
    let grid = Grid::of(&volume, &input("LPS", [0.5, 1.0, 1.0]));
    assert_eq!(grid.size, [8, 4, 4]);
    let values = sample(&volume, &grid, grid.whole(), OUTSIDE).expect("sampled");
    let along: Vec<f32> = (0..8).map(|a| values[a * 16]).collect();
    assert_eq!(along, [0.0, 0.25, 0.75, 1.25, 1.75, 2.25, 2.75, 3.0]);
}

#[test]
fn a_grid_turned_round_reads_from_the_far_end() {
    let volume = volume_of([4, 1, 1], |col, _, _| col as f32);
    let grid = Grid::of(&volume, &input("RPS", [1.0; 3]));
    let values = sample(&volume, &grid, grid.whole(), OUTSIDE).expect("sampled");
    assert_eq!(values, [3.0, 2.0, 1.0, 0.0]);
}

#[test]
fn a_box_larger_than_the_grid_is_surrounded_with_outside() {
    let volume = volume_of([4, 4, 4], |_, _, _| 1.0);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let region = grid.whole_box([8, 8, 8]);
    assert_eq!(region.from, [-2, -2, -2]);
    let values = sample(&volume, &grid, region, OUTSIDE).expect("sampled");
    assert_eq!(values.len(), 512);
    let outside = values.iter().filter(|value| **value == OUTSIDE).count();
    assert_eq!(outside, 512 - 64);
    assert_eq!(values[(2 * 8 + 2) * 8 + 2], 1.0);
    assert_eq!(overhang_mm(&grid, region), None);
}

#[test]
fn a_box_smaller_along_one_axis_and_larger_along_another_cuts_and_surrounds() {
    let volume = volume_of([10, 4, 4], |col, _, _| col as f32);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let region = grid.whole_box([6, 8, 8]);
    assert_eq!(region.from, [2, -2, -2]);
    let values = sample(&volume, &grid, region, OUTSIDE).expect("sampled");
    // Inside the scan along the other two axes, the box's first voxel is the scan's third.
    assert_eq!(values[(2 * 8) + 2], 2.0);
    assert_eq!(values[((5 * 8) + 2) * 8 + 2], 7.0);
    assert_eq!(values[0], OUTSIDE);
    assert_eq!(overhang_mm(&grid, region), Some(2.0));
}

#[test]
fn sampling_a_region_asks_only_for_its_own_voxels() {
    let volume = volume_of([32, 32, 2000], |col, row, slice| (col + row + slice) as f32);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let region = Region {
        from: [8, 8, 1000],
        size: [16, 16, 16],
    };
    let values = sample(&volume, &grid, region, OUTSIDE).expect("sampled");
    assert_eq!(values.len(), 16 * 16 * 16);
    assert_eq!(values[0], (8 + 8 + 1000) as f32);
}

#[test]
fn a_scan_of_thick_slices_has_fewer_voxels_at_one_mm_than_stored() {
    let placement =
        Affine::from_columns([0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 5.0], [0.0; 3]);
    let volume = placed([100, 80, 12], placement, |_, _, _| 0.0);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    assert_eq!(grid.size, [50, 40, 60]);
}
