//! The Labels tab: who made the labels and the model's own check come before any part, then
//! the overview by group, then the list; and clicking down the tab finds everything it offers.

use std::cell::Cell;
use std::path::PathBuf;

use egui::{Rect, Ui};
use voxa_labels::{Made, Side, Trust, Verdict, verdict_in_words};

use super::*;
use crate::reducer::reduce;
use crate::testing::{labelled_head, thick_scan_ready};
use crate::ui::hand::{Hand, Seen};
use crate::ui::panel::{self, PanelState, Tab};
use crate::ui::tabs::Kept;
use crate::ui::tabs::made_up::{
    WIDE, labelled_by, labelled_with, labels_tab, labels_written, named, part, parts, screen,
    sweep_labels,
};

/// The tab drawn twice, for its panels to find their sizes, with nothing asked.
fn seen(state: &AppState) -> Seen {
    let mut draw = labels_tab();
    screen().settle(&mut |ui| {
        let _ = draw(ui, state, &mut Vec::new());
    })
}

/// Where the piece of text, or the widget a screen reader calls, `name` is.
fn bounds_of(seen: &Seen, name: &str) -> Rect {
    let bounds = seen.text_bounds(name).or_else(|| seen.bounds_of(name));
    bounds.unwrap_or_else(|| panic!("{name:?} is drawn: {:?}", seen.words))
}

/// How far down the screen the piece of text, or the widget, `name` begins.
fn top_of(seen: &Seen, name: &str) -> f32 {
    bounds_of(seen, name).top()
}

/// Labels that Voxa made, whose model's own check failed, and what the check says.
fn made_and_failed() -> (AppState, String) {
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("general csf".to_owned(), 0.6)],
    };
    let said = verdict_in_words(&verdict);
    let made = Made {
        by: "Voxa 9.8.7".to_owned(),
        model: "Made-up model, robust".to_owned(),
        model_sum: String::new(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        notes: Vec::new(),
    };
    let state = labelled_by(Some(verdict), NamesFrom::Model, Some(made));
    (state, said)
}

#[test]
fn the_labels_tab_says_who_made_them_before_any_part() {
    let (state, _) = labelled_head();
    let seen = seen(&state);
    let made = top_of(&seen, MADE_ELSEWHERE);
    assert!(made < top_of(&seen, "half"), "{:?}", seen.words);
}

#[test]
fn a_failed_check_comes_first_then_who_made_them_then_the_overview_and_then_the_parts() {
    let (state, check) = made_and_failed();
    let seen = seen(&state);
    let made = "Labels made by Voxa with Made-up model, robust, on Made-up card A, 2099-01-02. \
                Not for diagnostic use.";
    let down = [
        top_of(&seen, &check),
        top_of(&seen, made),
        top_of(&seen, "Go to Deep brain"),
        top_of(&seen, "hippocampus"),
    ];
    assert!(down.windows(2).all(|pair| pair[0] < pair[1]), "{down:?}");
}

#[test]
fn the_overview_has_one_row_per_group_with_its_count() {
    let seen = seen(&named());
    let groups = [
        ("Deep brain", "2 parts"),
        ("Cerebellum and brainstem", "1 part"),
        ("Outside the head's brain", "1 part"),
    ];
    let go = |(name, _): &&(String, Rect)| name.starts_with("Go to ");
    assert_eq!(seen.names.iter().filter(go).count(), groups.len());
    let mut last = f32::MIN;
    for (group, count) in groups {
        let row = bounds_of(&seen, &format!("Go to {group}"));
        let counted = seen.words.iter().zip(&seen.placed);
        let mut beside = counted.filter(|(word, at)| {
            word.as_str() == count && (at.center().y - row.center().y).abs() < row.height()
        });
        assert!(beside.next().is_some(), "{group}: {count}");
        assert!(row.top() > last, "in the table's order");
        last = row.top();
    }
}

/// Click on the widget called `name` in the tab and give what it asked for.
fn click(state: &AppState, name: &str) -> Vec<Action> {
    let (hand, mut draw, mut actions) = (screen(), labels_tab(), Vec::new());
    let found = hand.click_named(name, &mut |ui| {
        let _ = draw(ui, state, &mut actions);
    });
    assert!(found, "{name:?} is drawn");
    actions
}

#[test]
fn hiding_a_group_from_the_overview_sends_group_show() {
    let mut state = named();
    let group = "Deep brain".to_owned();
    let hide = Action::GroupShow {
        group: group.clone(),
        shown: false,
    };
    assert_eq!(click(&state, "Hide Deep brain"), [hide]);
    // With a part of the group hidden the switch shows it again.
    let hidden = Action::PartShow {
        part: 1,
        shown: false,
    };
    reduce(&mut state, hidden);
    let show = Action::GroupShow { group, shown: true };
    assert_eq!(click(&state, "Show Deep brain"), [show]);
}

/// The last group of a long list, which is far down it.
const LAST: &str = "Outside the head's brain";

/// Labels of a long list: forty parts in a group of their own, and then the made-up parts, so
/// that the last group, which holds the eyeball, is far down.
fn a_long_list() -> AppState {
    let long = (0..40).map(|n| {
        let name = format!("long {n}");
        part(&name, "A long group", Side::Midline, Trust::Model)
    });
    let everything = [long.collect(), parts()].concat();
    labelled_with(everything, None, NamesFrom::Table, None)
}

#[test]
fn clicking_a_group_in_the_overview_scrolls_the_list_to_it() {
    let state = a_long_list();
    let (hand, mut view) = (Hand::new(WIDE, 400.0), PartsView::default());
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, &state, &mut view, &mut Vec::new());
    };
    let header = |seen: &Seen| seen.bounds_of(LAST).expect("the group's header");
    let before = hand.settle(&mut draw);
    assert!(header(&before).top() > 400.0, "out of sight");
    let search = bounds_of(&before, "Search parts");
    assert!(hand.click_named(&format!("Go to {LAST}"), &mut draw));
    let after = hand.settle(&mut draw);
    let header = header(&after);
    assert!(
        header.top() >= search.bottom() && header.bottom() <= 400.0,
        "in sight, below the search box: {header:?}"
    );
}

#[test]
fn the_search_and_the_part_chosen_stay_in_view_while_the_list_scrolls() {
    let mut state = a_long_list();
    // The eyeball, in the last group, far down the list.
    reduce(&mut state, Action::PartSelect(Some(40 + 3)));
    let hand = Hand::new(900.0, 600.0);
    let (mut panel, mut kept) = (
        PanelState::remembered(None, false, "Labels"),
        Kept::default(),
    );
    let rect = Cell::new(Rect::NOTHING);
    let mut draw = |ui: &mut Ui| {
        let contents =
            |ui: &mut Ui, tab| crate::ui::tabs::show(ui, tab, &state, &mut kept, &mut Vec::new());
        rect.set(panel::show(ui, &mut panel, &state, contents).rect);
    };
    let fixed = ["Search parts", "How solid it is drawn", "Show only this"];
    let place = |seen: &Seen| fixed.map(|name| bounds_of(seen, name));
    let before = hand.settle(&mut draw);
    let (panel_rect, was) = (rect.get(), place(&before));
    assert!(
        was.iter().all(|at| panel_rect.contains_rect(*at)),
        "{was:?} in {panel_rect:?}"
    );
    let overview = bounds_of(&before, "Go to Deep brain");
    let far = before.bounds_of(LAST).expect("the group's header");
    assert!(far.top() > panel_rect.bottom(), "out of sight: {far:?}");
    // Take the list down to the group the part is in.
    assert!(hand.click_named(&format!("Go to {LAST}"), &mut draw));
    let after = hand.settle(&mut draw);
    let far = after.bounds_of(LAST).expect("the group's header");
    assert!(
        panel_rect.contains_rect(far),
        "the list scrolled: {far:?} in {panel_rect:?}"
    );
    let moved = bounds_of(&after, "Go to Deep brain");
    assert!(moved.top() < overview.top(), "the overview went up with it");
    let now = place(&after);
    assert_eq!(
        now, was,
        "the search box and the part's controls did not move"
    );
    assert!(
        now.iter().all(|at| panel_rect.contains_rect(*at)),
        "{now:?} in {panel_rect:?}"
    );
}

#[test]
fn a_group_the_search_hides_is_reached_from_the_overview_by_clearing_the_search() {
    let state = named();
    let typed = || PartsView {
        search: "hip".to_owned(),
        ..PartsView::default()
    };
    let (hand, mut view) = (screen(), typed());
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, &state, &mut view, &mut Vec::new());
    };
    assert!(hand.click_named("Go to Cerebellum and brainstem", &mut draw));
    assert_eq!(view.search, "", "the group was not in the list");
    let (hand, mut view) = (screen(), typed());
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, &state, &mut view, &mut Vec::new());
    };
    assert!(hand.click_named("Go to Deep brain", &mut draw));
    assert_eq!(view.search, "hip", "the group was in the list already");
}

#[test]
fn clicking_down_the_tab_finds_everything_it_offers() {
    let state = named();
    let (actions, requests) = sweep_labels(&state);
    for part in 0..4 {
        let hide = Action::PartShow { part, shown: false };
        assert!(
            actions.contains(&hide),
            "a switch for part {part}: {actions:?}"
        );
    }
    for part in [0, 2, 3] {
        assert!(
            actions.contains(&Action::PartSelect(Some(part))),
            "a name selects part {part}"
        );
    }
    for group in [
        "Deep brain",
        "Cerebellum and brainstem",
        "Outside the head's brain",
    ] {
        let hide = Action::GroupShow {
            group: group.to_owned(),
            shown: false,
        };
        assert!(actions.contains(&hide), "{group}");
    }
    assert!(actions.contains(&Action::PartsShowAll) && actions.contains(&Action::CloseLabels));
    assert!(actions.contains(&Action::SetOverlay(Overlay::Off)));
    assert!(actions.contains(&Action::SetOverlay(Overlay::Filled)));
    assert!(
        !actions.contains(&Action::SetOverlay(Overlay::Outline)),
        "the one in use"
    );
    let files = [Asked::OpenNames, Asked::SaveCsv].map(Request::Choose);
    assert_eq!(requests.len(), files.len(), "{requests:?}");
    assert!(files.iter().all(|file| requests.contains(file)));
}

#[test]
fn the_part_selected_can_be_faded_shown_alone_and_let_go() {
    let mut state = named();
    reduce(&mut state, Action::PartSelect(Some(1)));
    let (actions, _) = sweep_labels(&state);
    assert!(actions.contains(&Action::PartIsolate(1)), "{actions:?}");
    // The slider is dragged, not clicked: its name is how it is found.
    let (hand, mut draw, mut dragged) = (screen(), labels_tab(), Vec::new());
    let found = hand.click_named("How solid it is drawn", &mut |ui| {
        let _ = draw(ui, &state, &mut dragged);
    });
    let faded = |action: &Action| matches!(action, Action::PartOpacity { part: 1, .. });
    assert!(found && dragged.iter().any(faded), "{dragged:?}");
    assert!(
        actions.contains(&Action::PartSelect(None)),
        "its own name lets it go"
    );
    let said = labels_written(&state);
    assert!(said.contains("hippocampus, right: 4.0 ml"), "{said}");
    let against = "Left 4.0 ml, right 4.0 ml: the same size.";
    assert!(said.contains(against), "its two sides are compared: {said}");
    reduce(&mut state, Action::PartSelect(Some(2)));
    let said = labels_written(&state);
    assert!(
        !said.contains("Left "),
        "a part with one side is not: {said}"
    );
}

#[test]
fn the_tab_says_what_the_labels_are_where_their_names_came_from_and_the_models_check() {
    let said = labels_written(&named());
    for sentence in ["A made-up head", MADE_ELSEWHERE, "Names from the table"] {
        assert!(said.contains(sentence), "{sentence}: {said}");
    }
    assert!(
        said.contains("approximate"),
        "how far a name can be trusted: {said}"
    );
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("general csf".to_owned(), 0.6)],
    };
    let said = labels_written(&labelled_by(Some(verdict), NamesFrom::Nothing, None));
    assert!(
        said.contains("The labelling model's own check failed"),
        "{said}"
    );
    assert!(said.contains("called by their numbers"), "{said}");
}

#[test]
fn with_no_labels_the_tab_says_what_they_are_and_offers_to_open_some_or_to_label_the_scan() {
    let state = thick_scan_ready();
    let (actions, requests) = sweep_labels(&state);
    let tools = Request::ShowTab(Tab::Tools);
    assert_eq!(
        (actions, requests),
        (vec![], vec![Request::OpenLabels, tools])
    );
    let said = labels_written(&state);
    assert!(said.contains("by Voxa, when it labels the scan, or by another program"));
    assert!(said.contains("Label this scan in the Tools tab."), "{said}");
    // With no volume there is nothing to put labels on; the Tools tab says why.
    let (actions, requests) = sweep_labels(&AppState::default());
    assert_eq!((actions, requests), (vec![], vec![tools]));
}

#[test]
fn while_labels_are_read_and_when_they_fail_the_tab_says_so() {
    let mut state = thick_scan_ready();
    let open = Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    reduce(&mut state, open);
    assert!(labels_written(&state).contains("Reading the labels"));
    reduce(
        &mut state,
        Action::LabelsFailed("This is not a NIfTI file.".to_owned()),
    );
    assert!(labels_written(&state).contains("This is not a NIfTI file."));
    let (_, requests) = sweep_labels(&state);
    assert!(
        requests.contains(&Request::OpenLabels),
        "and another can be tried"
    );
}

#[test]
fn labels_of_another_study_are_said_to_be_and_can_be_closed() {
    let mut state = named();
    state.labels.study = Some(voxa_core::StudyId("another".to_owned()));
    assert!(labels_written(&state).contains("another study"));
    let (actions, requests) = sweep_labels(&state);
    assert_eq!((actions, requests), (vec![Action::CloseLabels], vec![]));
}
