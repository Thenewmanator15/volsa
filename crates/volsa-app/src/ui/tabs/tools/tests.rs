//! The Tools tab: labelling a brain scan under "Part of Volsa", and under "Added by you" the
//! plugins the person added, with the button that adds one.

use super::*;
use crate::labelling::LabellingAction;
use crate::plugins::PluginEntry;
use crate::ui::hand::Hand;
use crate::ui::tabs::made_up::{able, sweep_tools, tools_written};

/// The least wide a panel's contents are, in points: 260 less its margins and its bar.
const NARROW: f32 = 236.0;

/// Where the first piece of text that is `line` comes among those written.
fn at(words: &str, line: &str) -> usize {
    let lines: Vec<&str> = words.lines().collect();
    let found = lines.iter().position(|one| *one == line);
    found.unwrap_or_else(|| panic!("{line:?} is written: {words}"))
}

/// A computer that can label, with one plugin added whose files failed their check.
fn with_a_plugin(folder: &str, why: &str) -> AppState {
    let mut state = able();
    state.plugins.entries = vec![PluginEntry::broken(folder, why.to_owned())];
    state
}

/// Click on the button a screen reader calls `name`, with the tab drawn tall enough to hold all
/// of it, and gather what was asked for.
fn pressed(state: &AppState, name: &str) -> (Vec<Action>, Vec<Request>) {
    let (mut actions, mut requests) = (Vec::new(), Vec::new());
    let found = Hand::new(NARROW, 900.0).click_named(name, &mut |ui| {
        requests.extend(show(ui, state, &mut actions));
    });
    assert!(found, "there is a button called {name:?}");
    (actions, requests)
}

#[test]
fn the_tools_tab_lists_labelling_under_part_of_volsa_and_then_what_was_added() {
    let state = able();
    let said = tools_written(&state);
    let order = [
        "Part of Volsa",
        "Label this brain scan",
        "Label this scan",
        "Added by you",
        "Add a plugin…",
    ];
    let places: Vec<usize> = order.iter().map(|line| at(&said, line)).collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{said}");
    let (actions, requests) = sweep_tools(&state);
    assert!(
        requests.contains(&Request::LabelScan),
        "the button asks the window to label the scan: {requests:?}"
    );
    let itself =
        |action: &Action| matches!(action, Action::Labelling(LabellingAction::LabelScan { .. }));
    assert!(!actions.iter().any(itself), "and does not label it itself");
}

#[test]
fn adding_a_plugin_is_asked_of_the_window_which_has_the_folder_chosen() {
    let state = able();
    let (actions, requests) = pressed(&state, "Add a plugin…");
    assert!(requests.contains(&Request::AddPlugin), "{requests:?}");
    let about_plugins = |action: &Action| matches!(action, Action::Plugins(_));
    assert!(
        !actions.iter().any(about_plugins),
        "nothing is added until a folder is chosen and its card is read: {actions:?}"
    );
}

#[test]
fn a_plugin_the_person_added_is_listed_under_added_by_you_and_can_be_removed() {
    let why = "This plugin's files have changed since it was added. Remove it and add it again.";
    let state = with_a_plugin("made-up-folder", why);
    let said = tools_written(&state);
    assert!(said.contains("made-up-folder"), "{said}");
    assert!(said.contains(why), "why it cannot be run: {said}");
    assert!(
        at(&said, "Added by you") > at(&said, "Label this brain scan"),
        "after what is part of Volsa: {said}"
    );
    let (actions, _) = pressed(&state, "Remove");
    let remove = Action::Plugins(crate::plugins::PluginAction::Remove(
        "made-up-folder".to_owned(),
    ));
    assert!(actions.contains(&remove), "{actions:?}");
    let run = |action: &Action| {
        matches!(
            action,
            Action::Plugins(crate::plugins::PluginAction::Run { .. })
        )
    };
    assert!(
        !actions.iter().any(run),
        "one whose files failed their check is not run"
    );
}

#[test]
fn a_plugin_with_long_words_does_not_widen_the_narrowest_panel() {
    let long = "a-very-long-made-up-folder-name-that-goes-on-and-on-and-on-and-on-and-on";
    let state = with_a_plugin(long, &long.replace('-', " "));
    let right = std::cell::Cell::new(0.0);
    let draw = &mut |ui: &mut Ui| {
        let _ = show(ui, &state, &mut Vec::new());
        right.set(ui.min_rect().right());
    };
    let _ = Hand::new(NARROW, 900.0).settle(draw);
    assert!(right.get() <= NARROW + 0.5, "{} points across", right.get());
}

#[test]
fn highlight_veins_comes_after_labelling_and_its_button_highlights_them() {
    let state = able();
    let said = tools_written(&state);
    assert!(
        at(&said, "Label this scan") < at(&said, "Highlight veins"),
        "{said}"
    );
    assert!(said.contains("not a trained model"), "{said}");
    let (actions, _) = pressed(&state, "Highlight veins");
    let finds = |action: &Action| matches!(action, Action::Labelling(LabellingAction::FindVeins));
    assert!(actions.iter().any(finds), "{actions:?}");
}

#[test]
fn once_found_the_veins_are_a_switch_that_shows_and_hides_them() {
    let (mut state, _) = crate::testing::labelled_head();
    state.labels.parts[1].name = volsa_segment::VEINS_PART.to_owned();
    let said = tools_written(&state);
    assert!(said.contains("Show veins"), "{said}");
    let (actions, _) = sweep_tools(&state);
    assert!(
        actions.contains(&Action::PartShow {
            part: 1,
            shown: false
        }),
        "the switch hides them: {actions:?}"
    );
    let finds = |action: &Action| matches!(action, Action::Labelling(LabellingAction::FindVeins));
    assert!(!actions.iter().any(finds), "and does not look again");
}
