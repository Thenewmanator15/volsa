//! The entries agree with what they are built from: the brightness presets, the sizes of the
//! interface, the most cuts there can be, and what the bars draw.

use voxa_render::MAX_CUTS;
use voxa_volume::PRESETS;

use super::super::size::{SIZES, percent};
use super::*;

/// The size an entry asks for, if it asks for one.
fn size_of(command: &Command) -> Option<f32> {
    match command.does {
        Does::Ask(Request::Size(size)) => Some(size),
        _ => None,
    }
}

#[test]
fn each_brightness_preset_is_an_entry_that_says_what_it_does() {
    let ids = ["window.auto", "window.soft", "window.hard"];
    let state = crate::testing::thick_scan_ready();
    for ((preset, named), id) in PRESETS.iter().enumerate().zip(ids) {
        let command = find(id).expect("listed");
        assert_eq!(command.label, named.name);
        let says = format!(
            "From the {} to the {} percentile of the slice shown",
            named.low_pct, named.high_pct
        );
        assert_eq!(command.explains, says);
        let Does::Act(act) = command.does else {
            panic!("{id} changes the state");
        };
        let applied = Action::ApplyPreset {
            viewport: 0,
            preset,
        };
        assert_eq!(act(&state), Some(applied));
    }
}

#[test]
fn each_size_the_menu_offers_is_an_entry_labelled_as_it_is_said() {
    let sizes: Vec<&Command> = all()
        .iter()
        .filter(|command| size_of(command).is_some())
        .collect();
    assert_eq!(sizes.len(), SIZES.len());
    for (command, size) in sizes.iter().zip(SIZES) {
        assert_eq!(size_of(command).map(percent), Some(percent(size)));
        assert_eq!(command.label, percent(size));
        assert_eq!(command.place, Place::Menu);
    }
}

#[test]
fn each_cut_there_can_be_has_an_entry_that_removes_it() {
    let state = AppState::default();
    for cut in 0..MAX_CUTS {
        let id = format!("scene.uncut.{}", cut + 1);
        let command = find(&id).expect("listed");
        assert!((command.why_not)(&state).is_some(), "there is no cut yet");
        let Does::Act(act) = command.does else {
            panic!("{id} changes the state");
        };
        assert_eq!(act(&state), Some(Action::CutRemove { cut }));
    }
    assert!(find(&format!("scene.uncut.{}", MAX_CUTS + 1)).is_none());
}

#[test]
fn an_action_that_cannot_be_used_says_why_in_a_sentence() {
    let nothing = AppState::default();
    let why = |id| (find(id).expect("listed").why_not)(&nothing);
    assert_eq!(why("measure.ruler"), Some(super::super::measure::NO_VOLUME));
    assert_eq!(
        why("scene.save.picture"),
        Some("There is no 3D view to save yet")
    );
    assert!(why("volume.before").is_some_and(|said| said.contains("several volumes")));
    assert_eq!(why("layout.two"), None);
}
