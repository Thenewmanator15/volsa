//! Annotations in the state: adding, naming, removing, undoing and redoing.

use std::path::PathBuf;

use super::scene_tests::{ready, select};
use super::tests::showing;
use super::*;
use crate::annotations::{Annotations, MAX_ANNOTATIONS, MAX_LABEL, Shape, Tool};
use crate::state::NoticeKind;
use crate::testing::{FINE_SCAN, THICK_SCAN};

fn ruler(length: f64) -> Shape {
    Shape::Ruler {
        a: [0.0; 3],
        b: [length, 0.0, 0.0],
    }
}

fn marker() -> Shape {
    Shape::Marker {
        at: [1.0, 2.0, 3.0],
    }
}

fn add(state: &mut AppState, shape: Shape) {
    reduce(state, Action::AnnotationAdd(shape));
}

fn labels(state: &AppState) -> Vec<&str> {
    let items = &state.annotations.items;
    items.iter().map(|item| item.label.as_str()).collect()
}

#[test]
fn an_annotation_is_named_by_its_kind_and_number_until_it_is_given_a_name() {
    let mut state = ready();
    assert_eq!(state.annotations, Annotations::default());
    add(&mut state, ruler(5.0));
    add(&mut state, marker());
    assert_eq!(labels(&state), ["Ruler 1", "Marker 2"]);
    let ids: Vec<u64> = state.annotations.items.iter().map(|item| item.id).collect();
    assert_eq!(ids, [1, 2]);
    let rename = |state: &mut AppState, id, label: &str| {
        let label = label.to_owned();
        reduce(state, Action::AnnotationRename { id, label });
    };
    rename(&mut state, 1, "  width of the thing  ");
    assert_eq!(labels(&state), ["width of the thing", "Marker 2"]);
    rename(&mut state, 1, &"x".repeat(200));
    assert_eq!(state.annotations.items[0].label.chars().count(), MAX_LABEL);
    rename(&mut state, 1, "   ");
    assert_eq!(
        labels(&state),
        ["Ruler 1", "Marker 2"],
        "no name is its own name again"
    );
}

#[test]
fn every_change_can_be_undone_and_done_again() {
    let mut state = ready();
    add(&mut state, ruler(5.0));
    add(&mut state, marker());
    assert!(state.annotations.can_undo() && !state.annotations.can_redo());
    reduce(&mut state, Action::Undo);
    assert_eq!(labels(&state), ["Ruler 1"]);
    assert!(state.annotations.can_redo());
    reduce(&mut state, Action::Redo);
    assert_eq!(labels(&state), ["Ruler 1", "Marker 2"]);
    assert_eq!(state.annotations.items[1].id, 2, "the same one comes back");
    reduce(&mut state, Action::Undo);
    add(&mut state, ruler(9.0));
    assert_eq!(labels(&state), ["Ruler 1", "Ruler 3"]);
    assert!(
        !state.annotations.can_redo(),
        "a new change leaves nothing to do again"
    );
    reduce(&mut state, Action::AnnotationRemove(1));
    assert_eq!(labels(&state), ["Ruler 3"]);
    reduce(&mut state, Action::Undo);
    assert_eq!(
        labels(&state),
        ["Ruler 1", "Ruler 3"],
        "a removal is undone too"
    );
    reduce(&mut state, Action::AnnotationsClear);
    assert_eq!(labels(&state), [] as [&str; 0]);
    reduce(&mut state, Action::Undo);
    assert_eq!(labels(&state), ["Ruler 1", "Ruler 3"]);
}

#[test]
fn undoing_and_redoing_with_nothing_to_do_changes_nothing() {
    let mut state = ready();
    reduce(&mut state, Action::Undo);
    reduce(&mut state, Action::Redo);
    assert_eq!(state.annotations, Annotations::default());
    add(&mut state, marker());
    let before = state.annotations.clone();
    reduce(&mut state, Action::Redo);
    reduce(&mut state, Action::AnnotationRemove(99));
    let (id, label) = (99, "no such one".to_owned());
    reduce(&mut state, Action::AnnotationRename { id, label });
    assert_eq!(
        state.annotations, before,
        "nor do changes to what is not there add a step"
    );
    reduce(&mut state, Action::Undo);
    reduce(&mut state, Action::Undo);
    assert_eq!(state.annotations.items, []);
    reduce(&mut state, Action::AnnotationsClear);
    assert!(
        !state.annotations.can_undo(),
        "clearing an empty list is no step"
    );
}

#[test]
fn a_hundred_steps_are_kept_and_no_more() {
    let mut state = ready();
    for _ in 0..101 {
        add(&mut state, marker());
    }
    let mut undone = 0;
    while state.annotations.can_undo() {
        reduce(&mut state, Action::Undo);
        undone += 1;
    }
    assert_eq!((undone, state.annotations.items.len()), (100, 1));
}

#[test]
fn the_list_holds_two_hundred_and_says_so_when_it_is_full() {
    let mut state = ready();
    for _ in 0..MAX_ANNOTATIONS {
        add(&mut state, marker());
    }
    assert_eq!(state.notices, []);
    add(&mut state, marker());
    assert_eq!(state.annotations.items.len(), MAX_ANNOTATIONS);
    let [notice] = &state.notices[..] else {
        panic!("{:?}", state.notices);
    };
    assert_eq!(notice.kind, NoticeKind::Warning);
    assert_eq!(
        notice.text,
        "There are already 200 measurements. Remove one to add another."
    );
    add(&mut state, marker());
    assert_eq!(state.notices.len(), 1, "said once");
}

#[test]
fn a_tool_is_taken_up_only_where_there_is_a_volume_to_measure() {
    let mut state = showing(THICK_SCAN);
    assert_eq!(state.tool, Tool::Navigate);
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(
        state.tool,
        Tool::Navigate,
        "the volume is still being built"
    );
    add(&mut state, marker());
    assert_eq!(
        state.annotations.items,
        [],
        "nor is there anything to mark yet"
    );
    let mut state = ready();
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler);
    reduce(&mut state, Action::SetTool(Tool::Navigate));
    assert_eq!(state.tool, Tool::Navigate);
}

#[test]
fn another_series_or_another_scan_starts_with_no_annotations_and_no_tool() {
    let mut state = ready();
    add(&mut state, marker());
    reduce(&mut state, Action::SetTool(Tool::Angle));
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(state.annotations, Annotations::default());
    assert_eq!(state.tool, Tool::Navigate);
    let mut state = ready();
    add(&mut state, marker());
    reduce(&mut state, Action::SetTool(Tool::Region));
    reduce(&mut state, Action::OpenPath(PathBuf::from("another.zip")));
    assert_eq!(
        (state.annotations, state.tool),
        (Annotations::default(), Tool::Navigate)
    );
    // Scrolling, or anything else that leaves the scene on its series, keeps them.
    let mut state = ready();
    add(&mut state, marker());
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 1,
        },
    );
    assert_eq!(state.annotations.items.len(), 1);
}
