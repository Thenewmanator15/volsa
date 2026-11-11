//! Annotations kept from one session to the next.

use std::time::Duration;

use super::tests::{open_until, ready_in_3d, session, session_keeping, until, zip_of};
use crate::action::Action;
use crate::annotations::Shape;
use crate::state::NoticeKind;
use crate::testing::{FINE_SCAN, three_series_files};

fn ruler() -> Shape {
    Shape::Ruler {
        a: [0.0, 0.0, 10.0],
        b: [6.0, 0.0, 10.0],
    }
}

fn marker() -> Shape {
    Shape::Marker {
        at: [2.0, 3.0, 25.0],
    }
}

#[test]
fn annotations_are_there_the_next_time_the_scan_is_opened_and_the_file_names_nothing_of_it() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let file = kept.path().join("annotations.json");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    session.dispatch(Action::AnnotationAdd(ruler()));
    session.dispatch(Action::AnnotationAdd(marker()));
    let (id, label) = (2, "the tip of it".to_owned());
    session.dispatch(Action::AnnotationRename { id, label });
    let written = |text: &str| std::fs::read_to_string(&file).is_ok_and(|kept| kept.contains(text));
    until(&mut session, |_| written("the tip of it"));

    let text = std::fs::read_to_string(&file).expect("reads");
    let index = session.state().index.as_ref().expect("indexed");
    let study = &index.studies[0];
    let private = [
        Some(FINE_SCAN.to_owned()),
        Some(study.id.0.clone()),
        Some(study.description.clone()),
        study.date.clone(),
        study.patient_name.clone(),
        study.patient_id.clone(),
    ];
    for word in private.iter().flatten().filter(|word| !word.is_empty()) {
        assert!(!text.contains(word.as_str()), "the file names {word}");
    }
    drop(session);

    let mut again = session_keeping(kept.path());
    open_until(&mut again, &path, |session| {
        ready_in_3d(session, FINE_SCAN) && !session.state().annotations.items.is_empty()
    });
    let annotations = &again.state().annotations;
    let labels: Vec<&str> = annotations
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(labels, ["Ruler 1", "the tip of it"]);
    assert_eq!(annotations.items[0].shape, ruler());
    assert!(
        !annotations.can_undo(),
        "what was kept is where this session starts from"
    );
}

#[test]
fn with_nowhere_to_keep_them_annotations_last_as_long_as_the_window() {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    session.dispatch(Action::AnnotationAdd(marker()));
    assert_eq!(session.state().annotations.items.len(), 1);
    session.wait(Duration::from_millis(300));
    assert_eq!(session.state().notices, []);
}

#[test]
fn annotations_that_cannot_be_written_are_said_once_to_be_lost_next_time() {
    // The settings folder is given as something that is a file, so nothing can be put in it.
    let kept = tempfile::tempdir().expect("a temporary folder");
    let not_a_folder = kept.path().join("in the way");
    std::fs::write(&not_a_folder, "a file").expect("writes");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(&not_a_folder);
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    session.dispatch(Action::AnnotationAdd(marker()));
    session.dispatch(Action::AnnotationAdd(ruler()));
    until(&mut session, |session| !session.state().notices.is_empty());
    session.wait(Duration::from_millis(300));
    let [notice] = &session.state().notices[..] else {
        panic!("{:?}", session.state().notices);
    };
    assert_eq!(notice.kind, NoticeKind::Warning);
    assert!(
        notice.text.starts_with("Measurements could not be written"),
        "{}",
        notice.text
    );
    assert_eq!(
        session.state().annotations.items.len(),
        2,
        "they are still there for now"
    );
}
