//! What is done to the measurements of a series stays with it while the scene looks at another:
//! clearing them can still be taken back when the series is shown again.

use std::path::PathBuf;

use super::annotations_tests::{add, labels, marker, ruler};
use super::scene_tests::{ready, select};
use super::*;
use crate::annotations::{Annotation, Annotations};
use crate::testing::{FINE_SCAN, THICK_SCAN, id, shown};

#[test]
fn clearing_the_measurements_can_still_be_taken_back_after_another_series_is_shown_and_this_one_returns()
 {
    let mut state = ready();
    add(&mut state, marker());
    add(&mut state, ruler(5.0));
    reduce(&mut state, Action::AnnotationsClear);
    assert_eq!(state.annotations.items.len(), 0);
    // Another card is clicked, and then this one again.
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(
        state.annotations,
        Annotations::default(),
        "another series' own"
    );
    shown(&mut state);
    select(&mut state, 0, THICK_SCAN);
    assert!(
        state.annotations.can_undo(),
        "what was done to this list is still there"
    );
    reduce(&mut state, Action::Undo);
    assert_eq!(labels(&state).len(), 2, "the measurements are back");
    // And what was read from where they are kept does not put the cleared list back over it.
    let kept = vec![Annotation {
        id: 9,
        label: "Old".to_owned(),
        shape: marker(),
        scale: crate::measuring::Scale::Patient,
    }];
    let series = id(THICK_SCAN);
    reduce(
        &mut state,
        Action::AnnotationsLoaded {
            series,
            items: kept,
        },
    );
    assert_eq!(labels(&state).len(), 2, "{:?}", labels(&state));
}

#[test]
fn only_the_last_few_series_are_remembered_and_a_new_export_forgets_them() {
    use crate::annotations::MOST_LEFT as MOST;
    let mut left = crate::annotations::Left::default();
    let mut changed = Annotations::default();
    changed.add(marker(), crate::measuring::Scale::Patient);
    for number in 0..=MOST {
        left.put_away(id(&format!("series.{number}")), changed.clone());
    }
    assert!(
        left.take_back(&id("series.0")).is_none(),
        "the first is forgotten"
    );
    assert!(left.take_back(&id(&format!("series.{MOST}"))).is_some());
    // A list with nothing to undo is read again from where it is kept, not remembered.
    left.put_away(id("untouched"), Annotations::loaded(Vec::new()));
    assert!(left.take_back(&id("untouched")).is_none());
    // Another export starts with nothing remembered.
    let mut state = ready();
    add(&mut state, marker());
    select(&mut state, 0, FINE_SCAN);
    assert_ne!(state.annotations_left, crate::annotations::Left::default());
    reduce(&mut state, Action::OpenPath(PathBuf::from("another.zip")));
    assert_eq!(state.annotations_left, crate::annotations::Left::default());
}
