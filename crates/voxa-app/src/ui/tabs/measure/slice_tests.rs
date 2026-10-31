//! The Measure tab for what is measured on the slices: where its millimetres come from, in the
//! list and in the saved table; why a scan cannot be measured; what a plain stack says; and
//! going to a measurement on another slice.

use voxa_core::Point;
use voxa_dicom::testkit::{SliceOptions, classic_slice};

use super::*;
use crate::annotations::Shape;
use crate::measuring::{NO_SCALE, PLAIN_STACK};
use crate::reducer::reduce;
use crate::testing::{
    THICK_SCAN, id, indexed, showing_a_picture, shown, slice_options, three_series,
};
use crate::ui::hand::{Hand, Seen};

/// How wide the contents of a tab are in the narrowest panel.
const NARROW: f32 = 236.0;
const NOTICE: &str = "Voxa is not a medical device and is not for diagnostic use.";

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
    let placement = scene_placement(&state).map(|at| at.placement);
    let table = measurements_csv(&rows(&state.annotations.items, placement.as_ref()), NOTICE);
    let line = table.split("\r\n").nth(1).expect("a line");
    assert!(line.starts_with("Ruler,Ruler 1,5.0 mm,"), "{line}");
    assert!(line.contains(",pixel spacing,1.000,1.500,0.000,"), "{line}");
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

#[test]
fn a_plain_stack_says_so_above_its_measurements() {
    // Slices 5 mm and then 10 mm apart: shown as a plain stack, with no 3D view.
    let files = [0.0, 5.0, 15.0].iter().zip(0..).map(|(z, place)| {
        let options = SliceOptions {
            position: Some([0.0, 0.0, *z]),
            ..slice_options("1.2.3.11", 11, place, 4)
        };
        (format!("S11/I{place}.dcm"), classic_slice(&options))
    });
    let state = opened(files.collect(), "1.2.3.11");
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
