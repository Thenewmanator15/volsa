use std::path::PathBuf;

use voxa_core::WindowLevel;

use super::*;
use crate::state::{Layout, NoticeKind, PresetWindows, ViewportState};
use crate::testing::{FINE_SCAN, SCOUT, THICK_SCAN, id, series_files, three_series};

/// A state with the three-series export indexed and nothing shown.
pub(super) fn loaded() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    state
}

/// A state showing a series in viewport 0.
pub(super) fn showing(series: &str) -> AppState {
    let mut state = loaded();
    reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id(series),
        },
    );
    state
}

pub(super) fn windows(center: f32) -> PresetWindows {
    [100.0, 200.0, 50.0].map(|width| WindowLevel::new(center, width))
}

pub(super) fn frames_around(series: &str, around: usize) -> Vec<Effect> {
    vec![Effect::RequestFrames {
        series: id(series),
        around,
    }]
}

#[test]
fn opening_a_path_starts_indexing_and_clears_what_was_shown() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    reduce(&mut state, Action::TogglePatient);
    reduce(&mut state, Action::IndexFailed("an old message".to_owned()));
    let path = PathBuf::from("C:/scans/export.zip");
    let effects = reduce(&mut state, Action::OpenPath(path.clone()));
    assert_eq!(effects, vec![Effect::StartIndex(path)]);
    assert_eq!(state.index, None);
    assert_eq!(state.viewports[0], ViewportState::default());
    assert_eq!(state.notices, Vec::new());
    assert_eq!(
        (state.layout, state.show_patient),
        (Layout::TwoAcross, true),
        "preferences stay"
    );
}

#[test]
fn switches_flip_and_the_tag_filter_is_kept() {
    let mut state = AppState::default();
    reduce(&mut state, Action::ToggleRadiological);
    reduce(&mut state, Action::TogglePatient);
    reduce(&mut state, Action::SetTagFilter("modal".to_owned()));
    assert_eq!((state.radiological, state.show_patient), (false, true));
    assert_eq!(state.tag_filter, "modal");
    reduce(&mut state, Action::ToggleRadiological);
    assert!(state.radiological);
}

#[test]
fn the_layout_keeps_the_active_viewport_within_those_shown() {
    let mut state = AppState::default();
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    reduce(&mut state, Action::SetActive(3));
    assert_eq!(state.active, 3);
    reduce(&mut state, Action::SetActive(4));
    assert_eq!(state.active, 3, "there is no fifth viewport");
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    assert_eq!(state.active, 0, "viewport 3 is no longer shown");
    reduce(&mut state, Action::SetActive(2));
    assert_eq!(state.active, 0);
}

#[test]
fn finishing_the_index_opens_the_most_detailed_series_once() {
    let mut state = loaded();
    let effects = reduce(&mut state, Action::IndexDone);
    // Thumbnails are asked for first, so the request for the whole series is the newer one.
    let mut expected = vec![Effect::RequestThumbnails];
    expected.extend(frames_around(FINE_SCAN, 32));
    expected.push(Effect::BuildVolume {
        series: id(FINE_SCAN),
    });
    expected.push(Effect::LoadViews {
        series: id(FINE_SCAN),
    });
    expected.push(Effect::LoadAnnotations {
        series: id(FINE_SCAN),
    });
    assert_eq!(effects, expected);
    assert_eq!(state.viewports[0].series, Some(id(FINE_SCAN)));

    reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id(THICK_SCAN),
        },
    );
    let again = reduce(&mut state, Action::IndexDone);
    // No other series is opened; the volume of the one shown is built again, in case
    // indexing added slices to it after it was chosen.
    let rebuilt = Effect::BuildVolume {
        series: id(THICK_SCAN),
    };
    let views = Effect::LoadViews {
        series: id(THICK_SCAN),
    };
    let annotations = Effect::LoadAnnotations {
        series: id(THICK_SCAN),
    };
    let expected = vec![Effect::RequestThumbnails, rebuilt, views, annotations];
    assert_eq!(again, expected);
    assert_eq!(
        state.viewports[0].series,
        Some(id(THICK_SCAN)),
        "the person's choice stands"
    );
}

#[test]
fn finishing_an_index_with_no_volume_fit_to_open_shows_the_scan_it_holds() {
    // Three slices are too few to be the best volume; with no other scan, they are shown.
    let mut state = AppState::default();
    let scout = crate::testing::indexed(series_files(SCOUT, 3, 3, 4));
    reduce(&mut state, Action::IndexProgress(scout));
    let effects = reduce(&mut state, Action::IndexDone);
    assert_eq!(effects.first(), Some(&Effect::RequestThumbnails));
    assert_eq!(state.viewports[0].series, Some(id(SCOUT)));
    // An export of no images at all shows nothing.
    let mut state = AppState::default();
    let nothing = voxa_dicom::Index {
        skipped: vec!["notes/readme.txt".to_owned()],
        ..voxa_dicom::Index::default()
    };
    reduce(&mut state, Action::IndexProgress(nothing));
    let effects = reduce(&mut state, Action::IndexDone);
    assert_eq!(effects, vec![Effect::RequestThumbnails]);
    assert_eq!(state.viewports[0].series, None);
}

#[test]
fn skipped_files_are_one_notice_that_keeps_up_with_the_count() {
    let mut index = three_series();
    index.skipped = vec!["notes/readme.txt".to_owned(), "report.pdf".to_owned()];
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index.clone()));
    assert_eq!(state.notices.len(), 1);
    let notice = &state.notices[0];
    assert_eq!(
        (notice.kind, notice.text.as_str()),
        (
            NoticeKind::Info,
            "2 files are not scans or documents Voxa lists"
        )
    );
    assert_eq!(notice.details, index.skipped);

    index.skipped.truncate(1);
    reduce(&mut state, Action::IndexProgress(index.clone()));
    let texts: Vec<&str> = state
        .notices
        .iter()
        .map(|notice| notice.text.as_str())
        .collect();
    assert_eq!(texts, vec!["1 file is not a scan or a document Voxa lists"]);

    index.skipped.clear();
    reduce(&mut state, Action::IndexProgress(index));
    assert_eq!(state.notices, Vec::new());
}

#[test]
fn a_failed_index_is_an_error_notice_that_can_be_dismissed() {
    let mut state = AppState::default();
    reduce(
        &mut state,
        Action::IndexFailed("That file is not a zip or DICOM export".to_owned()),
    );
    assert_eq!(state.notices.len(), 1);
    assert_eq!(state.notices[0].kind, NoticeKind::Error);
    assert_eq!(
        state.notices[0].text,
        "That file is not a zip or DICOM export"
    );
    reduce(&mut state, Action::DismissNotice(5));
    assert_eq!(state.notices.len(), 1, "there is no sixth notice");
    reduce(&mut state, Action::DismissNotice(0));
    assert_eq!(state.notices, Vec::new());
}

#[test]
fn a_slice_that_failed_is_remembered_with_its_reason_until_it_decodes() {
    let mut state = showing(THICK_SCAN);
    let failed = Action::FrameFailed {
        series: id(THICK_SCAN),
        slice: 4,
        reason: "The image could not be decoded.".to_owned(),
    };
    reduce(&mut state, failed);
    let key = (id(THICK_SCAN), 4);
    assert_eq!(
        state.failed.get(&key).map(String::as_str),
        Some("The image could not be decoded.")
    );
    assert_eq!(
        state.notices,
        Vec::new(),
        "one bad slice is shown in its place, not announced"
    );
    let ready = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 4,
        windows: windows(1.0),
    };
    reduce(&mut state, ready);
    assert_eq!(state.failed.get(&key), None);
}

#[test]
fn a_notice_posted_from_the_window_takes_the_place_of_the_last_with_its_key() {
    let mut state = AppState::default();
    let notice = |text: &str, key| Notice {
        kind: NoticeKind::Info,
        text: text.to_owned(),
        details: Vec::new(),
        key,
    };
    reduce(
        &mut state,
        Action::Post(notice("Saved one", Some("saving"))),
    );
    reduce(&mut state, Action::Post(notice("Something else", None)));
    reduce(
        &mut state,
        Action::Post(notice("Saved two", Some("saving"))),
    );
    let texts: Vec<&str> = state
        .notices
        .iter()
        .map(|notice| notice.text.as_str())
        .collect();
    assert_eq!(texts, ["Something else", "Saved two"]);
}
