//! Saving the slice shown as a picture: its pixels through the window, mirrored and turned as
//! shown, and nothing about the patient written.

use std::sync::Arc;

use volsa_core::{SeriesId, WindowLevel};
use volsa_dicom::Frame;
use volsa_render::ViewTransform;

use super::picture::{SLICE_NAME, drawn, save_to, shown};
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::testing::{THICK_SCAN, id, shown as decoded, three_series};

/// A grey frame `cols` wide holding `values`, row by row.
fn frame(cols: u32, values: &[f32]) -> Frame {
    let rows = u32::try_from(values.len()).expect("small") / cols;
    Frame {
        rows,
        cols,
        values: values.to_vec().into(),
        colour: None,
    }
}

/// The red of each pixel of a picture, row by row: the grey, for a grey one.
fn greys(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks(4).map(|pixel| pixel[0]).collect()
}

#[test]
fn the_picture_goes_from_black_to_white_through_the_window() {
    // A window from 0 to 200: below is black, above is white, the middle half grey.
    let wl = WindowLevel::new(100.0, 200.0);
    let values = [-500.0, 0.0, 100.0, 200.0, 9000.0, f32::NAN];
    let (rgba, size) = drawn(&frame(6, &values), wl, &ViewTransform::fit()).expect("drawn");
    assert_eq!(size, [6, 1]);
    assert_eq!(greys(&rgba), [0, 0, 128, 255, 255, 0]);
    let opaque = rgba.chunks(4).all(|pixel| pixel[3] == 255);
    assert!(opaque && rgba.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2]));
}

#[test]
fn the_picture_is_mirrored_and_turned_as_shown() {
    let wl = WindowLevel::new(100.0, 200.0);
    // Two columns and one row: black on the left, white on the right.
    let slice = frame(2, &[0.0, 200.0]);
    let mirrored = ViewTransform::fit().flipped(true);
    let (rgba, size) = drawn(&slice, wl, &mirrored).expect("drawn");
    assert_eq!((greys(&rgba), size), (vec![255, 0], [2, 1]));
    // A quarter turn clockwise stands the row up: its left end goes to the top.
    let turned = ViewTransform::fit().turned_clockwise();
    let (rgba, size) = drawn(&slice, wl, &turned).expect("drawn");
    assert_eq!((greys(&rgba), size), (vec![0, 255], [1, 2]));
    // Zoom and pan are left out: the whole slice is saved.
    let zoomed = ViewTransform::fit().zoomed_about(4.0, [30.0, 10.0]);
    let (rgba, _) = drawn(&slice, wl, &zoomed).expect("drawn");
    assert_eq!(greys(&rgba), [0, 255]);
    // A frame with fewer values than its size says is not drawn.
    let short = Frame {
        rows: 2,
        ..frame(2, &[0.0, 1.0])
    };
    assert!(drawn(&short, wl, &ViewTransform::fit()).is_none());
}

#[test]
fn a_slice_in_colour_keeps_its_own_colours() {
    let slice = Frame {
        colour: Some(vec![[200, 10, 20], [1, 2, 3]]),
        ..frame(2, &[0.0, 0.0])
    };
    let (rgba, _) = drawn(&slice, WindowLevel::new(0.0, 1.0), &ViewTransform::fit()).expect("x");
    assert_eq!(rgba, [200, 10, 20, 255, 1, 2, 3, 255]);
}

/// The kinds of chunk a PNG file holds, in order.
fn png_chunks(file: &[u8]) -> Vec<String> {
    let mut kinds = Vec::new();
    let mut at = 8;
    while let Some(head) = file.get(at..at + 8) {
        let (length, kind) = head.split_at(4);
        let length = u32::from_be_bytes(length.try_into().expect("four bytes"));
        kinds.push(String::from_utf8_lossy(kind).into_owned());
        at += 12 + usize::try_from(length).expect("a small chunk");
    }
    kinds
}

#[test]
fn the_saved_slice_holds_its_pixels_and_nothing_about_the_patient() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    decoded(&mut state);
    let name = "MADEUP^PATIENT";
    let index = state.index.as_mut().expect("indexed");
    index.studies[0].patient_name = Some(name.to_owned());
    index.studies[0].patient_id = Some("MADEUP-ID-7".to_owned());
    state.show_patient = true;
    let wl = state.viewports[0].wl;
    let values: Vec<f32> = (0..16_u8)
        .map(|at| wl.low() + wl.width * f32::from(at) / 15.0)
        .collect();
    let slice = Arc::new(frame(4, &values));
    let frames = |_: &SeriesId, _: usize| Some(Arc::clone(&slice));
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join(format!("{SLICE_NAME}.png"));
    let notice = save_to(&path, &state, &frames);
    assert!(notice.text.starts_with("Saved"), "{}", notice.text);
    let file = std::fs::read(&path).expect("written");
    assert_eq!(png_chunks(&file), ["IHDR", "IDAT", "IEND"], "pixels alone");
    for told in [name, "MADEUP", "MADEUP-ID-7", "Patient", THICK_SCAN] {
        let found = file
            .windows(told.len())
            .any(|bytes| bytes == told.as_bytes());
        assert!(!found, "{told} is written in the file");
    }
    // What is in the file is the picture as drawn.
    let read_back = image::load_from_memory(&file).expect("a PNG").to_rgba8();
    let (rgba, size) = shown(&state, &frames).expect("shown");
    assert_eq!((read_back.width(), read_back.height()), (size[0], size[1]));
    assert_eq!(read_back.into_raw(), rgba);
    // With no slice there, nothing is written and the person is told why.
    let nothing = |_: &SeriesId, _: usize| None;
    let elsewhere = folder.path().join("nothing.png");
    let notice = save_to(&elsewhere, &state, &nothing);
    assert!(
        !elsewhere.exists() && notice.text.contains("no slice"),
        "{notice:?}"
    );
}
