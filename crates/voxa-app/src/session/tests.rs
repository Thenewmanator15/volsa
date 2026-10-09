//! Opening scans from start to finish, with no window: state, workers and cache together.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::*;
use voxa_render::{Camera, NamedView};

use crate::scene::{Scene, SceneStatus};
use crate::state::NoticeKind;
use crate::testing::{
    FINE_SCAN, LOCALISER, THICK_SCAN, id, odd_series_files, series_files, three_series_files,
};

/// Long enough for any worker on any machine; a test that waits this long has failed.
const PATIENCE: Duration = Duration::from_secs(60);

fn session() -> Session {
    let (sender, reports) = channel();
    Session::new(sender, reports, 1 << 24)
}

/// A zip of the files, in a temporary folder that lasts as long as the first value returned.
fn zip_of(files: &[(String, Vec<u8>)]) -> (TempDir, PathBuf) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("export.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).expect("zip is created"));
    for (name, bytes) in files {
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file(name.as_str(), options)
            .expect("entry starts");
        writer.write_all(bytes).expect("entry is written");
    }
    writer.finish().expect("zip is finished");
    (folder, path)
}

/// Open a path and take in what the workers report until `done` holds.
fn open_until(session: &mut Session, path: &Path, done: impl Fn(&Session) -> bool) {
    session.dispatch(Action::OpenPath(path.to_owned()));
    let started = Instant::now();
    while !done(session) {
        assert!(
            started.elapsed() < PATIENCE,
            "still waiting: {:?}",
            session.state().notices
        );
        session.wait(Duration::from_millis(100));
    }
}

/// Whether the first viewport shows a series whose current slice has been decoded.
fn first_viewport_has_its_slice(session: &Session) -> bool {
    let viewport = &session.state().viewports[0];
    viewport
        .series
        .as_ref()
        .is_some_and(|series| session.cached(series, viewport.slice).is_some())
}

fn notice_texts(session: &Session) -> Vec<&str> {
    session
        .state()
        .notices
        .iter()
        .map(|notice| notice.text.as_str())
        .collect()
}

#[test]
fn opening_a_zip_shows_its_most_detailed_series_without_any_error() {
    let (_folder, zip) = zip_of(&three_series_files());
    let mut session = session();
    open_until(&mut session, &zip, first_viewport_has_its_slice);
    let state = session.state();
    let viewport = &state.viewports[0];
    assert_eq!(
        (viewport.series.clone(), viewport.slice),
        (Some(id(FINE_SCAN)), 32)
    );
    assert!(
        !viewport.wl_pending,
        "the automatic window came with the slice"
    );
    assert_eq!(
        state.index.as_ref().map(|index| index.series.len()),
        Some(3)
    );
    let errors = state
        .notices
        .iter()
        .filter(|notice| notice.kind == NoticeKind::Error)
        .count();
    assert_eq!(errors, 0, "{:?}", state.notices);
    let frame = session.cached(&id(FINE_SCAN), 32).expect("decoded");
    assert_eq!((frame.rows, frame.cols), (8, 8));
}

#[test]
fn opening_an_export_with_other_files_in_it_says_how_many_were_skipped() {
    let mut files = series_files(THICK_SCAN, 1, 12, 4);
    files.push(("readme.txt".to_owned(), b"about this export".to_vec()));
    let (_folder, zip) = zip_of(&files);
    let mut session = session();
    open_until(&mut session, &zip, first_viewport_has_its_slice);
    assert_eq!(notice_texts(&session), ["1 file skipped"]);
    assert_eq!(session.state().notices[0].details, ["readme.txt"]);
}

#[test]
fn opening_a_text_file_says_it_is_not_an_export() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("notes.txt");
    std::fs::write(&path, "not a scan").expect("writes");
    let mut session = session();
    let said = |session: &Session| {
        session
            .state()
            .notices
            .iter()
            .any(|n| n.kind == NoticeKind::Error)
    };
    open_until(&mut session, &path, said);
    assert!(notice_texts(&session).contains(&"That file is not a zip or DICOM export"));
    assert_eq!(session.state().viewports[0].series, None);
}

#[test]
fn opening_an_empty_folder_says_no_images_were_found_there() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let empty = folder.path().join("nothing here");
    std::fs::create_dir(&empty).expect("makes the folder");
    let mut session = session();
    let said = |session: &Session| !session.state().notices.is_empty();
    open_until(&mut session, &empty, said);
    assert_eq!(
        notice_texts(&session),
        ["No images were found in nothing here"]
    );
}

#[test]
fn opening_a_second_export_replaces_the_first() {
    let (_first_folder, first) = zip_of(&series_files(THICK_SCAN, 1, 12, 4));
    let (_second_folder, second) = zip_of(&series_files("1.2.3.77", 7, 14, 4));
    let mut session = session();
    open_until(&mut session, &first, first_viewport_has_its_slice);
    assert_eq!(session.state().viewports[0].series, Some(id(THICK_SCAN)));
    let shows_second = |session: &Session| {
        first_viewport_has_its_slice(session)
            && session.state().viewports[0].series == Some(id("1.2.3.77"))
    };
    open_until(&mut session, &second, shows_second);
    let listed: Vec<&str> = session
        .state()
        .index
        .iter()
        .flat_map(|index| &index.series)
        .map(|s| s.info.id.0.as_str())
        .collect();
    assert_eq!(listed, ["1.2.3.77"]);
    assert!(
        session.cached(&id(THICK_SCAN), 6).is_none(),
        "nothing of the first is kept"
    );
}

/// Whether the scene shows this series with its volume built.
fn ready_in_3d(session: &Session, series: &str) -> bool {
    let scene = &session.state().scene;
    scene.series == Some(id(series)) && scene.status == SceneStatus::Ready
}

#[test]
fn opening_a_zip_ends_with_the_most_detailed_series_ready_in_3d() {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let volume = session.volume(&id(FINE_SCAN)).expect("its volume is kept");
    assert_eq!(volume.dims(), [8, 8, 64]);
    let scene = &session.state().scene;
    assert_eq!(scene.corners, Some(volume.corners()));
    let fitted = Camera::fitted(
        NamedView::Anterior,
        &volume.corners(),
        Scene::FIRST_VIEWPORT,
    );
    assert_eq!(scene.camera, fitted, "seen whole, from the front");
}

#[test]
fn choosing_another_series_puts_its_volume_in_place_of_the_first() {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let (viewport, series) = (0, id(THICK_SCAN));
    session.dispatch(Action::SelectSeries { viewport, series });
    let started = Instant::now();
    while !ready_in_3d(&session, THICK_SCAN) {
        assert!(
            started.elapsed() < PATIENCE,
            "{:?}",
            session.state().scene.status
        );
        session.wait(Duration::from_millis(100));
    }
    assert!(session.volume(&id(FINE_SCAN)).is_none());
    let dims = session.volume(&id(THICK_SCAN)).map(|volume| volume.dims());
    assert_eq!(dims, Some([4, 4, 30]));
}

#[test]
fn a_series_with_no_3d_view_says_so_and_its_slices_are_still_shown() {
    let mut session = session();
    let (_folder, path) = zip_of(&odd_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let (viewport, series) = (0, id(LOCALISER));
    session.dispatch(Action::SelectSeries { viewport, series });
    let SceneStatus::Unavailable(lines) = &session.state().scene.status else {
        panic!("{:?}", session.state().scene.status);
    };
    assert_eq!(
        lines.first().map(String::as_str),
        Some("This series cannot be shown in 3D.")
    );
    assert!(
        session.volume(&id(FINE_SCAN)).is_some(),
        "nothing new was built"
    );
    let started = Instant::now();
    while !first_viewport_has_its_slice(&session) {
        assert!(started.elapsed() < PATIENCE, "the 2D viewport carries on");
        session.wait(Duration::from_millis(100));
    }
}
