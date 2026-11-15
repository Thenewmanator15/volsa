//! The folds of the tabs: what each tab keeps under More, and the Tools tab's plugins, are out
//! of sight while the fold is closed, which it is to begin with, and in sight once it is
//! opened with a click on its heading.

use egui::Ui;
use volsa_labels::Made;

use super::made_up::{able, labelled_by};
use super::{Fold, Kept, show};
use crate::labels::NamesFrom;
use crate::plugins::PluginEntry;
use crate::state::AppState;
use crate::testing::showing_a_scan_of;
use crate::ui::commands::{self, Tab, spoken};
use crate::ui::hand::{Hand, Seen};

/// What a screen reader is told of the button of the action known by `id`, which shows its
/// label.
fn button_of(id: &str) -> String {
    let command = commands::find(id).expect("listed");
    spoken(command, command.label).into_owned()
}

/// What a screen reader is told of the switch of the action known by `id`, which shows its
/// name.
fn switch_of(id: &str) -> String {
    commands::find(id).expect("listed").name.to_owned()
}

/// A window tall enough for the whole of any tab.
fn hand() -> Hand {
    Hand::new(360.0, 2400.0)
}

/// What `tab` draws for `state` once it has settled.
fn seen(hand: &Hand, tab: Tab, state: &AppState) -> Seen {
    let mut kept = Kept::default();
    hand.settle(&mut |ui: &mut Ui| {
        let _ = show(ui, tab, state, &mut kept, &mut Vec::new());
    })
}

/// Click on the heading a screen reader calls `fold` in `tab`, and let it finish opening.
fn open(hand: &Hand, tab: Tab, state: &AppState, fold: &str) {
    let mut kept = Kept::default();
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, tab, state, &mut kept, &mut Vec::new());
    };
    assert!(
        hand.click_named(fold, &mut draw),
        "{fold} is drawn in {tab:?}"
    );
    for _ in 0..30 {
        let _ = hand.pass(Vec::new(), &mut draw);
    }
}

/// Each of `names` is not drawn in `tab` while its More is closed, and is once it is opened.
fn folded(tab: Tab, state: &AppState, names: &[String]) {
    let hand = hand();
    let closed = seen(&hand, tab, state);
    assert!(closed.bounds_of("More").is_some(), "{tab:?} has a More");
    for name in names {
        let drawn = closed.bounds_of(name);
        assert!(
            drawn.is_none(),
            "{name} is in sight in {tab:?} with More closed"
        );
    }
    open(&hand, tab, state, "More");
    assert!(Fold::More(tab).is_open(&hand.ctx, false));
    let opened = seen(&hand, tab, state);
    for name in names {
        let drawn = opened.bounds_of(name);
        assert!(
            drawn.is_some(),
            "{name} is not drawn in {tab:?} with More open"
        );
    }
}

#[test]
fn the_window_numbers_and_the_switches_of_the_planes_are_under_the_view_tabs_more() {
    let names = [
        "Window width".to_owned(),
        "Window level".to_owned(),
        switch_of("planes.smooth"),
        switch_of("planes.link"),
    ];
    folded(Tab::View, &showing_a_scan_of("MR", None), &names);
}

#[test]
fn what_stays_in_sight_in_the_view_tab_is_drawn_with_more_closed() {
    let state = showing_a_scan_of("MR", None);
    let closed = seen(&hand(), Tab::View, &state);
    let names = [
        "Brightness",
        "Contrast",
        "Playback speed",
        &switch_of("slice.loop"),
        &switch_of("view.patient"),
    ];
    for name in names {
        assert!(closed.bounds_of(name).is_some(), "{name} is in sight");
    }
}

#[test]
fn the_angle_and_region_tools_and_saving_as_json_are_under_the_measure_tabs_more() {
    let names = [
        button_of("measure.angle"),
        button_of("measure.region"),
        button_of("measure.save_json"),
    ];
    folded(Tab::Measure, &showing_a_scan_of("MR", None), &names);
    let closed = seen(&hand(), Tab::Measure, &showing_a_scan_of("MR", None));
    for id in [
        "measure.move",
        "measure.ruler",
        "measure.marker",
        "measure.undo",
        "measure.redo",
        "measure.save_csv",
        "measure.clear",
    ] {
        let name = button_of(id);
        assert!(closed.bounds_of(&name).is_some(), "{name} is in sight");
    }
}

#[test]
fn saving_the_labels_their_names_and_how_they_are_drawn_are_under_the_labels_tabs_more() {
    let made = Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "Made-up model".to_owned(),
        model_sum: String::new(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        added: None,
        notes: Vec::new(),
    };
    let state = labelled_by(None, NamesFrom::Model, Some(made));
    let mut names: Vec<String> = ["labels.save", "labels.names"]
        .into_iter()
        .map(button_of)
        .collect();
    for way in ["off", "outline", "filled"] {
        names.push(button_of(&format!("labels.overlay.{way}")));
    }
    folded(Tab::Labels, &state, &names);
    let closed = seen(&hand(), Tab::Labels, &state);
    for id in ["labels.save_csv", "labels.show_all", "labels.close"] {
        let name = button_of(id);
        assert!(closed.bounds_of(&name).is_some(), "{name} is in sight");
    }
}

#[test]
fn the_overview_of_the_groups_has_no_show_or_hide_of_its_own() {
    let state = labelled_by(None, NamesFrom::Table, None);
    let seen = seen(&hand(), Tab::Labels, &state);
    let named = seen.named();
    for group in [
        "Deep brain",
        "Cerebellum and brainstem",
        "Outside the head's brain",
    ] {
        for doubled in [format!("Show {group}"), format!("Hide {group}")] {
            assert!(!named.contains(&doubled.as_str()), "{doubled}: {named:?}");
        }
        let go = format!("Go to {group}");
        assert!(named.contains(&go.as_str()), "{go}: {named:?}");
        // The group's own box in the list, which shows or hides it.
        assert!(named.contains(&group), "{group}: {named:?}");
    }
}

#[test]
fn the_plugins_added_are_folded_away_until_one_is_added() {
    let state = able();
    let hand = hand();
    let closed = seen(&hand, Tab::Tools, &state);
    assert!(closed.bounds_of("Added by you").is_some());
    assert!(closed.bounds_of("Add a plugin…").is_none(), "folded away");
    open(&hand, Tab::Tools, &state, "Added by you");
    let opened = seen(&hand, Tab::Tools, &state);
    assert!(
        opened.bounds_of("Add a plugin…").is_some(),
        "drawn once opened"
    );

    let mut added = able();
    let why = "This plugin's files have changed since it was added.";
    added.plugins.entries = vec![PluginEntry::broken("made-up-folder", why.to_owned())];
    let fresh = seen(&Hand::new(360.0, 2400.0), Tab::Tools, &added);
    assert!(
        fresh.bounds_of("Add a plugin…").is_some(),
        "open to begin with once a plugin has been added"
    );
}

#[test]
fn an_action_found_in_a_fold_opens_it_and_says_where_it_is() {
    let hand = hand();
    assert!(!Fold::More(Tab::Measure).is_open(&hand.ctx, false));
    super::open_fold_for(&hand.ctx, "measure.region");
    assert!(Fold::More(Tab::Measure).is_open(&hand.ctx, false));
    let state = showing_a_scan_of("MR", None);
    let opened = seen(&hand, Tab::Measure, &state);
    assert!(opened.bounds_of(&button_of("measure.region")).is_some());
    let said = super::said_of("labels.overlay.filled", "In the Labels tab");
    assert_eq!(said, "In the Labels tab, under More");
    assert_eq!(
        super::said_of("plugins.add", "In the Tools tab"),
        "In the Tools tab, under Added by you"
    );
}
