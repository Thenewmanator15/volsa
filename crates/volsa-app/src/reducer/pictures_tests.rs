//! How the pictures are arranged: chosen from the strip, kept from one export to the next in
//! a run, and the slices when Volsa opens an export.

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
    let best = volsa_dicom::best_series(index).map(|series| series.info.id.clone());
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

/// A made-up series numbered `number` of this modality, `side` pixels a side and `slices`
/// slices.
fn listed(number: i32, modality: &str, side: u32, slices: usize) -> volsa_dicom::IndexedSeries {
    let info = volsa_core::SeriesInfo {
        id: id(&format!("1.2.9.{number}")),
        number,
        modality: modality.to_owned(),
        rows: side,
        cols: side,
        slices,
        ..volsa_core::SeriesInfo::default()
    };
    let slices = (0..slices).map(|place| volsa_dicom::SliceRef {
        entry: format!("{number}/{place}.dcm"),
        frame: None,
    });
    volsa_dicom::IndexedSeries {
        info,
        slices: slices.collect(),
    }
}

/// A plain stack: a made-up series whose slices are not evenly spaced.
fn stack(number: i32, side: u32, slices: usize) -> volsa_dicom::IndexedSeries {
    let mut stack = listed(number, "MR", side, slices);
    stack.info.problems = vec![volsa_core::SeriesProblem::UnevenSpacing];
    stack
}

/// The series an export of these opens on.
fn opens_on(series: Vec<volsa_dicom::IndexedSeries>) -> Option<volsa_core::SeriesId> {
    let study = volsa_core::StudyInfo {
        id: volsa_core::StudyId("1.2.8.1".to_owned()),
        date: None,
        description: String::new(),
        patient_name: None,
        patient_id: None,
        series: series.iter().map(|listed| listed.info.clone()).collect(),
    };
    let index = volsa_dicom::Index {
        studies: vec![study],
        series,
        ..volsa_dicom::Index::default()
    };
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index));
    reduce(&mut state, Action::IndexDone);
    state.viewports[0].series.clone()
}

#[test]
fn with_no_volume_the_scan_of_the_most_pixels_opens_and_the_scans_come_first_always() {
    // No volume: the scan of the most pixels, the earlier of two as large.
    let stacks = vec![stack(1, 4, 20), stack(2, 8, 20), stack(3, 8, 20)];
    assert_eq!(opens_on(stacks), Some(id("1.2.9.2")));
    let single = listed(4, "DX", 64, 1);
    assert_eq!(opens_on(vec![stack(1, 8, 20), single]), Some(id("1.2.9.4")));
    // A page is never opened while there is a scan, however many pixels it has.
    let pages = listed(5, "SC", 64, 30);
    assert_eq!(opens_on(vec![pages, stack(1, 4, 20)]), Some(id("1.2.9.1")));
    // With none, the first of the other files.
    let only = vec![listed(6, "DOC", 4, 2), listed(5, "SC", 64, 30)];
    assert_eq!(opens_on(only), Some(id("1.2.9.6")));
}
