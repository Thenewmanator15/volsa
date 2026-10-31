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
    let no_volume = super::super::tabs::NO_VOLUME;
    assert_eq!(why("measure.ruler").as_deref(), Some(no_volume));
    assert_eq!(
        why("scene.save.picture").as_deref(),
        Some("There is no 3D view to save yet.")
    );
    assert!(why("volume.before").is_some_and(|said| said.contains("several volumes")));
    assert_eq!(why("layout.two"), None);
}

/// States in which actions give their reasons: nothing open; a scan shown; its 3D view cut
/// as many times as it can be; the labelled parts drawn as solids; the first and the last
/// volume of a scan that holds several; and a picture in colour, which has no 3D view.
fn states_that_give_reasons() -> Vec<AppState> {
    use crate::reducer::reduce;
    use crate::testing::{
        COLOUR, FIRST_ECHO, SECOND_ECHO, colour_files, id, indexed, labelled_head,
        thick_scan_ready, three_series_files, two_echoes_files,
    };
    let mut cut = thick_scan_ready();
    for _ in 0..MAX_CUTS {
        reduce(&mut cut, Action::CutAdd);
    }
    let (mut solid, _) = labelled_head();
    reduce(&mut solid, Action::SceneLook(voxa_render::Look::Parts));
    let on = |files: Vec<(String, Vec<u8>)>, shown: &str| {
        let mut state = AppState::default();
        reduce(&mut state, Action::IndexProgress(indexed(files)));
        let (viewport, series) = (0, id(shown));
        reduce(&mut state, Action::SelectSeries { viewport, series });
        state
    };
    let echoes = || [three_series_files(), two_echoes_files()].concat();
    vec![
        AppState::default(),
        thick_scan_ready(),
        cut,
        solid,
        on(echoes(), FIRST_ECHO),
        on(echoes(), SECOND_ECHO),
        on(colour_files(), COLOUR),
    ]
}

#[test]
fn every_reason_an_action_cannot_be_used_ends_with_a_full_stop() {
    // Search posts a reason as a notice, which is a sentence.
    let mut said = std::collections::HashSet::new();
    for state in states_that_give_reasons() {
        for command in all() {
            if let Some(why) = (command.why_not)(&state) {
                assert!(why.ends_with('.'), "{}: {why}", command.id);
                said.insert(why);
            }
        }
    }
    // The states reach the reasons that were without a stop.
    for reached in [
        "There is nothing to take back.",
        "There is nothing to do again.",
        "Three cuts at most.",
        "There is no cut 1.",
        "There is no cut 3.",
        "This is the first volume.",
        "This is the last volume.",
        "The scan shown is not one of several volumes.",
        "There is no 3D view to save yet.",
        "The slice is a picture of the scan: it is not drawn among the labelled parts.",
        "Open labels in the Labels tab to see them as solids here.",
        "There is no tool in hand to put down.",
    ] {
        assert!(said.contains(reached), "no state gives: {reached}");
    }
    // And a reason that says what the scan itself says.
    let no_view = said
        .iter()
        .find(|why| why.starts_with("This scan has no 3D view: "));
    assert!(no_view.is_some(), "{said:?}");
}
