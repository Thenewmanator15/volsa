//! The preset windows of a slice of a series held as a volume are worked out when the slice
//! is shown, and for no other.

use super::tests::{open_until, ready_in_3d, session, until, zip_of};
use crate::action::Action;
use crate::testing::{FINE_SCAN, id, three_series_files};

#[test]
fn the_windows_of_a_slice_shown_from_a_held_series_become_known() {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let has_its_windows = |session: &super::Session, slice: usize| {
        session
            .state()
            .windows
            .contains_key(&(id(FINE_SCAN), slice))
    };
    until(&mut session, |session| has_its_windows(session, 32));
    assert!(!has_its_windows(&session, 20), "slice 20 is not shown yet");
    session.dispatch(Action::SetSlice {
        viewport: 0,
        slice: 20,
    });
    until(&mut session, |session| has_its_windows(session, 20));
    assert!(
        !has_its_windows(&session, 50),
        "nor are the windows of a slice never shown worked out"
    );
}

#[test]
fn a_slice_from_the_volume_waits_for_its_viewports_automatic_window() {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let shown = &mut session.state.viewports[0];
    (shown.slice, shown.wl_pending) = (10, true);
    assert!(
        session.cached(&id(FINE_SCAN), 10).is_none(),
        "not drawn in a window chosen for nothing"
    );
    assert!(
        session.cached(&id(FINE_SCAN), 11).is_some(),
        "a slice no viewport waits on is read from the volume"
    );
    session.state.viewports[0].wl_pending = false;
    assert!(session.cached(&id(FINE_SCAN), 10).is_some());
}
