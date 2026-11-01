//! The 3D tab: what it offers, and that what a button asks of the window is passed on.

use volsa_render::{Look, MAX_CUTS};

use super::super::super::commands::Request;
use super::super::super::hand::Hand;
use super::super::super::scene::export;
use super::*;
use crate::pictures::Pictures;

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

fn views() -> Vec<Action> {
    VIEWS
        .iter()
        .map(|(view, _, _)| Action::SceneView(*view))
        .collect()
}

#[test]
fn the_tab_offers_each_thing_once_from_top_to_bottom() {
    let looks = vec![Action::SceneLook(Look::Vr), Action::SceneLook(Look::Mip)];
    let rest = vec![Action::CutAdd, Action::ShowSliceInScene(true)];
    let (asked, requests) = sweep(&AppState::default());
    assert_eq!(asked, [looks, views(), rest].concat());
    assert_eq!(requests, [], "the Export menu is closed");
}

#[test]
fn the_look_in_use_is_not_asked_for_again_and_the_slice_can_be_put_away() {
    let mut state = AppState::default();
    state.scene.look = Look::Mip;
    state.scene.slice_shown = true;
    let (asked, _) = sweep(&state);
    let looks = [
        Action::SceneLook(Look::Surface),
        Action::SceneLook(Look::Vr),
    ];
    assert_eq!(asked[..2], looks);
    assert_eq!(asked.last(), Some(&Action::ShowSliceInScene(false)));
}

#[test]
fn the_parts_look_is_offered_when_there_are_labels_on_this_series_and_only_then() {
    let parts = Action::SceneLook(Look::Parts);
    let (unlabelled, _) = sweep(&AppState::default());
    assert!(!unlabelled.contains(&parts), "nothing to draw as solids");
    let (mut state, _) = crate::testing::labelled_head();
    let scans = [Look::Surface, Look::Vr, Look::Mip].map(Action::SceneLook);
    let (asked, _) = sweep(&state);
    let offered = [&scans[1..], std::slice::from_ref(&parts)].concat();
    assert_eq!(asked[..3], offered, "after the looks of the scan");
    assert!(asked.contains(&Action::ShowSliceInScene(true)));
    crate::reducer::reduce(&mut state, parts.clone());
    let (asked, _) = sweep(&state);
    assert_eq!(asked[..3], scans, "the look in use is not asked for again");
    assert!(!asked.contains(&parts));
    let slice = Action::ShowSliceInScene(true);
    assert!(!asked.contains(&slice), "no slice among the solids");
}

#[test]
fn each_cut_has_a_button_that_removes_it_and_three_cuts_leave_no_room_for_more() {
    let mut state = AppState::default();
    let cut = volsa_render::CutPlane::facing(&state.scene.camera);
    state.scene.cuts = vec![cut; 2];
    let (asked, _) = sweep(&state);
    let removed = [Action::CutRemove { cut: 0 }, Action::CutRemove { cut: 1 }];
    assert!(
        removed.iter().all(|remove| asked.contains(remove)),
        "{asked:?}"
    );
    assert!(asked.contains(&Action::CutAdd), "there is room for a third");
    state.scene.cuts = vec![cut; MAX_CUTS];
    let (asked, _) = sweep(&state);
    assert!(
        !asked.contains(&Action::CutAdd),
        "but not for a fourth: {asked:?}"
    );
    assert!(asked.contains(&Action::CutRemove { cut: 2 }));
}

#[test]
fn the_buttons_carry_the_names_and_letters_radiology_uses_and_a_word_for_each_side() {
    assert_eq!(LOOKS.map(|(_, label, _)| label), ["Surface", "VR", "MIP"]);
    assert_eq!(
        VIEWS.map(|(_, label, _)| label),
        [
            "A front", "P back", "L left", "R right", "S top", "I bottom"
        ]
    );
    assert!(
        LOOKS.iter().all(|(_, _, hint)| hint.len() > 20),
        "each is explained"
    );
    assert!(VIEWS.iter().all(|(_, _, hint)| hint.starts_with("From ")));
}

#[test]
fn saving_the_view_is_asked_of_the_window_and_not_dropped() {
    let state = crate::testing::thick_scan_ready();
    let hand = Hand::new(320.0, 700.0);
    let mut asked = Vec::new();
    let mut draw = |ui: &mut egui::Ui| {
        let mut actions = Vec::new();
        let more = show(ui, &state, &mut actions);
        if !more.is_empty() {
            asked = more;
        }
    };
    assert!(
        hand.click_named("Export", &mut draw),
        "the Export menu has a button"
    );
    let command = crate::ui::commands::find("scene.save.picture").expect("listed");
    let picture = crate::ui::commands::spoken(command, command.label);
    assert!(
        hand.click_named(&picture, &mut draw),
        "and it holds {picture}"
    );
    assert_eq!(asked, [Request::SaveScene(export::Asked::Picture)]);
}

#[test]
fn the_tab_says_so_at_its_top_when_the_arrangement_shows_no_3d_view() {
    let words_of = |state: &AppState| {
        let hand = Hand::new(320.0, 700.0);
        hand.settle(&mut |ui| {
            let _ = show(ui, state, &mut Vec::new());
        })
        .words
    };
    let mut state = crate::testing::thick_scan_ready();
    for (pictures, hinted) in [
        (Pictures::Slices, true),
        (Pictures::SlicesAndScene, false),
        (Pictures::Planes, false),
        (Pictures::Scene, false),
    ] {
        crate::reducer::reduce(&mut state, Action::SetPictures(pictures));
        let words = words_of(&state);
        let at = words.iter().position(|word| word == NO_3D_VIEW);
        assert_eq!(at.is_some(), hinted, "{pictures:?}: {words:?}");
        // At the top: before the first heading of the controls.
        assert!(at.is_none_or(|at| at == 0), "{pictures:?}: {words:?}");
    }
    assert_eq!(NO_3D_VIEW, "Choose 3D, Planes or Slices + 3D to see this.");
}

#[test]
fn a_scan_with_no_3d_view_says_so_at_the_top_of_the_tab_in_the_scans_own_words() {
    let hand = Hand::new(320.0, 700.0);
    let mut state = AppState::default();
    state.scene.status = crate::scene::SceneStatus::Unavailable(vec!["It is a picture".to_owned()]);
    let words = hand
        .settle(&mut |ui| {
            let _ = show(ui, &state, &mut Vec::new());
        })
        .words;
    let said = "This scan has no 3D view: it is a picture.";
    assert_eq!(words.first().map(String::as_str), Some(said), "{words:?}");
    // Not the sentence that tells a person to choose an arrangement it does not have.
    assert!(!words.iter().any(|word| word == NO_3D_VIEW), "{words:?}");
}
