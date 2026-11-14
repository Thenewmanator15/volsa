//! Labels in the state: opening and closing them, what is shown, faded and selected, and
//! which series they are shown on.

use std::path::PathBuf;

use volsa_core::{Patient, Point};
use volsa_labels::{Made, Part, Side, Trust, Verdict};
use volsa_render::{Look, Overlay};

use super::scene_tests::{corners, ready, select};
use super::tests::showing;
use super::*;
use crate::labels::{
    Labels, LabelsStatus, NOWHERE_NEAR, NamesFrom, PartShown, look_drawn, on_this_series,
};
use crate::testing::{FINE_SCAN, THICK_SCAN};

fn part(name: &str, group: &str, side: Side) -> Part {
    Part {
        name: name.to_owned(),
        group: group.to_owned(),
        side,
        colour: [10, 20, 30],
        trust: Trust::Model,
        voxels: 1000,
        ml: 1.0,
    }
}

fn parts() -> Vec<Part> {
    vec![
        part("hippocampus", "Deep brain", Side::Left),
        part("hippocampus", "Deep brain", Side::Right),
        part("brain stem", "Cerebellum and brainstem", Side::Midline),
    ]
}

fn open() -> Action {
    Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    }
}

/// What a worker reports of labels whose box is `corners`.
fn read(corners: [Point<Patient>; 8]) -> Action {
    Action::LabelsRead {
        title: "A made-up head".to_owned(),
        parts: parts(),
        verdict: None,
        corners,
        names_from: NamesFrom::Table,
        made: None,
        run: None,
    }
}

/// The 30-slice series in the scene with its volume ready, and labels of it read.
fn labelled() -> AppState {
    let mut state = ready();
    reduce(&mut state, open());
    reduce(&mut state, read(corners()));
    state
}

fn shown(state: &AppState) -> Vec<bool> {
    state.labels.shown.iter().map(|part| part.shown).collect()
}

#[test]
fn opening_labels_needs_a_volume_and_asks_a_worker_to_read_them() {
    let mut building = showing(THICK_SCAN);
    assert_eq!(
        reduce(&mut building, open()),
        [],
        "no volume yet: nothing to put them on"
    );
    assert_eq!(building.labels, Labels::default());
    let mut state = ready();
    let effects = reduce(&mut state, open());
    let asked = Effect::ReadLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    assert_eq!(effects, [asked]);
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    let index = state.index.as_ref().expect("indexed");
    assert_eq!(
        state.labels.study,
        Some(index.studies[0].id.clone()),
        "the scene's study"
    );
    assert!(!on_this_series(&state), "not while they are being read");
}

#[test]
fn labels_that_are_read_start_all_shown_a_little_see_through_and_outlined() {
    let state = labelled();
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    assert_eq!(state.labels.title, "A made-up head");
    assert_eq!(state.labels.parts, parts());
    let solid = PartShown {
        shown: true,
        opacity: crate::labels::PARTS_AT_FIRST,
    };
    assert_eq!(state.labels.shown, [solid; 3]);
    assert_eq!(
        (state.labels.selected, state.labels.overlay),
        (None, Overlay::Outline)
    );
    assert_eq!(state.labels.names_from, NamesFrom::Table);
    assert!(on_this_series(&state));
    let looks = state.labels.looks();
    assert_eq!(looks.len(), 3);
    assert!(
        looks
            .iter()
            .all(|look| look.colour == [10, 20, 30] && look.opacity < 1.0 && !look.through)
    );
}

#[test]
fn labels_nowhere_near_the_series_are_refused_and_dropped() {
    let mut state = ready();
    reduce(&mut state, open());
    let far = corners().map(|corner| Point::new(corner.x + 900.0, corner.y, corner.z));
    let effects = reduce(&mut state, read(far));
    assert_eq!(effects, [Effect::DropLabels]);
    assert_eq!(
        state.labels.status,
        LabelsStatus::Failed(NOWHERE_NEAR.to_owned())
    );
    assert!(NOWHERE_NEAR.starts_with("These labels lie nowhere near this series."));
    assert!(state.labels.parts.is_empty() && !on_this_series(&state));
}

#[test]
fn a_read_that_fails_says_why_and_one_that_nobody_is_waiting_for_is_ignored() {
    let mut state = ready();
    reduce(&mut state, open());
    reduce(
        &mut state,
        Action::LabelsFailed("It is not labels.".to_owned()),
    );
    assert_eq!(
        state.labels.status,
        LabelsStatus::Failed("It is not labels.".to_owned())
    );
    // Nothing is being read now, so what arrives late changes nothing.
    let before = state.clone();
    reduce(&mut state, read(corners()));
    reduce(&mut state, Action::LabelsFailed("Late.".to_owned()));
    assert_eq!(state, before);
    // Another export opened while labels are read: the read's answer is not for it.
    let mut state = ready();
    reduce(&mut state, open());
    let effects = reduce(&mut state, Action::OpenPath(PathBuf::from("another")));
    assert!(effects.contains(&Effect::DropLabels), "{effects:?}");
    assert_eq!(state.labels.status, LabelsStatus::None);
    reduce(&mut state, read(corners()));
    assert_eq!(state.labels.status, LabelsStatus::None);
}

#[test]
fn closing_labels_forgets_them_and_keeps_how_they_were_drawn() {
    let mut state = labelled();
    reduce(&mut state, Action::SetOverlay(Overlay::Filled));
    let effects = reduce(&mut state, Action::CloseLabels);
    assert_eq!(effects, [Effect::DropLabels]);
    let closed = Labels {
        overlay: Overlay::Filled,
        ..Labels::default()
    };
    assert_eq!(state.labels, closed);
    assert_eq!(reduce(&mut state, Action::SetOverlay(Overlay::Off)), []);
    assert_eq!(state.labels.overlay, Overlay::Off);
}

#[test]
fn parts_are_hidden_shown_isolated_and_brought_back() {
    let mut state = labelled();
    reduce(
        &mut state,
        Action::PartShow {
            part: 1,
            shown: false,
        },
    );
    assert_eq!(shown(&state), [true, false, true]);
    assert_eq!(
        state.labels.looks()[1].opacity,
        0.0,
        "hidden is no opacity at all"
    );
    reduce(
        &mut state,
        Action::PartOpacity {
            part: 0,
            opacity: 0.4,
        },
    );
    reduce(&mut state, Action::PartIsolate(2));
    assert_eq!(shown(&state), [false, false, true]);
    reduce(&mut state, Action::PartsShowAll);
    assert_eq!(shown(&state), [true, true, true]);
    assert_eq!(
        state.labels.shown[0].opacity, 0.4,
        "each as solid as it was"
    );
    let group = "Deep brain".to_owned();
    reduce(
        &mut state,
        Action::GroupShow {
            group: group.clone(),
            shown: false,
        },
    );
    assert_eq!(
        shown(&state),
        [false, false, true],
        "a group's parts and no others"
    );
    reduce(&mut state, Action::GroupShow { group, shown: true });
    assert_eq!(shown(&state), [true, true, true]);
}

#[test]
fn what_is_asked_of_a_part_or_group_that_is_not_there_is_ignored() {
    let mut state = labelled();
    let before = state.clone();
    reduce(
        &mut state,
        Action::PartShow {
            part: 9,
            shown: false,
        },
    );
    reduce(&mut state, Action::PartIsolate(9));
    reduce(
        &mut state,
        Action::PartOpacity {
            part: 9,
            opacity: 0.5,
        },
    );
    reduce(
        &mut state,
        Action::GroupShow {
            group: "No such group".to_owned(),
            shown: false,
        },
    );
    assert_eq!(state, before);
}

#[test]
fn a_part_is_faded_between_just_visible_and_solid() {
    let mut state = labelled();
    let fade = |state: &mut AppState, opacity| {
        reduce(state, Action::PartOpacity { part: 0, opacity });
        state.labels.shown[0].opacity
    };
    assert_eq!(fade(&mut state, 0.5), 0.5);
    assert_eq!(fade(&mut state, 7.0), 1.0);
    assert_eq!(
        fade(&mut state, -3.0),
        0.1,
        "fading never hides: the switch does that"
    );
    assert_eq!(
        fade(&mut state, f32::NAN),
        0.1,
        "what is not a number changes nothing"
    );
}

#[test]
fn a_part_is_selected_while_it_is_shown_and_hiding_it_lets_it_go() {
    let mut state = labelled();
    reduce(&mut state, Action::PartSelect(Some(1)));
    assert_eq!(state.labels.selected, Some(1));
    reduce(&mut state, Action::PartSelect(Some(9)));
    assert_eq!(
        state.labels.selected,
        Some(1),
        "a part that is not there is not selected"
    );
    reduce(
        &mut state,
        Action::PartShow {
            part: 1,
            shown: false,
        },
    );
    assert_eq!(state.labels.selected, None);
    reduce(&mut state, Action::PartSelect(Some(1)));
    assert_eq!(state.labels.selected, None, "nor is a hidden one");
    reduce(&mut state, Action::PartSelect(Some(0)));
    reduce(&mut state, Action::PartIsolate(2));
    assert_eq!(state.labels.selected, None, "isolating another hides it");
    reduce(&mut state, Action::PartSelect(Some(2)));
    reduce(
        &mut state,
        Action::GroupShow {
            group: "Cerebellum and brainstem".to_owned(),
            shown: false,
        },
    );
    assert_eq!(state.labels.selected, None);
    reduce(&mut state, Action::PartsShowAll);
    reduce(&mut state, Action::PartSelect(Some(0)));
    reduce(&mut state, Action::PartSelect(None));
    assert_eq!(state.labels.selected, None);
}

#[test]
fn labels_stay_with_their_study_and_are_not_forgotten_when_another_is_looked_at() {
    let mut state = labelled();
    // Another series of the same study: the scene moves to it, and the labels go with it
    // once its volume is ready.
    select(&mut state, 0, FINE_SCAN);
    assert!(
        on_this_series(&state),
        "the same study, so the same patient where they lay"
    );
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    // A study the labels are not of.
    state.labels.study = Some(volsa_core::StudyId("another study".to_owned()));
    assert!(!on_this_series(&state));
    assert_eq!(state.labels.parts.len(), 3, "still held");
}

#[test]
fn the_parts_look_needs_labels_on_the_series_and_is_drawn_as_a_surface_without() {
    let mut state = ready();
    reduce(&mut state, Action::SceneLook(Look::Parts));
    assert_eq!(
        state.scene.look,
        Look::Surface,
        "no labels: the look is not taken up"
    );
    let mut state = labelled();
    reduce(&mut state, Action::SceneLook(Look::Parts));
    assert_eq!(
        (state.scene.look, look_drawn(&state)),
        (Look::Parts, Look::Parts)
    );
    state.labels.study = Some(volsa_core::StudyId("another study".to_owned()));
    assert_eq!(
        look_drawn(&state),
        Look::Surface,
        "nothing for it to draw on this series"
    );
    reduce(&mut state, Action::CloseLabels);
    assert_eq!(look_drawn(&state), Look::Surface);
    reduce(&mut state, Action::SceneLook(Look::Mip));
    assert_eq!(
        look_drawn(&state),
        Look::Mip,
        "the other looks are as chosen"
    );
}

#[test]
fn the_models_check_and_who_made_the_labels_come_with_them_and_go_with_them() {
    let mut state = ready();
    reduce(&mut state, open());
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("general csf".to_owned(), 0.6)],
    };
    let read = Action::LabelsRead {
        title: String::new(),
        parts: parts(),
        verdict: Some(verdict.clone()),
        corners: corners(),
        names_from: NamesFrom::Nothing,
        made: Some(Made {
            by: "Volsa 9.8.7".to_owned(),
            ..Made::default()
        }),
        run: None,
    };
    reduce(&mut state, read);
    assert_eq!(state.labels.verdict, Some(verdict));
    assert_eq!(state.labels.names_from, NamesFrom::Nothing);
    let by = state.labels.made.as_ref().map(|made| made.by.as_str());
    assert_eq!(by, Some("Volsa 9.8.7"));
    reduce(&mut state, Action::CloseLabels);
    assert_eq!(state.labels.made, None);
}
