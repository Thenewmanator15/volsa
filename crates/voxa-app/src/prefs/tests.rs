//! What Voxa remembers between runs of how the person had it set.

use voxa_render::Overlay;

use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};

/// The window as the person left it in [`changed`].
fn arranged() -> Arrangement {
    Arrangement {
        window: Some([1200.0, 800.0]),
        interface_size: 1.25,
        list_share: 0.2,
        column_share: 0.45,
        panel_share: Some(0.3),
        panel_folded: true,
        tab: "Scene".to_owned(),
    }
}

fn changed() -> Prefs {
    Prefs {
        window: Some([1200.0, 800.0]),
        interface_size: 1.25,
        list_share: 0.2,
        column_share: 0.45,
        panel_share: Some(0.3),
        panel_folded: true,
        tab: "Scene".to_owned(),
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
        "[639.0, 480.0]",
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
fn the_smallest_window_is_640_by_480_points() {
    assert_eq!(SMALLEST_WINDOW, [640.0, 480.0]);
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(folder.path().join(FILE), r#"{"window": [640.0, 480.0]}"#).expect("writes");
    assert_eq!(load(folder.path()).window, Some([640.0, 480.0]));
}

#[test]
fn the_interface_size_and_the_shares_start_as_voxa_has_them() {
    let start = Prefs::default().arrangement();
    assert_eq!(start.window, None);
    assert_eq!(start.interface_size, 1.0, "the interface's usual size");
    assert_eq!(start.list_share, LIST_SHARE);
    assert_eq!(start.column_share, COLUMN_SHARE);
    assert_eq!(start, Arrangement::default());
}

#[test]
fn an_interface_size_or_a_share_that_makes_no_sense_is_not_taken() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let read = |text: &str| {
        std::fs::write(folder.path().join(FILE), text).expect("writes");
        load(folder.path()).arrangement()
    };
    // Too small or too large to work in is held to what can be worked in.
    assert_eq!(read(r#"{"interface_size": 0.2}"#).interface_size, 0.5);
    assert_eq!(read(r#"{"interface_size": 9.0}"#).interface_size, 3.0);
    assert_eq!(read(r#"{"interface_size": 1.5}"#).interface_size, 1.5);
    assert_eq!(read(r#"{"interface_size": -1.0}"#).interface_size, 1.0);
    // A share is a part of the width: more than none and less than all of it.
    for share in ["0.0", "-0.2", "1.0", "4.0"] {
        let text = format!(r#"{{"list_share": {share}, "column_share": {share}}}"#);
        let read = read(&text);
        assert_eq!(read.list_share, LIST_SHARE, "{share}");
        assert_eq!(read.column_share, COLUMN_SHARE, "{share}");
    }
    let read = read(r#"{"list_share": 0.25, "column_share": 0.5}"#);
    assert_eq!((read.list_share, read.column_share), (0.25, 0.5));
}

#[test]
fn the_preferences_are_read_off_the_state_as_it_starts_and_as_it_is_changed() {
    let mut state = AppState::default();
    assert_eq!(Prefs::of(&state, Arrangement::default()), Prefs::default());
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    reduce(&mut state, Action::ToggleRadiological);
    reduce(&mut state, Action::SetLinked(false));
    reduce(&mut state, Action::SetOverlay(Overlay::Filled));
    assert_eq!(Prefs::of(&state, arranged()), changed());
    assert_eq!(changed().arrangement(), arranged());
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
    assert_eq!(Prefs::of(&state, arranged()), changed());
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

#[test]
fn the_panel_starts_open_on_the_view_tab_at_the_width_voxa_gives_it() {
    let start = Prefs::default();
    assert_eq!(
        start.panel_share, None,
        "no width of the person's choosing yet"
    );
    assert!(!start.panel_folded);
    assert_eq!(start.tab, "View");
    // A file from a Voxa that had no panel: all three are as Voxa starts.
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(folder.path().join(FILE), r#"{"list_share": 0.2}"#).expect("writes");
    let read = load(folder.path());
    assert_eq!((read.panel_share, read.panel_folded), (None, false));
    assert_eq!(read.tab, "View");
}

#[test]
fn choosing_a_tab_is_remembered() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let chosen = Prefs {
        tab: "Scene".to_owned(),
        ..Prefs::default()
    };
    save(folder.path(), &chosen).expect("saves");
    let read = load(folder.path());
    assert_eq!(read.tab, "Scene");
    assert_eq!(read.arrangement().tab, "Scene");
    // And the panel being folded, and the width it was dragged to.
    std::fs::write(
        folder.path().join(FILE),
        r#"{"tab": "Labels", "panel_folded": true, "panel_share": 0.33}"#,
    )
    .expect("writes");
    let read = load(folder.path());
    assert_eq!((read.tab.as_str(), read.panel_folded), ("Labels", true));
    assert_eq!(read.panel_share, Some(0.33));
}

#[test]
fn a_panel_width_that_makes_no_sense_is_not_taken() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for share in ["0.0", "-0.2", "1.0", "4.0"] {
        let text = format!(r#"{{"panel_share": {share}}}"#);
        std::fs::write(folder.path().join(FILE), text).expect("writes");
        assert_eq!(load(folder.path()).panel_share, None, "{share}");
    }
}

#[test]
fn patient_details_start_off_and_are_not_remembered() {
    let mut state = AppState::default();
    assert!(!state.show_patient, "off as Voxa starts");
    reduce(&mut state, Action::TogglePatient);
    assert!(state.show_patient);
    // Kept as the window is at the end of a run with the details shown.
    let kept = Prefs::of(&state, Arrangement::default());
    let written = serde_json::to_value(&kept).expect("serialises");
    let names = written.as_object().expect("a table of settings");
    assert!(
        names.keys().all(|name| !name.contains("patient")),
        "nothing about the patient is kept: {:?}",
        names.keys().collect::<Vec<_>>()
    );
    // And read back into a fresh window, they are off.
    let folder = tempfile::tempdir().expect("a temporary folder");
    save(folder.path(), &kept).expect("saves");
    let mut fresh = AppState::default();
    for action in load(folder.path()).actions(&fresh) {
        reduce(&mut fresh, action);
    }
    assert!(!fresh.show_patient);
}
