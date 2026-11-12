//! Where each series was left, kept from one session to the next: through the real state and
//! workers, on a made-up scan, with the places kept in a temporary folder.

use volsa_core::WindowLevel;

use super::tests::{open_until, session_keeping, until, zip_of};
use super::*;
use crate::places::{Place, PlacesAction};
use crate::testing::{FINE_SCAN, SCOUT, THICK_SCAN, id, series_files, three_series_files};

/// The file the places are kept in.
const FILE: &str = "places.json";

/// Whether what is kept for the export that is open has been read.
fn read(session: &Session) -> bool {
    session.state().places.read
}

/// The series, the slice and the place of what the first viewport shows.
fn shown(session: &Session) -> (Option<SeriesId>, Place) {
    let viewport = &session.state().viewports[0];
    (viewport.series.clone(), Place::of(viewport))
}

/// Choose the 30-slice series and leave it on its fifth slice, turned, zoomed and in a window
/// of the person's own; where that leaves it.
fn leave_the_thick_scan_somewhere(session: &mut Session) -> Place {
    let (viewport, series) = (0, id(THICK_SCAN));
    session.dispatch(Action::SelectSeries { viewport, series });
    session.dispatch(Action::SetSlice { viewport, slice: 4 });
    let wl = WindowLevel::new(55.0, 345.0);
    session.dispatch(Action::SetWindow { viewport, wl });
    session.dispatch(Action::Rotate { viewport });
    let (factor, about) = (2.0, [8.0, -6.0]);
    session.dispatch(Action::Zoom {
        viewport,
        factor,
        about,
    });
    let (series, place) = shown(session);
    assert_eq!(series, Some(id(THICK_SCAN)));
    assert_eq!((place.slice, place.turns), (4, 1));
    assert_eq!(place.window, Some([55.0, 345.0]));
    place
}

#[test]
fn a_study_opened_again_comes_back_to_the_series_slice_window_and_view_it_was_left_at() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let (folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, read);
    assert_eq!(shown(&session), (Some(id(FINE_SCAN)), Place::fresh(64)));
    assert!(
        !kept.path().join(FILE).exists(),
        "only opened: nothing to keep"
    );
    let left = leave_the_thick_scan_somewhere(&mut session);
    // The window closes.
    session.leave();
    let text = std::fs::read_to_string(kept.path().join(FILE)).expect("written on the way out");
    let index = session.state().index.as_ref().expect("indexed");
    let study = &index.studies[0];
    let folder_name = folder.path().file_name().and_then(|name| name.to_str());
    let private = [
        Some(THICK_SCAN.to_owned()),
        Some(FINE_SCAN.to_owned()),
        Some(study.id.0.clone()),
        Some(study.description.clone()),
        study.date.clone(),
        study.patient_name.clone(),
        study.patient_id.clone(),
        folder_name.map(str::to_owned),
        Some("export".to_owned()),
    ];
    for word in private.iter().flatten().filter(|word| !word.is_empty()) {
        assert!(!text.contains(word.as_str()), "the file names {word}");
    }
    drop(session);

    let mut again = session_keeping(kept.path());
    open_until(&mut again, &path, read);
    assert_eq!(shown(&again), (Some(id(THICK_SCAN)), left));
    let viewport = &again.state().viewports[0];
    assert!(!viewport.wl_auto && !viewport.wl_pending);
    assert_eq!(viewport.wl, WindowLevel::new(55.0, 345.0));
    assert_eq!(again.state().scene.series, Some(id(THICK_SCAN)));
    // The series nothing was done to opens as it always does.
    let (viewport, series) = (0, id(FINE_SCAN));
    again.dispatch(Action::SelectSeries { viewport, series });
    assert_eq!(shown(&again), (Some(id(FINE_SCAN)), Place::fresh(64)));
}

#[test]
fn opening_another_export_keeps_where_the_series_of_the_first_were_left() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let (_folder, path) = zip_of(&three_series_files());
    let (_other_folder, other) = zip_of(&series_files("1.2.3.77", 7, 14, 4));
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, read);
    let left = leave_the_thick_scan_somewhere(&mut session);
    open_until(&mut session, &other, read);
    assert_eq!(shown(&session), (Some(id("1.2.3.77")), Place::fresh(14)));
    let file = kept.path().join(FILE);
    until(&mut session, |_| file.exists());
    open_until(&mut session, &path, read);
    assert_eq!(shown(&session), (Some(id(THICK_SCAN)), left));
}

#[test]
fn a_series_opened_by_the_person_is_shown_whatever_series_was_kept_as_the_first() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, read);
    leave_the_thick_scan_somewhere(&mut session);
    session.leave();
    drop(session);

    let mut again = session_keeping(kept.path());
    again.dispatch(Action::OpenPath(path));
    // Chosen while the export is still being read, as soon as it is listed.
    let listed = |session: &Session| session.state().series(&id(SCOUT)).is_some();
    until(&mut again, listed);
    let (viewport, series) = (0, id(SCOUT));
    again.dispatch(Action::SelectSeries { viewport, series });
    until(&mut again, read);
    assert_eq!(again.state().viewports[0].series, Some(id(SCOUT)));
}

#[test]
fn closing_before_what_is_kept_has_been_read_leaves_the_file_as_it_was() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let file = kept.path().join(FILE);
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, read);
    leave_the_thick_scan_somewhere(&mut session);
    session.leave();
    drop(session);
    let before = std::fs::read_to_string(&file).expect("reads");

    // The same study is being opened when the window closes: nothing has been read yet.
    let mut again = session_keeping(kept.path());
    again.dispatch(Action::OpenPath(path));
    again.dispatch(Action::Places(PlacesAction::Keep));
    again.leave();
    assert!(!read(&again));
    assert_eq!(std::fs::read_to_string(&file).expect("reads"), before);
}

#[test]
fn a_damaged_file_of_places_is_put_aside_whole_before_another_is_written() {
    let kept = tempfile::tempdir().expect("a temporary folder");
    let file = kept.path().join(FILE);
    std::fs::write(&file, "what another program left here").expect("writes");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(kept.path());
    open_until(&mut session, &path, read);
    assert_eq!(shown(&session), (Some(id(FINE_SCAN)), Place::fresh(64)));
    let left = leave_the_thick_scan_somewhere(&mut session);
    session.leave();
    let aside = kept.path().join("places.json.unread");
    assert_eq!(
        std::fs::read_to_string(aside).expect("put aside"),
        "what another program left here"
    );
    drop(session);
    let mut again = session_keeping(kept.path());
    open_until(&mut again, &path, read);
    assert_eq!(shown(&again), (Some(id(THICK_SCAN)), left));
}
