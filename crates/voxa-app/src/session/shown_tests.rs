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
