//! The View tab: brightness, orientation, the slices, and the switches.

use super::super::super::commands::Request;
use super::super::super::hand::Hand;
use super::*;
use crate::state::Layout;

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
    let (actions, requests) = sweep(&AppState::default());
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
fn the_brightness_and_the_slices_are_those_of_the_active_viewport() {
    let state = AppState {
        layout: Layout::TwoByTwo,
        active: 2,
        ..AppState::default()
    };
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
