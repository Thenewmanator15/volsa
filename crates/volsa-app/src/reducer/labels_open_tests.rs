//! Labels opened when there is nothing to put them on.

use std::path::PathBuf;

use super::scene_tests::ready;
use super::tests::showing;
use super::*;
use crate::labels::{LabelsStatus, NOTHING_TO_LABEL};
use crate::testing::THICK_SCAN;

fn open() -> Action {
    Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    }
}

#[test]
fn labels_opened_with_nothing_to_put_them_on_are_not_read_and_the_person_is_told_why() {
    let said = |state: &AppState| -> Vec<String> {
        let notices = state.notices.iter();
        notices.map(|notice| notice.text.clone()).collect()
    };
    for mut state in [AppState::default(), showing(THICK_SCAN)] {
        assert_eq!(reduce(&mut state, open()), []);
        assert_eq!(state.labels.status, LabelsStatus::None);
        assert_eq!(
            said(&state),
            [NOTHING_TO_LABEL],
            "a file was dropped, and why not"
        );
        reduce(&mut state, open());
        assert_eq!(said(&state).len(), 1, "said once however often it is tried");
    }
    assert!(NOTHING_TO_LABEL.contains("3D view") && NOTHING_TO_LABEL.contains("scan"));
    // Once there is a volume the labels are read, and that is no longer said.
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, open());
    let mut state = AppState {
        notices: state.notices,
        ..ready()
    };
    assert_eq!(said(&state), [NOTHING_TO_LABEL]);
    assert_eq!(reduce(&mut state, open()).len(), 1);
    assert_eq!(said(&state), Vec::<String>::new());
}

#[test]
fn highlighting_veins_starts_once_there_is_a_volume_and_not_twice_at_once() {
    let find = || Action::Labelling(crate::labelling::LabellingAction::FindVeins);
    let mut state = showing(THICK_SCAN);
    assert_eq!(reduce(&mut state, find()), []);
    assert_eq!(state.labels.status, LabelsStatus::None);
    assert!(crate::labels::can_find_veins(&state).is_err());
    let mut state = ready();
    assert_eq!(crate::labels::can_find_veins(&state), Ok(()));
    let effects = reduce(&mut state, find());
    assert!(
        matches!(effects.as_slice(), [Effect::FindVeins { .. }]),
        "{effects:?}"
    );
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    assert!(
        crate::labels::can_find_veins(&state).is_err(),
        "not twice at once"
    );
}

#[test]
fn veins_are_put_onto_labels_open_on_the_study_and_open_alone_otherwise() {
    let find = || Action::Labelling(crate::labelling::LabellingAction::FindVeins);
    let (mut state, _) = crate::testing::labelled_head();
    let effects = reduce(&mut state, find());
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::FindVeins {
                onto: Some(crate::labels::NamesFrom::Nothing),
                ..
            }]
        ),
        "{effects:?}"
    );
    let mut state = ready();
    let effects = reduce(&mut state, find());
    assert!(
        matches!(effects.as_slice(), [Effect::FindVeins { onto: None, .. }]),
        "{effects:?}"
    );
}

#[test]
fn the_veins_found_are_drawn_through_the_other_parts_in_3d_and_no_other_part_is() {
    let part = |name: &str| volsa_labels::Part {
        name: name.to_owned(),
        group: String::new(),
        side: volsa_labels::Side::Midline,
        colour: [1, 2, 3],
        trust: volsa_labels::Trust::Model,
        voxels: 1,
        ml: 1.0,
    };
    let shown = crate::labels::PartShown {
        shown: true,
        opacity: 0.4,
    };
    let labels = crate::labels::Labels {
        parts: vec![part("Cortex"), part(volsa_segment::VEINS_PART)],
        shown: vec![shown; 2],
        ..crate::labels::Labels::default()
    };
    let through: Vec<bool> = labels.looks().iter().map(|look| look.through).collect();
    assert_eq!(through, [false, true]);
    assert!(
        (labels.looks()[1].opacity - 0.4).abs() < f32::EPSILON,
        "as strong as its slider"
    );
}

#[test]
fn labels_start_a_little_see_through_and_the_veins_found_fainter_still() {
    let part = |name: &str| volsa_labels::Part {
        name: name.to_owned(),
        group: String::new(),
        side: volsa_labels::Side::Midline,
        colour: [1, 2, 3],
        trust: volsa_labels::Trust::Model,
        voxels: 1,
        ml: 1.0,
    };
    let mut state = ready();
    reduce(&mut state, open());
    reduce(
        &mut state,
        Action::LabelsRead {
            title: String::new(),
            parts: vec![part("Cortex"), part(volsa_segment::VEINS_PART)],
            verdict: None,
            corners: super::scene_tests::corners(),
            names_from: crate::labels::NamesFrom::Table,
            made: None,
            run: None,
        },
    );
    let opacity: Vec<f32> = state.labels.shown.iter().map(|how| how.opacity).collect();
    let (parts, veins) = (crate::labels::PARTS_AT_FIRST, crate::labels::VEINS_AT_FIRST);
    assert_eq!(opacity, [parts, veins]);
    assert!(veins < parts && parts < 1.0, "both start below solid");
}
