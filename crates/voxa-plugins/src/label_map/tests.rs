//! Blocks' labels put together into one map, and the map made a label set on the scan.

use voxa_core::Point;
use voxa_labels::{Side as LabelSide, Trust};

use super::LabelMap;
use crate::finish::FinishError;
use crate::grid::tests::{input, volume_of};
use crate::{BlockPlan, Gives, Grid, LabelOutput, Name, Region, Side, sample};

/// An output naming the labels 1 to `count`, keeping the largest piece or not.
fn named(count: u16, keep_largest_piece: bool) -> LabelOutput {
    let names = (1..=count)
        .map(|id| Name {
            id,
            name: format!("part {id}"),
            side: if id == 1 { Side::Left } else { Side::Midline },
            group: "Parts".to_owned(),
            colour: (id == 1).then_some([1, 2, 3]),
        })
        .collect();
    LabelOutput {
        gives: Gives::Labels,
        from: None,
        keep_largest_piece,
        names,
    }
}

/// A start as a place in the grid.
fn signed(start: [usize; 3]) -> [isize; 3] {
    start.map(|side| isize::try_from(side).expect("small"))
}

#[test]
fn blocks_put_together_give_each_voxel_its_owners_label() {
    let plan = BlockPlan {
        grid: [6, 5, 4],
        size: [4, 4, 4],
        step: [2, 2, 2],
    };
    let whole = Region {
        from: [0; 3],
        size: plan.grid,
    };
    let mut map = LabelMap::new(whole).expect("a map");
    let starts = plan.starts();
    for (place, start) in starts.iter().enumerate() {
        let id = u16::try_from(place + 1).expect("few blocks");
        let block = vec![id; 64];
        map.put(signed(*start), plan.size, plan.owned(*start), &block);
    }
    for (place, start) in starts.iter().enumerate() {
        let owned = plan.owned(*start);
        let from = owned
            .from
            .map(|side| usize::try_from(side).expect("inside"));
        for x in from[0]..from[0] + owned.size[0] {
            for y in from[1]..from[1] + owned.size[1] {
                for z in from[2]..from[2] + owned.size[2] {
                    let id = map.ids[(x * 5 + y) * 4 + z];
                    assert_eq!(usize::from(id), place + 1, "voxel {x} {y} {z}");
                }
            }
        }
    }
}

#[test]
fn the_largest_piece_is_kept_when_asked_and_the_names_make_the_table() {
    let volume = volume_of([8, 1, 1], |_, _, _| 0.0);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let whole = grid.whole();
    // Two pieces: three voxels, then one.
    let labels = [1, 1, 2, 0, 0, 0, 1, 0];
    let mut map = LabelMap::new(whole).expect("a map");
    map.put([0; 3], [8, 1, 1], whole, &labels);
    let (kept, table) = map
        .finish(&named(2, true), "Example", &grid, &volume)
        .expect("kept");
    assert_eq!(kept.ids, [1, 1, 2, 0, 0, 0, 0, 0]);
    assert_eq!(table.title, "Example");
    assert_eq!(table.entries.len(), 2);
    assert_eq!(table.entries[0].name, "part 1");
    assert_eq!(table.entries[0].side, LabelSide::Left);
    assert_eq!(table.entries[0].ids, [1]);
    assert_eq!(table.entries[0].colour, [1, 2, 3]);
    assert_eq!(table.entries[0].trust, Trust::Model);
    assert_eq!(table.entries[1].colour, voxa_labels::colour_for(2));
    assert!(table.verdict.is_none() && table.made.is_none());
    let mut map = LabelMap::new(whole).expect("a map");
    map.put([0; 3], [8, 1, 1], whole, &labels);
    let (all, _) = map
        .finish(&named(2, false), "Example", &grid, &volume)
        .expect("all");
    assert_eq!(all.ids, [1, 1, 2, 0, 0, 0, 1, 0]);
}

#[test]
fn nothing_labelled_says_so() {
    let volume = volume_of([4, 4, 4], |_, _, _| 0.0);
    let grid = Grid::of(&volume, &input("LPS", [1.0; 3]));
    let map = LabelMap::new(grid.whole()).expect("a map");
    let refused = map
        .finish(&named(1, false), "Example", &grid, &volume)
        .expect_err("none");
    assert_eq!(refused, FinishError::Nothing);
    assert_eq!(
        refused.to_string(),
        "The plugin labelled nothing in this scan."
    );
}

#[test]
fn a_whole_box_result_lands_on_the_scan_as_sampled() {
    let bead = [7, 2, 1];
    let volume = volume_of([10, 4, 4], |col, row, slice| {
        f32::from(u8::from([col, row, slice] == bead))
    });
    let grid = Grid::of(&volume, &input("LIA", [1.0; 3]));
    let region = grid.whole_box([6, 8, 8]);
    let values = sample(&volume, &grid, region, 0.0).expect("sampled");
    let labels: Vec<u16> = values.iter().map(|value| u16::from(*value > 0.5)).collect();
    let mut map = LabelMap::new(region).expect("a map");
    map.put(region.from, region.size, region, &labels);
    let (grid_labels, _) = map
        .finish(&named(1, false), "Example", &grid, &volume)
        .expect("labels");
    let at = grid_labels
        .ids
        .iter()
        .position(|id| *id == 1)
        .expect("the bead");
    let [cols, rows, _] = grid_labels.dims.map(|side| side as usize);
    let voxel = Point::new(
        (at % cols) as f64,
        ((at / cols) % rows) as f64,
        (at / (cols * rows)) as f64,
    );
    let landed = grid_labels.placement.apply(voxel);
    let expected = volume.voxel_to_patient().apply(Point::new(7.0, 2.0, 1.0));
    assert!((landed.x - expected.x).abs() < 1e-9);
    assert!((landed.y - expected.y).abs() < 1e-9);
    assert!((landed.z - expected.z).abs() < 1e-9);
}
