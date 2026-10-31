//! The brightness of what a network is given: statistics gathered a slab at a time, and the
//! four scalings.

use super::{Scale, ScaleError, gather, outside_value, percentile};
use crate::grid::tests::{input, volume_of};
use crate::{Grid, Outside, Over, Scaling, Voxels};

/// A ramp of a million values, 0 to 999 999, in a 100 x 100 x 100 scan.
fn ramp() -> voxa_core::Volume {
    volume_of([100, 100, 100], |col, row, slice| {
        (col + 100 * row + 10_000 * slice) as f32
    })
}

#[test]
fn percentiles_of_a_ramp_are_within_a_bin_of_numpys() {
    let volume = ramp();
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    assert_eq!(stats.count, 1_000_000);
    assert_eq!((stats.lowest, stats.highest), (0.0, 999_999.0));
    let bin = 999_999.0 / 65_536.0;
    // NumPy's percentiles of the same values, worked by hand: rank p/100 x (n - 1).
    assert!((percentile(&stats, 2.0) - 19_999.98).abs() <= bin);
    assert!((percentile(&stats, 98.0) - 979_999.02).abs() <= bin);
    assert_eq!(percentile(&stats, 0.0), 0.0);
    assert!((percentile(&stats, 100.0) - 999_999.0).abs() <= bin);
}

#[test]
fn percentiles_scale_and_hold_between_nought_and_one() {
    let volume = ramp();
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    let scaling = Scaling::Percentiles {
        low: 0.0,
        high: 100.0,
        over: Over::Scan,
    };
    let scale = Scale::new(&scaling, &stats).expect("a scale");
    let mut values = [-5.0, 0.0, 499_999.5, 999_999.0, 2e6];
    scale.apply(&mut values);
    assert_eq!(values[0], 0.0);
    assert_eq!(values[1], 0.0);
    assert!((values[2] - 0.5).abs() < 1e-4);
    assert!((values[3] - 1.0).abs() < 1e-4);
    assert_eq!(values[4], 1.0);
}

#[test]
fn z_score_above_zero_ignores_the_air() {
    // Half the scan is air at 0; the rest is 10 and 30, so their mean is 20 and their
    // standard deviation 10.
    let volume = volume_of([4, 4, 4], |col, row, _| match (col < 2, row % 2) {
        (true, _) => 0.0,
        (false, 0) => 10.0,
        (false, _) => 30.0,
    });
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::AboveZero).expect("gathered");
    assert_eq!(stats.count, 32);
    assert!((stats.mean - 20.0).abs() < 1e-9);
    assert!((stats.sd - 10.0).abs() < 1e-9);
    let scaling = Scaling::ZScore {
        voxels: Voxels::AboveZero,
        over: Over::Scan,
    };
    let mut values = [30.0, 10.0, 0.0];
    Scale::new(&scaling, &stats)
        .expect("a scale")
        .apply(&mut values);
    assert_eq!(values, [1.0, -1.0, -2.0]);
}

#[test]
fn fixed_clips_then_subtracts_and_divides() {
    let volume = ramp();
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    let scaling = Scaling::Fixed {
        clip: [-1024.0, 276.0],
        subtract: -370.0,
        divide: 436.6,
    };
    let mut values = [-3000.0, -370.0, 66.6, 5000.0];
    Scale::new(&scaling, &stats)
        .expect("a scale")
        .apply(&mut values);
    let expected = [(-1024.0 + 370.0) / 436.6, 0.0, 1.0, (276.0 + 370.0) / 436.6];
    for (value, expected) in values.iter().zip(expected) {
        assert!(
            (value - expected).abs() < 1e-5,
            "{value} against {expected}"
        );
    }
}

#[test]
fn none_leaves_values_as_stored() {
    let volume = ramp();
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    let mut values = [-3000.0, 7.5];
    Scale::new(&Scaling::None, &stats)
        .expect("a scale")
        .apply(&mut values);
    assert_eq!(values, [-3000.0, 7.5]);
}

#[test]
fn over_the_grid_counts_the_surroundings_at_the_lowest_value() {
    let volume = volume_of([2, 2, 2], |col, _, _| if col == 0 { 4.0 } else { 8.0 });
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let region = grid.whole_box([4, 2, 2]);
    let stats = gather(&volume, &grid, Some(region), Voxels::All).expect("gathered");
    // Eight voxels of the scan and eight of surroundings, at the lowest value, 4.
    assert_eq!(stats.count, 16);
    assert!((stats.mean - 5.0).abs() < 1e-9);
    assert_eq!(percentile(&stats, 50.0), 4.0);
}

#[test]
fn a_flat_scan_is_refused_in_words() {
    let words = "This series holds no values that differ, so there is nothing in it for the \
                 plugin to see.";
    let flat = volume_of([4, 4, 4], |_, _, _| 3.0);
    let grid = Grid::of(&flat, &input("LPS", [1.0; 3]));
    let stats = gather(&flat, &grid, None, Voxels::All).expect("gathered");
    let percentiles = Scaling::Percentiles {
        low: 2.0,
        high: 98.0,
        over: Over::Scan,
    };
    let z = Scaling::ZScore {
        voxels: Voxels::All,
        over: Over::Scan,
    };
    for scaling in [percentiles, z] {
        let refused = Scale::new(&scaling, &stats).expect_err("flat");
        assert_eq!(refused, ScaleError::Flat);
        assert_eq!(refused.to_string(), words);
    }
    // All but two voxels the same: the 2nd and 98th percentiles are equal.
    let nearly = volume_of([10, 10, 10], |col, row, slice| match (col, row, slice) {
        (0, 0, 0) => 0.0,
        (9, 9, 9) => 100.0,
        _ => 50.0,
    });
    let grid = Grid::of(&nearly, &input("LPS", [1.0; 3]));
    let stats = gather(&nearly, &grid, None, Voxels::All).expect("gathered");
    assert_eq!(
        Scale::new(&percentiles, &stats).expect_err("flat"),
        ScaleError::Flat
    );
    let air = volume_of([4, 4, 4], |_, _, _| 0.0);
    let grid = Grid::of(&air, &input("LPS", [1.0; 3]));
    let stats = gather(&air, &grid, None, Voxels::AboveZero).expect("gathered");
    let above = Scaling::ZScore {
        voxels: Voxels::AboveZero,
        over: Over::Scan,
    };
    assert_eq!(
        Scale::new(&above, &stats).expect_err("flat"),
        ScaleError::Flat
    );
}

#[test]
fn zero_outside_becomes_zero_after_scaling() {
    let volume = ramp();
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    assert_eq!(outside_value(Outside::Lowest, &stats), 0.0);
    let outside = outside_value(Outside::Zero, &stats);
    assert!(outside.is_nan());
    let scaling = Scaling::Fixed {
        clip: [0.0, 10.0],
        subtract: 5.0,
        divide: 5.0,
    };
    let mut values = [outside, 10.0];
    Scale::new(&scaling, &stats)
        .expect("a scale")
        .apply(&mut values);
    assert_eq!(values, [0.0, 1.0]);
}

#[test]
fn gathering_a_long_scan_holds_one_slab() {
    let volume = volume_of([8, 6, 900], |_, _, slice| slice as f32);
    let grid = Grid::of(&volume, &input("SPL", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    assert_eq!(stats.count, 8 * 6 * 900);
    // The grid's first axis runs along the slices: a slab is one slice of 8 x 6.
    assert_eq!(stats.largest_slab, 6 * 8);
}

#[test]
fn percentiles_are_numpys_exactly_when_each_value_has_its_own_bin() {
    let volume = volume_of([100, 1, 1], |col, _, _| col as f32);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let stats = gather(&volume, &grid, None, Voxels::All).expect("gathered");
    // NumPy: rank 0.02 x 99 = 1.98, between the values 1 and 2.
    assert!((percentile(&stats, 2.0) - 1.98).abs() < 1e-9);
    assert!((percentile(&stats, 98.0) - 97.02).abs() < 1e-9);
    assert!((percentile(&stats, 50.0) - 49.5).abs() < 1e-9);
}

#[test]
fn above_zero_the_surroundings_are_the_scans_lowest_and_are_not_counted() {
    // Air at 0 and tissue at 4 and 8; the box is twice as long as the scan.
    let volume = volume_of([2, 2, 2], |col, row, _| match (col, row) {
        (0, 0) => 0.0,
        (0, _) => 4.0,
        _ => 8.0,
    });
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let region = grid.whole_box([4, 2, 2]);
    let stats = gather(&volume, &grid, Some(region), Voxels::AboveZero).expect("gathered");
    // The six voxels above 0 count; the air and the eight voxels around the scan do not.
    assert_eq!(stats.count, 6);
    assert!((stats.mean - 20.0 / 3.0).abs() < 1e-9, "{}", stats.mean);
    // Around the scan is the scan's lowest value, 0, not the lowest counted, 4.
    assert_eq!(outside_value(Outside::Lowest, &stats), 0.0);
}
