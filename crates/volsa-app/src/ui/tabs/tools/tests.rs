//! The Tools tab: labelling a brain scan under "Part of Volsa", and under "Added by you" the
//! tools the person added, of which there are none yet.

use super::*;
use crate::labelling::LabellingAction;
use crate::tools::{ADDED_BY_YOU, MadeBy};
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

/// A tool someone added, with everything said of it.
fn added() -> ToolInfo {
    ToolInfo {
        name: "Made-up tool".to_owned(),
        made_by: MadeBy::AddedByYou,
        source: "made-up.example/tool".to_owned(),
        licence: "Made-up licence".to_owned(),
        checked: Some(true),
        accepts: "a made-up scan".to_owned(),
        returns: "made-up labels".to_owned(),
    }
}

/// What the tab writes with these tools added.
fn written_with(added: &[ToolInfo]) -> String {
    let state = able();
    let draw = &mut |ui: &mut Ui| {
        let _ = show(ui, &state, added, &mut Vec::new());
    };
    Hand::new(NARROW, 900.0).settle(draw).words.join("\n")
}

#[test]
fn the_tools_tab_lists_labelling_under_part_of_volsa() {
    let state = able();
    let said = tools_written(&state);
    let order = [
        "Part of Volsa",
        "Label this brain scan",
        "Label this scan",
        "Added by you",
    ];
    let places: Vec<usize> = order.iter().map(|line| at(&said, line)).collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{said}");
    let (actions, requests) = sweep_tools(&state);
    assert_eq!(
        requests,
        [Request::LabelScan],
        "the button asks the window to label the scan"
    );
    let itself =
        |action: &Action| matches!(action, Action::Labelling(LabellingAction::LabelScan { .. }));
    assert!(!actions.iter().any(itself), "and does not label it itself");
}

#[test]
fn added_by_you_is_empty_and_says_so() {
    let said = written_with(&[]);
    assert!(said.contains(NONE_ADDED), "{said}");
    assert_eq!(
        NONE_ADDED,
        "Tools you add will be listed here. Adding tools comes in a later version of Volsa."
    );
    assert!(
        !said.contains(ADDED_BY_YOU),
        "no tool is said to be added: {said}"
    );
}

#[test]
fn a_tool_the_person_added_is_listed_with_everything_said_of_it() {
    let said = written_with(&[added()]);
    assert!(!said.contains(NONE_ADDED), "{said}");
    let lines = [
        "Made-up tool",
        ADDED_BY_YOU,
        "Came from: made-up.example/tool",
        "Licence: Made-up licence",
        "Its files passed their check.",
        "Accepts: a made-up scan",
        "Returns: made-up labels",
    ];
    let places: Vec<usize> = lines.iter().map(|line| at(&said, line)).collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{said}");
    assert!(at(&said, "Added by you") < places[0], "under its heading");
}

#[test]
fn a_tool_says_whether_its_files_passed_their_check_and_who_made_it() {
    let says = |checked, made_by| {
        let tool = ToolInfo {
            made_by,
            checked,
            ..added()
        };
        written_with(&[tool])
    };
    assert!(says(Some(false), MadeBy::AddedByYou).contains("Its files did not pass their check."));
    assert!(says(None, MadeBy::AddedByYou).contains("Its files have not been checked."));
    let by_volsa = says(Some(true), MadeBy::Volsa);
    assert!(by_volsa.contains("Made by Volsa") && !by_volsa.contains(ADDED_BY_YOU));
}

#[test]
fn a_tool_with_long_words_does_not_widen_the_narrowest_panel() {
    let long = "a-very-long-made-up-address-that-goes-on-and-on/and-on-and-on/and-on";
    let tool = ToolInfo {
        source: long.to_owned(),
        accepts: long.replace('-', " "),
        ..added()
    };
    let state = able();
    let right = std::cell::Cell::new(0.0);
    let draw = &mut |ui: &mut Ui| {
        let _ = show(ui, &state, std::slice::from_ref(&tool), &mut Vec::new());
        right.set(ui.min_rect().right());
    };
    let _ = Hand::new(NARROW, 900.0).settle(draw);
    assert!(right.get() <= NARROW + 0.5, "{} points across", right.get());
}
