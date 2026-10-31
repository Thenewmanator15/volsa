//! How the pictures are arranged: chosen from the strip, kept from one export to the next in
//! a run, and the slices when Voxa opens an export.

use std::path::PathBuf;

use super::*;
use crate::pictures::Pictures;
use crate::prefs::Prefs;
use crate::state::Layout;
use crate::testing::{FINE_SCAN, id, three_series};

/// The state as a run starts it, put as the run before left it: four viewports.
fn started_with_four_viewports() -> AppState {
    let mut state = AppState::default();
    let remembered = Prefs {
        viewports: 4,
        ..Prefs::default()
    };
    for action in remembered.actions(&state) {
        reduce(&mut state, action);
    }
    state
}

/// Open an export of the three made-up series, to the end of indexing.
fn open(state: &mut AppState) {
    reduce(state, Action::OpenPath(PathBuf::from("export.zip")));
    reduce(state, Action::IndexProgress(three_series()));
    reduce(state, Action::IndexDone);
}

#[test]
fn an_export_opens_on_slices_with_the_remembered_viewports() {
    let mut state = started_with_four_viewports();
    open(&mut state);
    assert_eq!(state.pictures, Pictures::Slices);
    assert_eq!(state.layout, Layout::TwoByTwo);
    let index = state.index.as_ref().expect("indexed");
    let best = voxa_dicom::best_series(index).map(|series| series.info.id.clone());
    assert_eq!(best, Some(id(FINE_SCAN)), "the scan with the most voxels");
    assert_eq!(state.viewports[0].series, best);
}

#[test]
fn another_export_in_the_same_run_keeps_the_arrangement() {
    let mut state = started_with_four_viewports();
    open(&mut state);
    reduce(&mut state, Action::SetPictures(Pictures::Planes));
    open(&mut state);
    assert_eq!(
        (state.pictures, state.layout),
        (Pictures::Planes, Layout::TwoByTwo)
    );
}

#[test]
fn each_arrangement_is_chosen_and_shown_as_far_as_the_scan_allows() {
    let mut state = started_with_four_viewports();
    open(&mut state);
    for pictures in Pictures::ALL {
        reduce(&mut state, Action::SetPictures(pictures));
        assert_eq!(state.pictures, pictures);
        assert_eq!(state.pictures.given(&state.scene.status), pictures);
    }
    state.scene.status = crate::scene::SceneStatus::Unavailable(vec!["why".to_owned()]);
    assert_eq!(
        state.pictures.given(&state.scene.status),
        Pictures::Slices,
        "with no 3D view, the slices"
    );
}
