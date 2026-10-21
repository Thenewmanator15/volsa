//! What Voxa remembers between runs of how the person had it set.

use voxa_render::Overlay;

use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};

fn changed() -> Prefs {
    Prefs {
        window: Some([1200.0, 800.0]),
        viewports: 4,
        radiological: false,
        linked: false,
        overlay: "filled".to_owned(),
    }
}

#[test]
fn what_is_kept_comes_back_as_it_was() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    save(folder.path(), &changed()).expect("saves");
    assert_eq!(load(folder.path()), changed());
    assert_eq!(load(&folder.path().join("not there")), Prefs::default());
}

#[test]
fn a_file_that_is_not_what_voxa_wrote_costs_only_the_memory() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(folder.path().join(FILE), "not what Voxa wrote").expect("writes");
    assert_eq!(load(folder.path()), Prefs::default());
    // A file from a Voxa that kept less: what is missing is as Voxa starts.
    std::fs::write(folder.path().join(FILE), r#"{"viewports": 2}"#).expect("writes");
    let partly = Prefs {
        viewports: 2,
        ..Prefs::default()
    };
    assert_eq!(load(folder.path()), partly);
    // A folder that is a file: nothing can be kept, and that is an error, not a crash.
    let not_a_folder = folder.path().join("in the way");
    std::fs::write(&not_a_folder, "a file").expect("writes");
    assert!(save(&not_a_folder, &changed()).is_err());
    assert_eq!(load(&not_a_folder), Prefs::default());
}

#[test]
fn a_window_too_small_or_of_no_size_is_not_taken() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for size in [
        "[300.0, 200.0]",
        "[1200.0, 100.0]",
        "[0.0, 0.0]",
        "[-5.0, 900.0]",
    ] {
        let text = format!(r#"{{"window": {size}}}"#);
        std::fs::write(folder.path().join(FILE), text).expect("writes");
        assert_eq!(load(folder.path()).window, None, "{size}");
    }
    let text = r#"{"window": [1200.0, 800.0], "viewports": 3, "overlay": "sideways"}"#;
    std::fs::write(folder.path().join(FILE), text).expect("writes");
    let read = load(folder.path());
    assert_eq!(read.window, Some([1200.0, 800.0]));
    assert_eq!(
        (read.viewports, read.overlay.as_str()),
        (1, "outline"),
        "nonsense is not kept"
    );
}

#[test]
fn the_preferences_are_read_off_the_state_as_it_starts_and_as_it_is_changed() {
    let mut state = AppState::default();
    assert_eq!(Prefs::of(&state, None), Prefs::default());
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    reduce(&mut state, Action::ToggleRadiological);
    reduce(&mut state, Action::SetLinked(false));
    reduce(&mut state, Action::SetOverlay(Overlay::Filled));
    assert_eq!(Prefs::of(&state, Some([1200.0, 800.0])), changed());
}

#[test]
fn putting_them_back_asks_only_for_what_differs() {
    let mut state = AppState::default();
    assert_eq!(Prefs::default().actions(&state), Vec::new());
    let actions = changed().actions(&state);
    assert_eq!(actions.len(), 4);
    for action in actions {
        reduce(&mut state, action);
    }
    assert_eq!(Prefs::of(&state, Some([1200.0, 800.0])), changed());
    assert_eq!(
        changed().actions(&state),
        Vec::new(),
        "nothing left to put back"
    );
}

#[test]
fn with_nowhere_to_keep_them_nothing_is_read_or_written() {
    let mut kept = Kept::new(None);
    assert_eq!(kept.remembered(), &Prefs::default());
    assert!(!kept.keep(&changed()), "nowhere to write");
    let folder = tempfile::tempdir().expect("a temporary folder");
    let mut kept = Kept::new(Some(folder.path().to_owned()));
    assert!(!kept.keep(&Prefs::default()), "nothing has changed");
    assert!(!folder.path().join(FILE).exists());
    assert!(kept.keep(&changed()), "written once");
    assert!(!kept.keep(&changed()), "and not again while it is the same");
    assert_eq!(load(folder.path()), changed());
}
