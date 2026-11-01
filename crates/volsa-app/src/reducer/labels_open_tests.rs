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
