//! The View tab: brightness, orientation, the slices, and the switches.

use super::super::super::commands::Request;
use super::super::super::hand::Hand;
use super::*;
use crate::profile::CT_WINDOWS;
use crate::state::Layout;
use crate::testing::{COLOUR, KIND_SCAN, colour_files, id, indexed, showing_a_scan_of};
use volsa_core::WindowLevel;

/// Click all the way down the tab and gather everything that was asked for, once each, in
/// the order met: the changes of state, and what was asked of the window.
fn sweep(state: &AppState) -> (Vec<Action>, Vec<Request>) {
    let (mut asked, mut requests) = (Vec::new(), Vec::new());
    let hand = Hand::new(320.0, 700.0);
    hand.click_each(&mut |ui| {
        let mut actions = Vec::new();
        let more = show(ui, state, &mut actions);
        for action in actions {
            if !asked.contains(&action) {
                asked.push(action);
            }
        }
        for request in more {
            if !requests.contains(&request) {
                requests.push(request);
            }
        }
    });
    (asked, requests)
}

#[test]
fn the_tab_offers_each_thing_once_from_top_to_bottom() {
    let (actions, requests) = sweep(&showing_a_scan_of("MR", None));
    let preset = |preset| Action::ApplyPreset {
        viewport: 0,
        preset,
    };
    let expected = vec![
        preset(0),
        preset(1),
        preset(2),
        Action::ToggleRadiological,
        Action::CineToggle { viewport: 0 },
        Action::CineSet {
            viewport: 0,
            fps: Cine::DEFAULT_FPS,
            looping: false,
        },
        Action::Flip {
            viewport: 0,
            horizontal: true,
        },
        Action::Flip {
            viewport: 0,
            horizontal: false,
        },
        Action::Rotate { viewport: 0 },
        Action::Fit { viewport: 0 },
        Action::SetSmooth(true),
        Action::SetLinked(false),
        Action::TogglePatient,
    ];
    assert_eq!(actions, expected);
    assert_eq!(
        requests,
        [],
        "none of its buttons asks anything of the window"
    );
}

#[test]
fn the_switches_are_said_in_plain_words_and_the_orientation_in_what_it_means() {
    let hand = Hand::new(320.0, 700.0);
    let state = AppState::default();
    let seen = hand.settle(&mut |ui| {
        let _ = show(ui, &state, &mut Vec::new());
    });
    let wanted = [
        "Patient's left on the right",
        "Smooth the planes",
        "Link the views",
        "Show patient details",
    ];
    let said: Vec<&str> = seen
        .words
        .iter()
        .map(String::as_str)
        .filter(|word| wanted.contains(word))
        .collect();
    assert_eq!(said, wanted, "{:?}", seen.words);
}

#[test]
fn the_link_says_what_it_takes_for_granted() {
    let taken = "Linked views take it that the patient did not move between series.";
    assert!(LINK_SAYS.ends_with(taken), "{LINK_SAYS}");
    let mut state = AppState::default();
    state.planes.linked = false;
    let (actions, _) = sweep(&state);
    assert!(actions.contains(&Action::SetLinked(true)), "{actions:?}");
}

#[test]
fn the_smooth_switch_says_that_it_adds_no_detail() {
    let honest = "Smoothed: a curve through the measured values, not more detail.";
    assert!(SMOOTH_SAYS.ends_with(honest), "{SMOOTH_SAYS}");
    let mut state = AppState::default();
    state.planes.smooth = true;
    let (actions, _) = sweep(&state);
    assert!(actions.contains(&Action::SetSmooth(false)), "{actions:?}");
}

#[test]
fn the_brightness_the_slices_and_the_picture_are_those_of_the_active_viewport() {
    let state = showing_in_the_third_viewport("MR", None);
    let (actions, _) = sweep(&state);
    for preset in 0..3 {
        assert!(
            actions.contains(&Action::ApplyPreset {
                viewport: 2,
                preset
            }),
            "{actions:?}"
        );
    }
    assert!(actions.contains(&Action::CineToggle { viewport: 2 }));
    let flip = |horizontal| Action::Flip {
        viewport: 2,
        horizontal,
    };
    for picture in [
        flip(true),
        flip(false),
        Action::Rotate { viewport: 2 },
        Action::Fit { viewport: 2 },
    ] {
        assert!(actions.contains(&picture), "{actions:?}");
    }
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::SetWindow { .. })),
        "a click changes no number"
    );
}

#[test]
fn a_scan_of_several_volumes_steps_between_them_here() {
    use crate::testing::{FIRST_ECHO, id, indexed, two_echoes_files};
    let mut state = AppState::default();
    crate::reduce(
        &mut state,
        Action::IndexProgress(indexed(two_echoes_files())),
    );
    let series = id(FIRST_ECHO);
    crate::reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series,
        },
    );
    let (actions, _) = sweep(&state);
    assert!(
        actions.contains(&Action::StepVolume { viewport: 0, by: 1 }),
        "{actions:?}"
    );
}

/// Four viewports with the made-up scan of this kind shown in the third, which is the active
/// one.
fn showing_in_the_third_viewport(modality: &str, rescale_type: Option<&str>) -> AppState {
    let mut state = showing_a_scan_of(modality, rescale_type);
    state.layout = Layout::TwoByTwo;
    state.active = 2;
    let (viewport, series) = (2, id(KIND_SCAN));
    crate::reduce(&mut state, Action::SelectSeries { viewport, series });
    state
}

/// What the brightness section asks for, in the order met: the changes of the window, and the
/// presets.
fn brightness_asked(state: &AppState) -> Vec<Action> {
    let (actions, _) = sweep(state);
    let brightness = |action: &Action| {
        matches!(
            action,
            Action::SetWindow { .. } | Action::ApplyPreset { .. }
        )
    };
    actions.into_iter().filter(brightness).collect()
}

/// The window an action sets, as a name of the CT windows would.
fn sets(viewport: usize, window: usize) -> Action {
    let wl = CT_WINDOWS[window].wl;
    Action::SetWindow { viewport, wl }
}

/// Every piece of text the tab draws for this state.
fn words_of(state: &AppState) -> Vec<String> {
    let hand = Hand::new(320.0, 700.0);
    let seen = hand.settle(&mut |ui| {
        let _ = show(ui, state, &mut Vec::new());
    });
    seen.words
}

#[test]
fn a_ct_scan_offers_brain_soft_tissue_lung_bone_and_auto() {
    let state = showing_a_scan_of("CT", None);
    let preset = Action::ApplyPreset {
        viewport: 0,
        preset: 0,
    };
    let expected = vec![sets(0, 0), sets(0, 1), sets(0, 2), sets(0, 3), preset];
    assert_eq!(brightness_asked(&state), expected);
    let words = words_of(&state);
    for label in ["Brain", "Soft tissue", "Lung", "Bone", "Auto"] {
        assert!(words.iter().any(|word| word == label), "{label}: {words:?}");
    }
    for gone in ["Soft", "Hard"] {
        assert!(!words.iter().any(|word| word == gone), "{gone}: {words:?}");
    }
}

#[test]
fn pressing_brain_sets_width_80_level_40_on_the_active_viewport() {
    let mut state = showing_in_the_third_viewport("CT", None);
    let hand = Hand::new(320.0, 700.0);
    let mut actions = Vec::new();
    let found = hand.click_named("Brain window", &mut |ui| {
        let _ = show(ui, &state, &mut actions);
    });
    assert!(found, "the button is there");
    let brain = Action::SetWindow {
        viewport: 2,
        wl: WindowLevel::new(40.0, 80.0),
    };
    assert_eq!(actions.as_slice(), std::slice::from_ref(&brain));
    crate::reduce(&mut state, brain);
    let set = state.viewports[2].wl;
    assert_eq!((set.width, set.center), (80.0, 40.0));
    assert_ne!(
        state.viewports[0].wl, set,
        "no other viewport is changed by it"
    );
}

#[test]
fn an_mr_scan_offers_auto_soft_hard() {
    let state = showing_a_scan_of("MR", None);
    let preset = |preset| Action::ApplyPreset {
        viewport: 0,
        preset,
    };
    assert_eq!(
        brightness_asked(&state),
        [preset(0), preset(1), preset(2)],
        "as 0.6.0 does, and no CT window"
    );
    let words = words_of(&state);
    for label in ["Auto", "Soft", "Hard"] {
        assert!(words.iter().any(|word| word == label), "{label}: {words:?}");
    }
    assert!(!words.iter().any(|word| word == "Brain"), "{words:?}");
}

#[test]
fn a_basic_scan_offers_auto() {
    for modality in ["US", "CR", "PT", ""] {
        let state = showing_a_scan_of(modality, None);
        let auto = Action::ApplyPreset {
            viewport: 0,
            preset: 0,
        };
        assert_eq!(brightness_asked(&state), [auto], "{modality:?}");
        let words = words_of(&state);
        for gone in ["Soft", "Hard", "Brain", "Bone"] {
            assert!(!words.iter().any(|word| word == gone), "{modality:?}");
        }
    }
}

#[test]
fn with_no_scan_shown_only_auto_is_offered_and_it_is_greyed_with_its_reason() {
    // Offered, so that it can be found and says why it does nothing; a click does nothing.
    assert_eq!(brightness_asked(&AppState::default()), []);
    let words = words_of(&AppState::default());
    assert!(words.iter().any(|word| word == "Auto"), "{words:?}");
    assert!(
        !words.iter().any(|word| word == "Soft" || word == "Brain"),
        "{words:?}"
    );
}

#[test]
fn a_picture_in_colour_has_no_brightness_to_set_and_says_so() {
    let mut state = AppState::default();
    crate::reduce(&mut state, Action::IndexProgress(indexed(colour_files())));
    let series = id(COLOUR);
    crate::reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series,
        },
    );
    let words = words_of(&state);
    let said = "In its own colours: no brightness to set.";
    assert!(words.iter().any(|word| word == said), "{words:?}");
    assert_eq!(brightness_asked(&state), [], "not a button, not a number");
}

#[test]
fn the_fine_tune_fields_say_hu_when_the_values_are_in_hounsfield_units() {
    for (modality, rescale, hu) in [
        ("CT", None, true),
        ("CT", Some("HU"), true),
        ("CT", Some("US"), false),
        ("CT", Some("MGML"), false),
        ("MR", None, false),
        ("MR", Some("HU"), false),
        ("US", None, false),
    ] {
        let words = words_of(&showing_a_scan_of(modality, rescale));
        // The suffix of each of the two fields, the width and the level.
        let said = words.iter().filter(|word| *word == " HU").count();
        let wanted = if hu { 2 } else { 0 };
        assert_eq!(said, wanted, "{modality} {rescale:?}: {words:?}");
    }
}

#[test]
fn the_window_and_level_are_written_as_the_corner_of_the_picture_writes_them() {
    let mut state = showing_a_scan_of("MR", None);
    state.viewports[0].wl = WindowLevel::new(315.6, 590.97);
    let words = words_of(&state);
    // The same figures as `W 591  L 316` in the corner of the picture, not 590.97 and 315.60.
    for wanted in ["591", "316"] {
        assert!(
            words.iter().any(|word| word == wanted),
            "{wanted}: {words:?}"
        );
    }
    assert!(
        !words.iter().any(|word| word.contains("590.97")),
        "{words:?}"
    );
    let corner = crate::ui::overlay::window_number(state.viewports[0].wl.width);
    assert_eq!(format!("W {corner}"), "W 591");
}

#[test]
fn with_no_picture_there_are_no_numbers_to_set_and_the_tab_says_why() {
    // Nothing open: no W 1.0 and L 0.0 to type into, and the button that sets the brightness
    // says why it does nothing.
    let nothing = words_of(&AppState::default());
    let said = "Open a scan to see it here.";
    assert!(nothing.iter().any(|word| word == said), "{nothing:?}");
    assert!(
        !nothing.iter().any(|word| word.starts_with("W ")),
        "{nothing:?}"
    );
    // Under a document the pictures are out of sight and have no brightness to set.
    let mut covered = showing_a_scan_of("MR", None);
    covered.document = Some(crate::state::OpenDocument {
        entry: "DOCS/D0001".to_owned(),
        report: None,
    });
    let words = words_of(&covered);
    let said = "A document is open: close it to see the pictures.";
    assert!(words.iter().any(|word| word == said), "{words:?}");
    assert!(
        !words.iter().any(|word| word.starts_with("W ")),
        "{words:?}"
    );
    assert_eq!(brightness_asked(&covered), [], "and a click does nothing");
}
