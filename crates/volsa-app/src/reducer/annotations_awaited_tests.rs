//! What is measured before the measurements kept for a series have been read is not written
//! over them: it waits for them, is put after them, and is written then.

use super::annotations_tests::{add, labels, marker, ruler};
use super::scene_tests::{built, select};
use super::*;
use crate::annotations::Annotation;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, shown};

fn old(id: u64) -> Annotation {
    Annotation {
        id,
        label: "kept".to_owned(),
        shape: marker(),
        scale: crate::measuring::Scale::Patient,
    }
}

fn read(series: &str, items: Vec<Annotation>) -> Action {
    let series = id(series);
    Action::AnnotationsLoaded { series, items }
}

fn written(series: &str, state: &AppState) -> Vec<Effect> {
    let (series, items) = (id(series), state.annotations.items.clone());
    vec![Effect::StoreAnnotations { series, items }]
}

#[test]
fn a_measurement_made_before_the_kept_ones_are_read_waits_for_them_and_is_written_with_them() {
    let mut state = built();
    assert!(state.annotations.awaits_kept(), "they were asked for");
    let held = reduce(&mut state, Action::AnnotationAdd(marker()));
    assert_eq!(held, [], "not written over what has not been read");
    assert_eq!(reduce(&mut state, Action::Undo), []);
    assert_eq!(reduce(&mut state, Action::Redo), []);
    let effects = reduce(&mut state, read(THICK_SCAN, vec![old(4)]));
    assert_eq!(labels(&state), ["kept", "Marker 5"]);
    assert_eq!(effects, written(THICK_SCAN, &state), "both, once read");
    assert!(!state.annotations.awaits_kept());
    // From then on every change is written as it is made.
    let effects = reduce(&mut state, Action::AnnotationAdd(ruler(5.0)));
    assert_eq!(labels(&state), ["kept", "Marker 5", "Ruler 6"]);
    assert_eq!(effects, written(THICK_SCAN, &state));
    // And undoing everything done here leaves what was kept.
    while state.annotations.can_undo() {
        reduce(&mut state, Action::Undo);
    }
    assert_eq!(labels(&state), ["kept"]);
}

#[test]
fn with_nothing_kept_what_was_measured_meanwhile_is_written_once_that_is_known() {
    let mut state = built();
    add(&mut state, marker());
    let effects = reduce(&mut state, read(THICK_SCAN, Vec::new()));
    assert_eq!(labels(&state), ["Marker 1"]);
    assert_eq!(effects, written(THICK_SCAN, &state));
}

#[test]
fn a_series_left_before_its_kept_measurements_were_read_has_them_put_with_its_own_when_they_are() {
    let mut state = built();
    add(&mut state, marker());
    // Another card is clicked before the first series' measurements have been read.
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(labels(&state), [] as [&str; 0]);
    let effects = reduce(&mut state, read(THICK_SCAN, vec![old(4)]));
    let both = match effects.as_slice() {
        [Effect::StoreAnnotations { series, items }] if *series == id(THICK_SCAN) => items,
        other => panic!("{other:?}"),
    };
    let said: Vec<&str> = both.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(said, ["kept", "Marker 5"], "written for the series left");
    assert_eq!(labels(&state), [] as [&str; 0], "not this series' own");
    // Back on the first series, its list holds both, and reading them again adds nothing.
    shown(&mut state);
    select(&mut state, 0, THICK_SCAN);
    assert_eq!(labels(&state), ["kept", "Marker 5"]);
    assert_eq!(reduce(&mut state, read(THICK_SCAN, vec![old(4)])), []);
    assert_eq!(labels(&state), ["kept", "Marker 5"]);
}

#[test]
fn what_is_read_for_a_series_neither_shown_nor_waiting_is_let_go() {
    let mut state = built();
    assert_eq!(reduce(&mut state, read(FINE_SCAN, vec![old(4)])), []);
    assert_eq!(labels(&state), [] as [&str; 0]);
    assert!(state.annotations.awaits_kept(), "still waiting for its own");
}
