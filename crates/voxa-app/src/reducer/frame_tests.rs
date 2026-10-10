//! What the scene's camera is fitted to: the scan, or, in the Parts look, labels that reach
//! beyond it.

use std::path::PathBuf;

use voxa_core::{Patient, Point};
use voxa_render::{Camera, Look, NamedView};

use super::scene_tests::{corners, ready};
use super::*;
use crate::labels::NamesFrom;

/// The scan's box made `times` as large each way about its own middle.
fn grown(times: f64) -> [Point<Patient>; 8] {
    let middle = [5.0, -7.0, 50.0];
    corners().map(|corner| {
        let [x, y, z] = [corner.x, corner.y, corner.z];
        let [x, y, z] = [x - middle[0], y - middle[1], z - middle[2]].map(|part| part * times);
        Point::new(x + middle[0], y + middle[1], z + middle[2])
    })
}

/// The scan ready, with labels read on it whose box is `of`.
fn labelled(of: [Point<Patient>; 8]) -> AppState {
    let mut state = ready();
    let open = Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    reduce(&mut state, open);
    let read = Action::LabelsRead {
        title: String::new(),
        parts: Vec::new(),
        verdict: None,
        corners: of,
        names_from: NamesFrom::Nothing,
    };
    reduce(&mut state, read);
    state
}

fn alike(a: &Camera, b: &Camera) -> bool {
    let apart = |a: Point<Patient>, b: Point<Patient>| {
        (a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs()
    };
    apart(a.target, b.target) < 1e-6 && (a.mm_per_px - b.mm_per_px).abs() < 1e-9
}

fn fitted(view: NamedView, to: &[Point<Patient>; 8], state: &AppState) -> Camera {
    Camera::fitted(view, to, state.scene.viewport)
}

#[test]
fn labels_no_larger_than_the_scan_leave_the_camera_where_it_is() {
    for labels in [corners(), grown(0.2), grown(1.04)] {
        let mut state = labelled(labels);
        reduce(&mut state, Action::SceneOrbit { by: [40.0, 10.0] });
        let before = state.scene.camera;
        reduce(&mut state, Action::SceneLook(Look::Parts));
        assert_eq!(state.scene.camera, before, "the same view, as solids");
    }
}

#[test]
fn labels_that_reach_beyond_the_scan_are_fitted_in_the_parts_look_and_the_scan_after_it() {
    let labels = grown(3.0);
    let mut state = labelled(labels);
    let on_the_scan = state.scene.camera;
    assert!(alike(
        &on_the_scan,
        &fitted(NamedView::Anterior, &corners(), &state)
    ));
    reduce(&mut state, Action::SceneLook(Look::Parts));
    let on_the_labels = fitted(NamedView::Anterior, &labels, &state);
    assert!(
        alike(&state.scene.camera, &on_the_labels),
        "the whole of the solids"
    );
    assert!(state.scene.camera.mm_per_px > 2.5 * on_the_scan.mm_per_px);
    reduce(&mut state, Action::SceneLook(Look::Mip));
    assert!(
        alike(&state.scene.camera, &on_the_scan),
        "and the scan again"
    );
}

#[test]
fn a_camera_the_person_turned_is_fitted_facing_as_they_left_it() {
    let labels = grown(3.0);
    let mut state = labelled(labels);
    reduce(&mut state, Action::SceneOrbit { by: [60.0, 20.0] });
    let turned = state.scene.camera;
    reduce(&mut state, Action::SceneLook(Look::Parts));
    let expected = turned.refitted(&labels, state.scene.viewport);
    assert_eq!(
        state.scene.camera, expected,
        "the labels, from where they were looking"
    );
    assert_eq!(state.scene.camera.axes(), turned.axes());
}

#[test]
fn the_named_views_and_a_new_size_fit_what_the_look_draws() {
    let labels = grown(3.0);
    let mut state = labelled(labels);
    reduce(&mut state, Action::SceneLook(Look::Parts));
    reduce(&mut state, Action::SceneView(NamedView::Left));
    assert!(alike(
        &state.scene.camera,
        &fitted(NamedView::Left, &labels, &state)
    ));
    reduce(&mut state, Action::CloseLabels);
    assert!(
        alike(
            &state.scene.camera,
            &fitted(NamedView::Left, &corners(), &state)
        ),
        "with the labels closed there is only the scan to fit"
    );
    // An untouched camera in a window that changes size is fitted to the solids too.
    let mut state = labelled(labels);
    reduce(&mut state, Action::SceneLook(Look::Parts));
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [900.0, 500.0],
        },
    );
    assert!(alike(
        &state.scene.camera,
        &fitted(NamedView::Anterior, &labels, &state)
    ));
}
