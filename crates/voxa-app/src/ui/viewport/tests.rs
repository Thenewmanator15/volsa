use egui::{Rect, pos2, vec2};
use voxa_dicom::Frame;

use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};
use crate::testing::{THICK_SCAN, id, showing_a_scan_of, three_series};
use crate::ui::hand::Hand;

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

/// A viewport of 400 by 400 points at the top left of a window, showing a 4 x 4 slice whole.
const SIZE: [f32; 2] = [400.0, 400.0];

/// A 4 x 4 slice of zeros but for -12 in column 1 of row 2.
fn slice_with_a_dip() -> Frame {
    let mut values = vec![0.0; 16];
    values[2 * 4 + 1] = -12.0;
    Frame {
        rows: 4,
        cols: 4,
        values,
        colour: None,
    }
}

/// Where, in the viewport, column `col` of row `row` of the 4 x 4 slice lies when it is drawn
/// through `view`.
fn centre_of(view: &ViewTransform, col: f64, row: f64) -> [f32; 2] {
    let to_screen = view.image_to_screen([4, 4], SIZE, 1.0);
    let at = to_screen.apply(voxa_core::Point::new(col, row, 0.0));
    [narrowed(at.x), narrowed(at.y)]
}

/// The pane of the first viewport of `state`, drawn whole in the cell at the window's corner.
fn pane_of(state: &AppState) -> Pane<'_> {
    let cell = Rect::from_min_size(pos2(0.0, 0.0), vec2(SIZE[0], SIZE[1]));
    Pane {
        state,
        at: 0,
        cell,
        shown: ViewTransform::fit(),
    }
}

/// The lines of the bottom left corner of the viewport when the pointer is over it, with this
/// value under it.
fn bottom_left(state: &AppState, value: Option<f32>) -> Vec<String> {
    let pane = pane_of(state);
    let (info, study) = pane.series().expect("a scan is shown");
    let [_, _, bottom_left, _] = annotate::corners(&pane, info, study, value);
    bottom_left
}

#[test]
fn the_value_under_the_pointer_is_in_hu_for_ct() {
    let state = showing_a_scan_of("CT", None);
    let frame = slice_with_a_dip();
    let pointer = centre_of(&ViewTransform::fit(), 1.0, 2.0);
    let value = value::value_at(&frame, &ViewTransform::fit(), SIZE, 1.0, pointer);
    assert_eq!(value, Some(-12.0));
    let lines = bottom_left(&state, value);
    assert_eq!(lines.last().map(String::as_str), Some("Value -12 HU"));
    assert!(
        lines[0].starts_with("Slice 1 of 1"),
        "after the slice, not in place of it: {lines:?}"
    );
}

#[test]
fn a_non_hu_ct_value_has_no_unit() {
    for (rescale, said) in [
        (Some("US"), "Value -12 (no unit)"),
        (Some("MGML"), "Value -12 MGML"),
        (Some("HU"), "Value -12 HU"),
        (None, "Value -12 HU"),
    ] {
        let state = showing_a_scan_of("CT", rescale);
        let lines = bottom_left(&state, Some(-12.0));
        assert_eq!(lines.last().map(String::as_str), Some(said), "{rescale:?}");
    }
}

#[test]
fn a_value_of_any_other_scan_is_never_in_hu() {
    for modality in ["MR", "US", "PT", ""] {
        let state = showing_a_scan_of(modality, None);
        let lines = bottom_left(&state, Some(312.0));
        let said = Some("Value 312 (no unit)");
        assert_eq!(lines.last().map(String::as_str), said, "{modality:?}");
    }
    // Even when the file says HU, a scan that is not CT only names what the file says.
    let state = showing_a_scan_of("MR", Some("HU"));
    let lines = bottom_left(&state, Some(312.0));
    assert_eq!(lines.last().map(String::as_str), Some("Value 312 HU"));
}

#[test]
fn with_nothing_under_the_pointer_the_corner_says_no_value() {
    let state = showing_a_scan_of("CT", None);
    for value in [None, Some(f32::NAN)] {
        let lines = bottom_left(&state, value);
        assert!(
            lines.iter().all(|line| !line.starts_with("Value")),
            "{lines:?}"
        );
    }
}

#[test]
fn the_value_is_that_of_the_pixel_under_the_pointer_however_the_picture_is_placed() {
    // Each pixel holds its own place: 0 to 15, row by row.
    let frame = Frame {
        values: (0..16_u8).map(f32::from).collect(),
        ..slice_with_a_dip()
    };
    let fit = ViewTransform::fit();
    let zoomed = fit.zoomed_about(2.0, [30.0, -10.0]);
    let views = [
        fit,
        fit.flipped(true),
        fit.flipped(false),
        fit.turned_clockwise(),
        fit.turned_clockwise().flipped(true),
        zoomed,
        zoomed.turned_clockwise().turned_clockwise(),
    ];
    for view in views {
        for row in 0..4_u8 {
            for col in 0..4_u8 {
                let pointer = centre_of(&view, f64::from(col), f64::from(row));
                let got = value::value_at(&frame, &view, SIZE, 1.0, pointer);
                let wanted = Some(f32::from(row * 4 + col));
                assert_eq!(got, wanted, "{view:?} column {col}, row {row}");
            }
        }
    }
}

#[test]
fn off_the_picture_the_pointer_reads_nothing() {
    let frame = slice_with_a_dip();
    let fit = ViewTransform::fit();
    // The picture is square and so is the viewport: just outside it is off the picture.
    for outside in [[-1.0, 200.0], [401.0, 200.0], [200.0, -1.0], [200.0, 401.0]] {
        assert_eq!(value::value_at(&frame, &fit, SIZE, 1.0, outside), None);
    }
    // A picture wider than tall leaves bands above and below it, where there is no pixel.
    let wide = Frame {
        cols: 8,
        rows: 2,
        values: vec![1.0; 16],
        colour: None,
    };
    let above = value::value_at(&wide, &fit, SIZE, 1.0, [200.0, 100.0]);
    assert_eq!(above, None, "above a picture that is wider than tall");
    let on_it = value::value_at(&wide, &fit, SIZE, 1.0, [200.0, 200.0]);
    assert_eq!(on_it, Some(1.0));
}

#[test]
fn a_picture_in_colour_has_no_value_and_a_short_frame_does_not_stop_anything() {
    let fit = ViewTransform::fit();
    let pointer = centre_of(&fit, 1.0, 2.0);
    let coloured = Frame {
        colour: Some(vec![[1, 2, 3]; 16]),
        ..slice_with_a_dip()
    };
    assert_eq!(value::value_at(&coloured, &fit, SIZE, 1.0, pointer), None);
    let short = Frame {
        values: vec![0.0; 3],
        ..slice_with_a_dip()
    };
    assert_eq!(value::value_at(&short, &fit, SIZE, 1.0, pointer), None);
}

#[test]
fn the_slice_is_looked_up_only_with_a_pointer_over_the_viewport_and_only_if_decoded() {
    let state = showing_a_scan_of("CT", None);
    let pane = pane_of(&state);
    let [x, y] = centre_of(&ViewTransform::fit(), 1.0, 2.0);
    let at = Some(pos2(x, y));
    let there = |_: &SeriesId, _| Some(Arc::new(slice_with_a_dip()));
    assert_eq!(value::under_pointer(&pane, &there, at), Some(-12.0));
    assert_eq!(
        value::under_pointer(&pane, &there, None),
        None,
        "no pointer"
    );
    let not_decoded = |_: &SeriesId, _| None;
    assert_eq!(value::under_pointer(&pane, &not_decoded, at), None);
    let looked_up = std::cell::Cell::new(false);
    let watching = |_: &SeriesId, _| {
        looked_up.set(true);
        None
    };
    let _ = value::under_pointer(&pane, &watching, None);
    assert!(
        !looked_up.get(),
        "nothing is fetched while no pointer is over"
    );
}

#[test]
fn the_value_is_written_in_the_bottom_left_corner_while_the_pointer_is_over_the_picture() {
    let state = showing_a_scan_of("CT", None);
    let pane = pane_of(&state);
    let hand = Hand::new(400.0, 400.0);
    let drawn = |value| hand.settle(&mut |ui| annotate::overlays(ui, &pane, value));
    let with = drawn(Some(-12.0));
    assert!(
        with.words.iter().any(|word| word == "Value -12 HU"),
        "{:?}",
        with.words
    );
    let without = drawn(None);
    assert!(!without.words.iter().any(|word| word.starts_with("Value")));
}
