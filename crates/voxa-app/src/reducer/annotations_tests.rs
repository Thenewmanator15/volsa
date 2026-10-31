//! Annotations in the state: adding, naming, removing, undoing and redoing.

use std::path::PathBuf;

use super::scene_tests::{ready, select};
use super::tests::showing;
use super::*;
use crate::annotations::{Annotations, MAX_ANNOTATIONS, MAX_LABEL, Shape, Tool};
use crate::measuring::Scale;
use crate::scene::SceneStatus;
use crate::state::NoticeKind;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, showing_a_picture, shown};

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
fn a_tool_is_taken_up_only_where_there_is_a_slice_with_a_scale_to_measure_on() {
    let mut state = showing(THICK_SCAN);
    assert_eq!(state.tool, Tool::Navigate);
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Navigate, "its slice is not shown yet");
    add(&mut state, marker());
    assert_eq!(
        state.annotations.items,
        [],
        "nor is there anything to mark yet"
    );
    // A detector's spacing is no scale for the body.
    let mut state = showing_a_picture(None, Some([0.2, 0.2]));
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Navigate, "the picture has no scale");
    add(&mut state, marker());
    assert_eq!(state.annotations.items, []);
    let mut state = ready();
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler);
    reduce(&mut state, Action::SetTool(Tool::Navigate));
    assert_eq!(state.tool, Tool::Navigate);
}

#[test]
fn a_ruler_can_be_drawn_on_a_slice_before_the_volume_is_ready() {
    let mut state = showing(THICK_SCAN);
    shown(&mut state);
    assert!(matches!(state.scene.status, SceneStatus::Building { .. }));
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler);
    let effects = reduce(&mut state, Action::AnnotationAdd(ruler(5.0)));
    assert_eq!(labels(&state), ["Ruler 1"]);
    assert_eq!(state.annotations.items[0].scale, Scale::Patient);
    assert_eq!(effects, kept_as_it_is(&state), "and it is kept");
    // The tool stays in hand, and the ruler in the list, once the volume is ready.
    let (series, corners) = (id(THICK_SCAN), super::scene_tests::corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    assert_eq!((state.tool, labels(&state)), (Tool::Ruler, vec!["Ruler 1"]));
}

#[test]
fn a_picture_with_a_pixel_spacing_alone_is_measured_in_its_own_millimetres() {
    let mut state = showing_a_picture(Some([0.5, 0.5]), None);
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler);
    add(&mut state, ruler(5.0));
    let [ruler] = &state.annotations.items[..] else {
        panic!("{:?}", state.annotations.items);
    };
    assert_eq!(ruler.scale, Scale::PixelSpacing);
}

#[test]
fn another_series_or_another_scan_starts_with_no_annotations_and_no_tool() {
    let mut state = ready();
    add(&mut state, marker());
    reduce(&mut state, Action::SetTool(Tool::Angle));
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(state.annotations, Annotations::default());
    assert_eq!(state.tool, Tool::Navigate);
    // Even back on a series whose slice was shown before, and so can be measured on at once,
    // the tool is to be taken up again.
    shown(&mut state);
    select(&mut state, 0, THICK_SCAN);
    shown(&mut state);
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler);
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(crate::measuring::why_not_measurable(&state), None);
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

fn kept_as_it_is(state: &AppState) -> Vec<Effect> {
    let (series, items) = (id(THICK_SCAN), state.annotations.items.clone());
    vec![Effect::StoreAnnotations { series, items }]
}

#[test]
fn every_change_to_the_list_asks_for_it_to_be_kept() {
    let mut state = ready();
    let rename = || {
        let (id, label) = (2, "the long way".to_owned());
        Action::AnnotationRename { id, label }
    };
    let changes = [
        Action::AnnotationAdd(marker()),
        Action::AnnotationAdd(ruler(5.0)),
        Action::Undo,
        Action::Redo,
        rename(),
        Action::AnnotationRemove(1),
        Action::AnnotationsClear,
    ];
    for change in changes {
        let said = format!("{change:?}");
        let effects = reduce(&mut state, change);
        assert_eq!(effects, kept_as_it_is(&state), "{said}");
    }
    let nothing = [
        Action::Redo,
        Action::AnnotationRemove(99),
        Action::AnnotationsClear,
        Action::SetTool(Tool::Ruler),
    ];
    for action in nothing {
        let said = format!("{action:?}");
        assert_eq!(
            reduce(&mut state, action),
            [],
            "{said} changes nothing to keep"
        );
    }
}

#[test]
fn annotations_read_from_where_they_are_kept_become_the_list_with_nothing_to_undo() {
    use crate::annotations::Annotation;
    let mut state = ready();
    let kept = vec![Annotation {
        id: 4,
        label: "kept".to_owned(),
        shape: marker(),
        scale: crate::measuring::Scale::Patient,
    }];
    let read = |series: &str| {
        let (series, items) = (id(series), kept.clone());
        Action::AnnotationsLoaded { series, items }
    };
    assert_eq!(reduce(&mut state, read(FINE_SCAN)), []);
    assert_eq!(
        state.annotations.items,
        [],
        "another series' are not this one's"
    );
    assert_eq!(
        reduce(&mut state, read(THICK_SCAN)),
        [],
        "reading is no change to keep"
    );
    assert_eq!(labels(&state), ["kept"]);
    assert!(
        !state.annotations.can_undo(),
        "what was read cannot be undone away"
    );
    add(&mut state, marker());
    assert_eq!(
        state.annotations.items[1].id, 5,
        "numbers carry on from what was kept"
    );
    // Once the person has changed the list, what arrives late does not undo their change.
    reduce(&mut state, read(THICK_SCAN));
    assert_eq!(labels(&state), ["kept", "Marker 5"]);
}

#[test]
fn a_scene_started_on_a_series_asks_for_its_annotations() {
    use super::tests::loaded;
    use crate::testing::{LOCALISER, SINGLE, with_odd_series};
    let mut state = loaded();
    let effects = select(&mut state, 0, THICK_SCAN);
    let series = id(THICK_SCAN);
    assert!(
        effects.contains(&Effect::LoadAnnotations { series }),
        "{effects:?}"
    );
    // A series with no 3D view is measured on its slices, and so has annotations kept too.
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(with_odd_series()));
    let effects = select(&mut state, 0, SINGLE);
    let series = id(SINGLE);
    assert!(
        effects.contains(&Effect::LoadAnnotations { series }),
        "{effects:?}"
    );
    let effects = select(&mut state, 0, LOCALISER);
    let asked = |effect: &Effect| matches!(effect, Effect::LoadAnnotations { .. });
    assert!(
        !effects.iter().any(asked),
        "nothing is measured on slices in several planes"
    );
}

#[test]
fn annotations_that_could_not_be_written_are_said_to_be_lost_next_time() {
    let mut state = ready();
    for _ in 0..2 {
        let reason = "the disk is full".to_owned();
        reduce(&mut state, Action::AnnotationsNotStored { reason });
    }
    let [notice] = &state.notices[..] else {
        panic!("said once: {:?}", state.notices);
    };
    assert_eq!(notice.kind, NoticeKind::Warning);
    let said = "Measurements could not be written, so they will not be there next time";
    assert_eq!(
        (notice.text.as_str(), &notice.details[..]),
        (said, &["the disk is full".to_owned()][..])
    );
}
