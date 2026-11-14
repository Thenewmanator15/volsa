//! Where a segmentation's grid lies: oblique frames, frames stored out of order or with whole
//! slices left out, and frames that do not lie on one evenly spaced grid.

use volsa_core::Point;

use super::tests::{frame, mask};
use super::*;
use crate::testkit::{SegFrame, SegOptions, SegSegment, segmentation};

fn one_segment(frames: Vec<SegFrame>) -> SegOptions {
    SegOptions {
        segments: vec![SegSegment {
            number: 1,
            label: "Part".to_owned(),
            lab: None,
        }],
        frames,
        ..SegOptions::default()
    }
}

fn at_heights(heights: &[f64]) -> SegOptions {
    let frames = heights.iter().map(|z| frame(*z, 1, &[0])).collect();
    one_segment(frames)
}

fn read_ok(opts: &SegOptions) -> Segmentation {
    read_segmentation(&segmentation(opts)).expect("the made-up segmentation is read")
}

fn refusal(opts: &SegOptions) -> String {
    read_segmentation(&segmentation(opts))
        .expect_err("refused")
        .0
}

fn close(a: Point<Patient>, b: [f64; 3]) -> bool {
    (a.x - b[0]).abs() < 1e-6 && (a.y - b[1]).abs() < 1e-6 && (a.z - b[2]).abs() < 1e-6
}

#[test]
fn an_oblique_grid_puts_each_voxel_centre_where_its_frame_says() {
    // Columns run along (0.6, 0.8, 0) and rows straight down; slices step 3 mm along the
    // normal, which is (−0.8, 0.6, 0).
    let start = [10.0, 20.0, 30.0];
    let normal = [-0.8, 0.6, 0.0];
    let at = |slice: f64| [0, 1, 2].map(|axis| start[axis] + normal[axis] * 3.0 * slice);
    let frames = (0..3)
        .map(|slice| SegFrame {
            position: at(f64::from(slice)),
            segment: 1,
            mask: mask(&[usize::try_from(slice).expect("small")]),
        })
        .collect();
    let opts = SegOptions {
        orientation: [0.6, 0.8, 0.0, 0.0, 0.0, -1.0],
        pixel_spacing: [2.0, 0.5],
        ..one_segment(frames)
    };
    let read = read_ok(&opts);
    assert_eq!(read.dims, [5, 3, 3]);
    for slice in 0..3 {
        let origin = read.placement.apply(Point::new(0.0, 0.0, f64::from(slice)));
        assert!(
            close(origin, at(f64::from(slice))),
            "slice {slice}: {origin:?}"
        );
    }
    // Column 4, row 2 of slice 1: 4 × 0.5 mm across, 2 × 2 mm down.
    let [x, y, z] = at(1.0);
    let voxel = read.placement.apply(Point::new(4.0, 2.0, 1.0));
    assert!(close(voxel, [x + 1.2, y + 1.6, z - 4.0]), "{voxel:?}");
    assert_eq!(read.numbers.iter().filter(|n| **n == 1).count(), 3);
    assert_eq!(read.numbers.get(15 + 1), Some(&1));
}

#[test]
fn frames_stored_out_of_order_go_to_their_own_slices() {
    let opts = one_segment(vec![frame(5.0, 1, &[2]), frame(0.0, 1, &[1])]);
    let read = read_ok(&opts);
    assert_eq!(read.dims, [5, 3, 2]);
    assert_eq!(read.numbers.get(1), Some(&1));
    assert_eq!(read.numbers.get(15 + 2), Some(&1));
    let origin = read.placement.apply(Point::new(0.0, 0.0, 0.0));
    assert!(close(origin, [-10.0, -20.0, 0.0]), "{origin:?}");
}

#[test]
fn whole_slices_with_no_frame_are_kept_empty() {
    // Frames only where something is marked: 0, 2 and 8 mm, every 2 mm, five slices.
    let read = read_ok(&at_heights(&[0.0, 2.0, 8.0]));
    assert_eq!(read.dims, [5, 3, 5]);
    let marked: Vec<usize> = (0..5)
        .filter(|s| read.numbers.get(s * 15) == Some(&1))
        .collect();
    assert_eq!(marked, vec![0, 1, 4]);
    let top = read.placement.apply(Point::new(0.0, 0.0, 4.0));
    assert!(close(top, [-10.0, -20.0, 8.0]), "{top:?}");
}

#[test]
fn unevenly_spaced_frames_are_refused_in_a_sentence() {
    let why = refusal(&at_heights(&[0.0, 5.0, 12.0]));
    assert_eq!(
        why,
        "The slices of this segmentation are not evenly spaced, so Volsa does not open it as \
         labels."
    );
}

#[test]
fn frames_shifted_within_their_plane_are_refused() {
    // With three frames the one in the middle cannot be on the line through the other two.
    let mut opts = at_heights(&[0.0, 2.0, 4.0]);
    if let Some(second) = opts.frames.get_mut(1) {
        second.position[0] += 3.0;
    }
    assert!(refusal(&opts).contains("do not lie on one grid"));
}

#[test]
fn one_frame_steps_by_the_spacing_the_file_states() {
    let opts = SegOptions {
        spacing_between: Some(1.5),
        ..at_heights(&[4.0])
    };
    let read = read_ok(&opts);
    assert_eq!(read.dims, [5, 3, 1]);
    let next = read.placement.apply(Point::new(0.0, 0.0, 1.0));
    assert!(close(next, [-10.0, -20.0, 5.5]), "{next:?}");
}

#[test]
fn a_grid_too_large_is_refused_before_room_is_made_for_it() {
    let why = refusal(&at_heights(&[0.0, 0.01, 1_000_000.0]));
    assert_eq!(why, "This segmentation is larger than Volsa will read.");
}
