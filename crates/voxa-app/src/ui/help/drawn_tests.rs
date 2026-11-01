//! The topics name each button, box and tab as the window draws it. A word that is only a
//! screen reader's name, or the name of a thing in the code, is no use to a person who sees
//! the window: so the words are checked against the text the window draws, not against the
//! names it gives its controls.

use egui::Ui;

use super::TOPICS;
use crate::action::Action;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::testing::{labelled_head, showing_a_scan_of, thick_scan_ready, with_odd_series};
use crate::ui::browser::Thumbnails;
use crate::ui::commands::Tab;
use crate::ui::hand::Hand;
use crate::ui::search::Search;
use crate::ui::theme::ThemeChoice;
use crate::ui::{rail, tabs, top};

/// What each topic says of the window, by name: every one is drawn, word for word.
const DRAWN: [(&str, &[&str]); 6] = [
    ("open", &["Open", "Folder…", "Zip file or DICOMDIR…"]),
    ("find", &["Other files"]),
    (
        "brightness",
        &[
            "View tab",
            "Brain",
            "Soft tissue",
            "Lung",
            "Bone",
            "Auto",
            "Soft",
            "Hard",
        ],
    ),
    (
        "measure",
        &["Measure tab", "Ruler", "Angle", "Region", "Marker"],
    ),
    (
        "sides",
        &[
            "Patient's left on the right",
            "View tab",
            "3D tab",
            "A front",
            "P back",
            "L left",
            "R right",
            "S top",
            "I bottom",
        ],
    ),
    (
        "labels",
        &["Label this scan", "Tools tab", "Open labels…", "Labels tab"],
    ),
];

/// What the topics say a box or a sentence begins with: drawn, but with a number or more words
/// after it. The pairs are what the topic says, and what is drawn.
const BEGINNING: [(&str, &str, &str); 3] = [
    ("brightness", "the box marked W", "W "),
    ("brightness", "the box marked L", "L "),
    (
        "labels",
        "\"Made by another program\"",
        "Made by another program",
    ),
];

/// Every piece of text `tab` draws in `state`.
fn tab_words(state: &AppState, tab: Tab) -> Vec<String> {
    let hand = Hand::new(360.0, 2400.0);
    let seen = hand.settle(&mut |ui: &mut Ui| {
        let _ = tabs::show(ui, tab, state, &mut tabs::Kept::default(), &mut Vec::new());
    });
    seen.words
}

/// Every piece of text the top bar draws, and then the menu it opens with a click on Open.
fn top_words(state: &AppState) -> Vec<String> {
    let hand = Hand::new(900.0, 400.0);
    let mut draw = |ui: &mut Ui| {
        let mut search = Search::default();
        let _ = top::show(ui, state, &mut search, ThemeChoice::System, &mut Vec::new());
    };
    assert!(
        hand.click_named("Open", &mut draw),
        "the Open menu is there"
    );
    hand.settle(&mut draw).words
}

/// Every piece of text the rail draws for a state that holds other files.
fn rail_words(state: &AppState) -> Vec<String> {
    let hand = Hand::new(300.0, 900.0);
    let thumbnails = Thumbnails::default();
    let mut shown = rail::RailState::default();
    let seen = hand.settle(&mut |ui: &mut Ui| {
        let _ = rail::column(
            ui,
            240.0,
            true,
            state,
            &thumbnails,
            &mut shown,
            &mut Vec::new(),
        );
    });
    seen.words
}

/// Everything the window draws that a topic may name, in the states where it is drawn.
fn drawn() -> Vec<String> {
    let (full, mr, ct) = (
        thick_scan_ready(),
        showing_a_scan_of("MR", None),
        showing_a_scan_of("CT", None),
    );
    // An export with locating images among its scans has other files for the rail to group.
    let mut odd = AppState::default();
    reduce(&mut odd, Action::IndexProgress(with_odd_series()));
    let mut words = top_words(&full);
    words.extend(rail_words(&odd));
    for tab in Tab::ALL {
        // The tab's button, which the panel draws.
        words.push(format!("{} tab", tab.title()));
        for state in [&full, &mr, &ct, &labelled_head().0] {
            words.extend(tab_words(state, tab));
        }
    }
    words
}

#[test]
fn the_topics_name_each_control_as_the_window_draws_it() {
    let drawn = drawn();
    for (id, names) in DRAWN {
        let body = TOPICS
            .iter()
            .find(|topic| topic.id == id)
            .expect("listed")
            .body;
        for name in names {
            assert!(body.contains(name), "{id} does not say {name:?}: {body}");
            let there = drawn.iter().any(|word| word == name);
            assert!(there, "{name:?}, said by {id}, is not drawn by the window");
        }
    }
    for (id, said, begins) in BEGINNING {
        let body = TOPICS
            .iter()
            .find(|topic| topic.id == id)
            .expect("listed")
            .body;
        assert!(body.contains(said), "{id} does not say {said:?}: {body}");
        let there = drawn.iter().any(|word| word.starts_with(begins));
        assert!(there, "{said:?}, said by {id}, is not drawn by the window");
    }
}

#[test]
fn the_width_and_level_are_the_boxes_marked_w_and_l_and_not_the_names_a_screen_reader_hears() {
    // The boxes read "W 400 HU" and "L 40 HU": "Window width" and "Window level" are only what a
    // screen reader is told, so no topic sends a person to look for them.
    let words = tab_words(&showing_a_scan_of("CT", None), Tab::View);
    for mark in ["W ", "L "] {
        assert!(
            words.iter().any(|word| word.starts_with(mark)),
            "{mark:?}: {words:?}"
        );
    }
    for name in ["Window width", "Window level"] {
        assert!(!words.iter().any(|word| word == name), "{name:?} is drawn");
        for topic in &TOPICS {
            assert!(!topic.body.contains(name), "{} says {name:?}", topic.id);
        }
    }
}
