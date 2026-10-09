use egui::{Rect, pos2};

use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};
use crate::testing::{THICK_SCAN, id, three_series};

fn area() -> Rect {
    Rect::from_min_max(pos2(240.0, 40.0), pos2(1040.0, 640.0))
}

#[test]
fn one_viewport_takes_the_whole_area() {
    assert_eq!(grid(area(), Layout::One), vec![area()]);
}

#[test]
fn two_viewports_sit_side_by_side_with_a_gap_between() {
    let cells = grid(area(), Layout::TwoAcross);
    assert_eq!(cells.len(), 2);
    assert_eq!((cells[0].min, cells[1].max), (area().min, area().max));
    assert_eq!(cells[0].width(), cells[1].width());
    assert_eq!(cells[1].min.x - cells[0].max.x, GAP);
    assert_eq!(cells[0].height(), area().height());
}

#[test]
fn four_viewports_make_two_rows_of_two() {
    let cells = grid(area(), Layout::TwoByTwo);
    assert_eq!(cells.len(), 4);
    assert_eq!((cells[0].min, cells[3].max), (area().min, area().max));
    assert_eq!(cells[0].size(), cells[3].size());
    assert_eq!(cells[2].min.y - cells[0].max.y, GAP);
    assert_eq!(
        (cells[1].min.y, cells[2].min.x),
        (cells[0].min.y, cells[0].min.x)
    );
}

fn showing() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id(THICK_SCAN),
        },
    );
    state
}

#[test]
fn a_viewport_says_what_it_is_waiting_for() {
    let state = showing();
    assert_eq!(message(&state, 0, false).as_deref(), Some("Loading…"));
    assert_eq!(
        message(&state, 0, true),
        None,
        "the slice is there to be drawn"
    );
    assert_eq!(
        message(&state, 1, false).as_deref(),
        Some("No series is shown here")
    );
    assert_eq!(message(&state, 9, false), None, "there is no such viewport");
}

#[test]
fn a_slice_that_failed_says_so_with_the_reason() {
    let mut state = showing();
    let failed = Action::FrameFailed {
        series: id(THICK_SCAN),
        slice: 15,
        reason: "The image could not be decoded: the file is damaged or cut short.".to_owned(),
    };
    reduce(&mut state, failed);
    assert_eq!(
        message(&state, 0, false).as_deref(),
        Some(
            "This slice could not be decoded\n\
             The image could not be decoded: the file is damaged or cut short."
        )
    );
}

#[test]
fn the_pixels_of_a_series_are_as_tall_as_its_rows_are_apart() {
    let mut state = showing();
    assert_eq!(pixel_aspect(&state, 0), 1.0);
    let index = state.index.as_mut().expect("indexed");
    index.series[0].info.spacing_mm = [0.5, 1.5, 5.0];
    assert_eq!(pixel_aspect(&state, 0), 3.0);
    index_spacing(&mut state, [0.0, 1.0, 1.0]);
    assert_eq!(
        pixel_aspect(&state, 0),
        1.0,
        "a spacing of nothing is not believed"
    );
    assert_eq!(pixel_aspect(&state, 3), 1.0, "nothing shown");
}

fn index_spacing(state: &mut AppState, spacing: [f64; 3]) {
    let index = state.index.as_mut().expect("indexed");
    index.series[0].info.spacing_mm = spacing;
}

#[test]
fn the_position_is_marked_by_four_short_lines_that_leave_the_point_itself_clear() {
    let lines = annotate::position_mark([100.0, 50.0]);
    assert_eq!(lines[0], [[88.0, 50.0], [96.0, 50.0]], "to the left of it");
    assert_eq!(lines[3], [[100.0, 54.0], [100.0, 62.0]], "below it");
    for [[ax, ay], [bx, by]] in lines {
        let level = ay == 50.0 && by == 50.0;
        let upright = ax == 100.0 && bx == 100.0;
        assert!(
            level != upright,
            "each lies along the row or the column of the point"
        );
        let nearest =
            (ax - 100.0).abs().min((bx - 100.0).abs()) + (ay - 50.0).abs().min((by - 50.0).abs());
        assert_eq!(nearest, 4.0, "and stops short of it");
    }
}
