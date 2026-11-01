//! The Measure tab for what is measured on the slices: where its millimetres come from, in the
//! list and in the saved table; why a scan cannot be measured; what a plain stack says; and
//! going to a measurement on another slice.

use volsa_core::Point;
use volsa_dicom::testkit::{SliceOptions, classic_slice};

use super::*;
use crate::annotations::Shape;
use crate::measuring::{NO_SCALE, PLAIN_STACK};
use crate::reducer::reduce;
use crate::testing::patient::said_of;
use crate::testing::{
    THICK_SCAN, id, indexed, showing_a_picture, shown, slice_options, three_series,
    three_series_files,
};
use crate::ui::hand::{Hand, Seen};

/// How wide the contents of a tab are in the narrowest panel.
const NARROW: f32 = 236.0;
const NOTICE: &str = "Volsa is not a medical device and is not for diagnostic use.";

/// What the tab draws for `state`, once it has found its size.
fn drawn(state: &AppState) -> Seen {
    let (hand, mut tab) = (Hand::new(NARROW, 900.0), MeasureTab::default());
    hand.settle(&mut |ui| {
        let _ = show(ui, state, &mut tab, &mut Vec::new());
    })
}

/// What clicking every button of the tab asks for.
fn clicked(state: &AppState) -> Vec<Action> {
    let (hand, mut tab, mut actions) =
        (Hand::new(NARROW, 900.0), MeasureTab::default(), Vec::new());
    hand.click_each(&mut |ui| {
        let _ = show(ui, state, &mut tab, &mut actions);
    });
    actions
}

/// A state with these files indexed and `series` shown in the first viewport, its slice shown.
fn opened(files: Vec<(String, Vec<u8>)>, series: &str) -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(files)));
    let (viewport, series) = (0, id(series));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    state
}

#[test]
fn measurements_from_pixel_spacing_say_so_in_the_list_and_the_csv() {
    let mut state = showing_a_picture(Some([0.5, 0.5]), None);
    let on_row = |col: f64| [col * 0.5, 1.5, 0.0];
    let ruler = Shape::Ruler {
        a: on_row(2.0),
        b: on_row(12.0),
    };
    reduce(&mut state, Action::AnnotationAdd(ruler.clone()));
    let seen = drawn(&state);
    let said = "5.0 mm (from the file's pixel spacing)";
    assert!(
        seen.words.iter().any(|word| word == said),
        "{:?}",
        seen.words
    );
    let table = written(Save::Csv, &state);
    let line = table.split("\r\n").nth(1).expect("a line");
    let starts = "Ruler,Ruler 1,5.0 mm,Its ends can be placed to about one voxel: ± 0.5 mm along \
                  this line.,1.000,1.500,0.000,6.000,1.500,0.000,";
    assert!(line.starts_with(starts), "{line}");
    assert!(line.ends_with(&format!("{NOTICE},pixel spacing")), "{line}");
    // The same ruler on a scan placed in the patient says nothing after its value.
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    reduce(&mut state, Action::AnnotationAdd(ruler));
    let seen = drawn(&state);
    assert!(
        seen.words.iter().any(|word| word == "5.0 mm"),
        "{:?}",
        seen.words
    );
    assert!(!seen.words.iter().any(|word| word.contains("pixel spacing")));
}

#[test]
fn a_scan_that_cannot_be_measured_says_why_at_the_top_of_the_tab() {
    let state = showing_a_picture(None, Some([0.2, 0.2]));
    let seen = drawn(&state);
    assert_eq!(seen.words.first().map(String::as_str), Some(NO_SCALE));
    assert_eq!(clicked(&state), [], "no tool is offered");
    let picture = showing_a_picture(Some([0.5, 0.5]), None);
    assert!(!drawn(&picture).words.iter().any(|word| word == NO_SCALE));
    let tools = [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker].map(Action::SetTool);
    assert_eq!(clicked(&picture), tools, "every tool, on the slice");
}

/// A series of slices 5 mm and then 10 mm apart, shown as a plain stack with no 3D view, its
/// middle slice shown.
fn plain_stack() -> AppState {
    let files = [0.0, 5.0, 15.0].iter().zip(0..).map(|(z, place)| {
        let options = SliceOptions {
            position: Some([0.0, 0.0, *z]),
            ..slice_options("1.2.3.11", 11, place, 4)
        };
        (format!("S11/I{place}.dcm"), classic_slice(&options))
    });
    opened(files.collect(), "1.2.3.11")
}

/// `state` with a ruler and a marker measured on its slice, and the same on the 30-slice
/// series, which is no plain stack.
fn measured_on(mut state: AppState) -> (AppState, AppState) {
    let mut placed = AppState::default();
    reduce(&mut placed, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut placed, Action::SelectSeries { viewport, series });
    shown(&mut placed);
    let ruler = Shape::Ruler {
        a: [0.0, 0.0, 5.0],
        b: [3.0, 4.0, 5.0],
    };
    let marker = Shape::Marker {
        at: [1.0, 1.0, 5.0],
    };
    for shape in [ruler, marker] {
        reduce(&mut state, Action::AnnotationAdd(shape.clone()));
        reduce(&mut placed, Action::AnnotationAdd(shape));
    }
    assert_eq!(
        state.annotations.items.len(),
        2,
        "measured on the plain stack"
    );
    (state, placed)
}

#[test]
fn a_plain_stacks_measurements_say_so_in_every_line_of_the_saved_table() {
    let (stack, placed) = measured_on(plain_stack());
    let lines = |state: &AppState| -> Vec<String> {
        let table = written(Save::Csv, state);
        let lines = table.split("\r\n").skip(1).filter(|line| !line.is_empty());
        lines.map(str::to_owned).collect()
    };
    // The notice holds a comma, so it is written in quotes.
    let carried = format!("\"{NOTICE} {PLAIN_STACK}\",patient");
    let on_the_stack = lines(&stack);
    assert_eq!(on_the_stack.len(), 2);
    for line in &on_the_stack {
        assert!(line.ends_with(&carried), "{line}");
    }
    for line in lines(&placed) {
        assert!(line.ends_with(&format!(",{NOTICE},patient")), "{line}");
    }
}

#[test]
fn a_plain_stacks_measurements_say_so_in_the_saved_json() {
    let (stack, placed) = measured_on(plain_stack());
    let notice = |state: &AppState| {
        let read: serde_json::Value =
            serde_json::from_str(&written(Save::Json, state)).expect("it parses");
        read["notice"].as_str().map(str::to_owned)
    };
    assert_eq!(notice(&stack), Some(format!("{NOTICE} {PLAIN_STACK}")));
    assert_eq!(notice(&placed).as_deref(), Some(NOTICE));
}

#[test]
fn a_plain_stack_says_so_above_its_measurements() {
    let state = plain_stack();
    let seen = drawn(&state);
    assert!(
        seen.words.iter().any(|word| word == PLAIN_STACK),
        "{:?}",
        seen.words
    );
    let picture = showing_a_picture(Some([0.5, 0.5]), None);
    assert!(!drawn(&picture).words.iter().any(|word| word == PLAIN_STACK));
}

#[test]
fn going_to_a_measurement_shows_the_slice_it_was_made_on() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let at = info
        .voxel_to_patient
        .expect("placed")
        .apply(Point::new(1.0, 2.0, 21.0));
    reduce(
        &mut state,
        Action::AnnotationAdd(Shape::Marker {
            at: [at.x, at.y, at.z],
        }),
    );
    assert_eq!(state.viewports[0].slice, 15, "shown on another slice");
    let actions = clicked(&state);
    assert!(
        actions.contains(&Action::SetSlice {
            viewport: 0,
            slice: 21
        }),
        "{actions:?}"
    );
    assert!(
        actions.contains(&Action::SetPosition(at)),
        "and the planes go there too"
    );
    // A picture's own millimetres are no place in the patient for the planes to go to.
    let mut picture = showing_a_picture(Some([0.5, 0.5]), None);
    reduce(
        &mut picture,
        Action::AnnotationAdd(Shape::Marker {
            at: [3.0, 2.0, 0.0],
        }),
    );
    let actions = clicked(&picture);
    assert!(
        actions.contains(&Action::SetSlice {
            viewport: 0,
            slice: 0
        }),
        "{actions:?}"
    );
    let placed = |action: &Action| matches!(action, Action::SetPosition(_));
    assert!(!actions.iter().any(placed), "{actions:?}");
}

#[test]
fn saved_measurements_hold_nothing_of_the_patient_even_with_the_details_shown() {
    let mut state = opened(three_series_files(), THICK_SCAN);
    state.show_patient = true;
    let ruler = Shape::Ruler {
        a: [0.0, 0.0, 0.0],
        b: [3.0, 0.0, 0.0],
    };
    reduce(&mut state, Action::AnnotationAdd(ruler));
    reduce(
        &mut state,
        Action::AnnotationAdd(Shape::Marker {
            at: [1.0, 1.0, 0.0],
        }),
    );
    // What the made-up export says of its patient, its study and its scans.
    let said = said_of(state.index.as_ref().expect("an export is open"));
    for kind in [Save::Csv, Save::Json] {
        let file = written(kind, &state);
        assert!(
            file.contains("Ruler") || file.contains("ruler"),
            "a measurement: {file}"
        );
        for text in &said {
            let found = file.to_lowercase().contains(&text.to_lowercase());
            assert!(!found, "{text} is in the saved file: {file}");
        }
    }
}
