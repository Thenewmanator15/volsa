//! Blocks laid over the grid as nnU-Net lays them, and each voxel given to the block whose
//! centre is nearest.

use proptest::prelude::*;

use super::BlockPlan;
use crate::Region;

/// The plan of blocks of `size`, `step` apart, over a grid of `grid`.
fn plan(grid: [usize; 3], size: [usize; 3], step: [usize; 3]) -> BlockPlan {
    BlockPlan { grid, size, step }
}

/// Which block owns each voxel of the grid, by its place among the starts; `usize::MAX` for
/// none, and a voxel owned twice is a failure.
fn owners(plan: &BlockPlan) -> Vec<usize> {
    let [a, b, c] = plan.grid;
    let mut owner = vec![usize::MAX; a * b * c];
    for (place, start) in plan.starts().into_iter().enumerate() {
        let Region { from, size } = plan.owned(start);
        let from = from.map(|from| usize::try_from(from).expect("inside the grid"));
        for x in from[0]..from[0] + size[0] {
            for y in from[1]..from[1] + size[1] {
                for z in from[2]..from[2] + size[2] {
                    let slot = &mut owner[(x * b + y) * c + z];
                    assert_eq!(*slot, usize::MAX, "voxel {x} {y} {z} owned twice");
                    *slot = place;
                }
            }
        }
    }
    owner
}

#[test]
fn blocks_cover_the_grid_and_the_last_lies_against_the_end() {
    let plan = plan([10, 4, 7], [4, 4, 4], [4, 2, 3]);
    let starts = plan.starts();
    assert_eq!(plan.count(), 6);
    assert_eq!(starts.len(), 6);
    assert_eq!(starts[0], [0, 0, 0]);
    assert_eq!(starts[1], [0, 0, 3]);
    assert_eq!(starts[5], [6, 0, 3]);
}

#[test]
fn a_grid_smaller_than_a_block_is_one_block() {
    let plan = plan([3, 3, 3], [8, 8, 8], [4, 4, 4]);
    assert_eq!(plan.starts(), [[0, 0, 0]]);
    let owned = plan.owned([0, 0, 0]);
    assert_eq!(
        owned,
        Region {
            from: [0; 3],
            size: [3, 3, 3]
        }
    );
}

#[test]
fn ties_go_to_the_earlier_block() {
    // Centres at 1, 3 and 4: voxel 2 is as near the first as the second, and is the first's.
    let plan = plan([6, 1, 1], [3, 1, 1], [2, 1, 1]);
    assert_eq!(plan.starts(), [[0, 0, 0], [2, 0, 0], [3, 0, 0]]);
    assert_eq!(owners(&plan), [0, 0, 0, 1, 2, 2]);
}

#[test]
fn a_voxel_is_owned_by_the_block_whose_centre_is_nearest() {
    let cases = [
        plan([9, 7, 5], [4, 3, 5], [2, 2, 1]),
        plan([12, 5, 8], [5, 5, 3], [5, 1, 2]),
        plan([4, 9, 6], [4, 4, 4], [1, 3, 4]),
    ];
    for plan in cases {
        let starts = plan.starts();
        let [_, b, c] = plan.grid;
        let owned = owners(&plan);
        for (voxel, owner) in owned.iter().enumerate() {
            let at = [voxel / (b * c), (voxel / c) % b, voxel % c];
            let distance = |start: &[usize; 3]| -> f64 {
                (0..3)
                    .map(|axis| {
                        let centre = start[axis] as f64 + (plan.size[axis] as f64 - 1.0) / 2.0;
                        (at[axis] as f64 - centre).powi(2)
                    })
                    .sum()
            };
            let nearest = (0..starts.len())
                .min_by(|x, y| distance(&starts[*x]).total_cmp(&distance(&starts[*y])))
                .expect("a block");
            assert_eq!(*owner, nearest, "voxel {at:?}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn every_voxel_is_owned_by_exactly_one_block(
        grid in prop::array::uniform3(1_usize..40),
        size in prop::array::uniform3(1_usize..12),
        steps in prop::array::uniform3(1_usize..12),
    ) {
        let step = [0, 1, 2].map(|axis| steps[axis].min(size[axis]));
        let plan = plan(grid, size, step);
        let owned = owners(&plan);
        prop_assert!(owned.iter().all(|owner| *owner != usize::MAX));
        for start in plan.starts() {
            for axis in 0..3 {
                prop_assert!(start[axis] + size[axis].min(grid[axis]) <= grid[axis]);
            }
        }
    }
}
