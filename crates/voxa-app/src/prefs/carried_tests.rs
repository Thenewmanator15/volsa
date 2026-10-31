//! Settings carried from an older Voxa and from one run to the next: a file as 0.6.0 wrote it, the
//! columns that were folded, and what is not kept at all.

use voxa_render::Overlay;

use super::*;
use crate::action::Action;
use crate::pictures::Pictures;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};

/// A settings file as Voxa 0.6.0 wrote it, with every setting at a value of the person's choosing.
const FILE_OF_0_6_0: &str = r#"{
  "window": [1200.0, 800.0],
  "interface_size": 1.25,
  "list_share": 0.2,
  "column_share": 0.35,
  "viewports": 4,
  "radiological": false,
  "linked": false,
  "overlay": "filled"
}"#;

#[test]
fn a_0_6_0_settings_file_opens_with_its_settings() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(folder.path().join(FILE), FILE_OF_0_6_0).expect("writes");
    let read = load(folder.path());
    // The window, the size of the interface and the share of the list are kept as they were.
    let window = read.arrangement();
    assert_eq!(window.window, Some([1200.0, 800.0]));
    assert_eq!(window.interface_size, 1.25);
    assert_eq!(window.list_share, 0.2);
    // The viewports, the convention, the link and the overlay are put back by asking the state
    // for what differs.
    let mut state = AppState::default();
    assert!(state.radiological && state.planes.linked, "0.6.0's start");
    for action in read.actions(&state) {
        reduce(&mut state, action);
    }
    assert_eq!(state.layout, Layout::TwoByTwo);
    assert!(!state.radiological && !state.planes.linked);
    assert_eq!(state.labels.overlay, Overlay::Filled);
    // What did not exist in 0.6.0 is as Voxa starts, and the column's width is the panel's.
    let share = window
        .panel_share
        .expect("the column's width is carried over");
    assert!((share - 0.35 * 0.8).abs() < 1e-6, "{share}");
    assert!(!window.panel_folded && !window.rail_folded);
    assert_eq!(
        (window.tab.as_str(), window.theme.as_str()),
        ("View", "system")
    );
    // Written again, the file is Voxa's own, and the column is no longer in it.
    save(folder.path(), &Prefs::of(&state, window)).expect("saves");
    let text = std::fs::read_to_string(folder.path().join(FILE)).expect("reads");
    assert!(!text.contains("column_share"), "{text}");
    assert_eq!(load(folder.path()).arrangement().list_share, 0.2);
}

#[test]
fn which_columns_are_folded_is_remembered_and_a_file_that_says_nothing_has_them_open() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let both = Prefs {
        panel_folded: true,
        rail_folded: true,
        ..Prefs::default()
    };
    save(folder.path(), &both).expect("saves");
    let read = load(folder.path());
    assert!(read.panel_folded && read.rail_folded);
    assert!(read.arrangement().rail_folded);
    let open = Prefs::default();
    assert!(!open.panel_folded && !open.rail_folded);
    std::fs::write(folder.path().join(FILE), r#"{"viewports": 2}"#).expect("writes");
    assert!(
        !load(folder.path()).rail_folded,
        "a file without it has it open"
    );
}

#[test]
fn no_arrangement_of_the_pictures_is_remembered() {
    // Voxa opens on the slices in every run: the arrangement is not a setting.
    let mut state = AppState::default();
    for pictures in Pictures::ALL {
        reduce(&mut state, Action::SetPictures(pictures));
        let kept = Prefs::of(&state, Arrangement::default());
        let written = serde_json::to_value(&kept).expect("serialises");
        let names = written.as_object().expect("a table of settings");
        assert!(
            names
                .keys()
                .all(|name| !name.contains("picture") && !name.contains("arrangement")),
            "{pictures:?}: {:?}",
            names.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            kept,
            Prefs::default(),
            "{pictures:?}: nothing of it is kept"
        );
    }
    // And a fresh run, given what was kept, shows the slices.
    let mut fresh = AppState::default();
    for action in Prefs::of(&state, Arrangement::default()).actions(&fresh) {
        reduce(&mut fresh, action);
    }
    assert_eq!(fresh.pictures, Pictures::Slices);
}
