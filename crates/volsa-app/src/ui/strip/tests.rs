//! The strip above the pictures: the four arrangements, the viewports while the slices are
//! shown, and where the slice shown is.

use egui::{Rect, Ui, pos2, vec2};

use super::super::hand::{Hand, Seen};
use super::*;
use crate::reducer::reduce;
use crate::state::Layout;
use crate::testing::thick_scan_ready;
use crate::testing::{COLOUR, FIRST_ECHO, colour_files, id, indexed, two_echoes_files};

/// How wide the strip is drawn.
const WIDE: f32 = 900.0;

/// What a screen reader calls the button of the action `id`, which writes its label on itself.
fn spoken(id: &str) -> String {
    let command = commands::find(id).expect("listed");
    commands::spoken(command, command.label).into_owned()
}

/// The names of the four arrangements' buttons, as a screen reader says them.
fn arrangements() -> [String; 4] {
    ARRANGEMENTS.map(|(_, id)| spoken(id))
}

/// The names of the viewports' buttons.
fn viewports() -> [String; 3] {
    ["layout.one", "layout.two", "layout.four"].map(spoken)
}

/// Draw the strip along the top of a window for `state`, keeping what it asked for.
fn strip<'a>(state: &'a AppState, actions: &'a mut Vec<Action>) -> impl FnMut(&mut Ui) + 'a {
    move |ui| {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDE, super::super::layout::STRIP));
        let asked = show(ui, rect, state, actions);
        assert!(asked.is_empty(), "the strip asks nothing of the window");
    }
}

/// What the strip shows for `state`.
fn seen(state: &AppState) -> Seen {
    Hand::new(WIDE, 300.0).settle(&mut strip(state, &mut Vec::new()))
}

/// Click each of the strip's buttons named `names` in turn, and gather what was asked for.
fn clicked(state: &AppState, names: &[String]) -> Vec<Action> {
    let hand = Hand::new(WIDE, 300.0);
    let mut actions = Vec::new();
    for name in names {
        assert!(
            hand.click_named(name, &mut strip(state, &mut actions)),
            "{name} is in the strip"
        );
    }
    actions
}

#[test]
fn the_strip_offers_the_four_arrangements_and_the_viewports_while_the_slices_are_shown() {
    let state = thick_scan_ready();
    assert_eq!(state.pictures, Pictures::Slices);
    let seen = seen(&state);
    let [slices, both, planes, scene] = arrangements();
    let [one, two, four] = viewports();
    let left = |name: &String| seen.bounds_of(name).expect(name).left();
    let left_to_right = [&slices, &one, &two, &four, &both, &planes, &scene].map(left);
    assert!(left_to_right.is_sorted(), "{left_to_right:?}");
    let asked = clicked(&state, &[both, planes, scene, two, four]);
    assert_eq!(
        asked,
        [
            Action::SetPictures(Pictures::SlicesAndScene),
            Action::SetPictures(Pictures::Planes),
            Action::SetPictures(Pictures::Scene),
            Action::SetLayout(Layout::TwoAcross),
            Action::SetLayout(Layout::TwoByTwo),
        ]
    );
    assert_eq!(
        clicked(&state, &[slices, one]),
        [],
        "what is in use is not asked for again"
    );
}

#[test]
fn the_viewports_are_offered_only_while_the_slices_are_shown() {
    let mut state = thick_scan_ready();
    for (pictures, offered) in [
        (Pictures::Slices, true),
        (Pictures::SlicesAndScene, true),
        (Pictures::Planes, false),
        (Pictures::Scene, false),
    ] {
        reduce(&mut state, Action::SetPictures(pictures));
        let names = seen(&state).named().join(", ");
        for name in viewports() {
            assert_eq!(names.contains(&name), offered, "{pictures:?}: {names}");
        }
        for name in arrangements() {
            assert!(names.contains(&name), "{pictures:?}: {names}");
        }
    }
}

#[test]
fn an_arrangement_the_scan_cannot_have_is_greyed_and_says_why() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(colour_files())));
    let (viewport, series) = (0, id(COLOUR));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    reduce(&mut state, Action::SetPictures(Pictures::Scene));
    let [slices, both, planes, scene] = arrangements();
    assert_eq!(
        clicked(&state, &[both, planes, scene]),
        [],
        "greyed: a click does nothing"
    );
    for id in ["pictures.slices3d", "pictures.planes", "pictures.scene"] {
        let why = (commands::find(id).expect("listed").why_not)(&state);
        let why = why.expect("it says why");
        assert!(
            why.starts_with("This scan has no 3D view: ") && why.ends_with('.'),
            "{why}"
        );
        assert!(why.contains("colour"), "and the scan's own reason: {why}");
    }
    let why = (commands::find("pictures.slices").expect("listed").why_not)(&state);
    assert_eq!(why, None, "the slices are always there");
    assert!(seen(&state).named().contains(&slices.as_str()));
}

#[test]
fn the_strip_says_which_slice_and_which_volume() {
    let state = thick_scan_ready();
    let slice = state.viewports[0].slice;
    let said = format!("slice {} of 30", slice + 1);
    assert!(
        seen(&state).words.contains(&said),
        "{:?}",
        seen(&state).words
    );
    let mut state = AppState::default();
    reduce(
        &mut state,
        Action::IndexProgress(indexed(two_echoes_files())),
    );
    let (viewport, series) = (0, id(FIRST_ECHO));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    let words = seen(&state).words.join(" | ");
    assert!(words.contains("volume 1 of 2"), "{words}");
    assert!(
        seen(&AppState::default())
            .words
            .iter()
            .all(|word| !word.starts_with("slice")),
        "nothing shown, nothing said"
    );
}

#[test]
fn the_labelled_parts_are_a_tab_of_the_panel_and_not_part_of_the_strip() {
    let mut state = thick_scan_ready();
    for pictures in Pictures::ALL {
        reduce(&mut state, Action::SetPictures(pictures));
        let words = seen(&state).words;
        assert!(
            !words.iter().any(|word| word == "Parts"),
            "{pictures:?}: {words:?}"
        );
    }
}

#[test]
fn with_four_viewports_the_others_are_offered_and_four_is_not_asked_for_again() {
    let mut state = thick_scan_ready();
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    reduce(&mut state, Action::SetActive(2));
    let [one, two, four] = viewports();
    assert_eq!(
        clicked(&state, &[one, two, four]),
        [
            Action::SetLayout(Layout::One),
            Action::SetLayout(Layout::TwoAcross),
        ]
    );
}

#[test]
fn the_slice_is_said_only_while_slices_are_in_sight() {
    let mut state = thick_scan_ready();
    for (pictures, in_sight) in [
        (Pictures::Slices, true),
        (Pictures::SlicesAndScene, true),
        (Pictures::Planes, false),
        (Pictures::Scene, false),
    ] {
        reduce(&mut state, Action::SetPictures(pictures));
        let words = seen(&state).words;
        let said = words.iter().any(|word| word.starts_with("slice "));
        assert_eq!(said, in_sight, "{pictures:?}: {words:?}");
    }
}

/// Draw the strip `wide` points across for `state`, keeping what it asked for.
fn strip_across<'a>(
    wide: f32,
    state: &'a AppState,
    actions: &'a mut Vec<Action>,
) -> impl FnMut(&mut Ui) + 'a {
    move |ui| {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(wide, super::super::layout::STRIP));
        let _ = show(ui, rect, state, actions);
    }
}

/// How wide the strip has to be to hold every button of the arrangements and the viewports:
/// to the right edge of the last of them.
fn full_width(state: &AppState) -> f32 {
    let seen = Hand::new(WIDE, 300.0).settle(&mut strip(state, &mut Vec::new()));
    let [.., last] = arrangements();
    seen.bounds_of(&last).expect("drawn").right()
}

#[test]
fn in_a_narrow_strip_each_arrangement_and_viewport_count_can_still_be_chosen() {
    let state = thick_scan_ready();
    let full = full_width(&state);
    assert!(full < WIDE - 40.0, "the buttons take {full} of {WIDE}");
    let every_name: Vec<String> = [arrangements().to_vec(), viewports().to_vec()].concat();
    // Wide enough, they are all in the strip itself.
    let hand = Hand::new(full + 40.0, 300.0);
    let seen = hand.settle(&mut strip_across(full + 40.0, &state, &mut Vec::new()));
    for name in &every_name {
        assert!(seen.bounds_of(name).is_some(), "{name} is in the strip");
    }
    // Whatever the width around the one they need, none is cut off at the strip's edge.
    for step in 0..40_i16 {
        let wide = full - 20.0 + f32::from(step);
        let hand = Hand::new(wide, 300.0);
        let seen = hand.settle(&mut strip_across(wide, &state, &mut Vec::new()));
        for (name, at) in &seen.names {
            assert!(
                at.right() <= wide + 0.5,
                "{name:?} at {at:?} is cut off at {wide}"
            );
        }
    }
    // Narrower, from just short of that to the narrowest middle of the smallest window at twice
    // the interface's size, nothing is cut off at the strip's edge, and each is one click or
    // two away.
    for wide in [full - 20.0, 0.7 * full, 0.5 * full, 150.0] {
        let hand = Hand::new(wide, 300.0);
        let seen = hand.settle(&mut strip_across(wide, &state, &mut Vec::new()));
        for (name, at) in &seen.names {
            assert!(
                at.right() <= wide + 0.5,
                "{name:?} at {at:?} is cut off by the strip's edge at {wide}"
            );
        }
        let wanted = [
            Action::SetPictures(Pictures::SlicesAndScene),
            Action::SetPictures(Pictures::Planes),
            Action::SetPictures(Pictures::Scene),
            Action::SetLayout(Layout::TwoAcross),
            Action::SetLayout(Layout::TwoByTwo),
        ];
        let names = [
            &every_name[1],
            &every_name[2],
            &every_name[3],
            &every_name[5],
            &every_name[6],
        ];
        for (name, action) in names.into_iter().zip(wanted) {
            let mut actions = Vec::new();
            {
                let mut draw = strip_across(wide, &state, &mut actions);
                if !hand.click_named(name, &mut draw) {
                    let opened = hand.click_named(&menu_name(state.pictures), &mut draw);
                    assert!(
                        opened,
                        "{name} is not in the strip, nor is the menu at {wide}"
                    );
                    let in_it = hand.click_named(name, &mut draw);
                    assert!(in_it, "{name} is in the menu at {wide}");
                }
            }
            assert_eq!(actions, [action], "{name} at {wide}");
        }
    }
}
