//! The sentences about the kind of scan, at the foot of the View tab.

use super::super::super::hand::Hand;
use super::*;
use crate::naming::ABOUT_HEADING;
use crate::testing::showing_a_scan_of;

/// The least wide a panel's contents are, in points: 260 less its margins and its bar.
const NARROW: f32 = 236.0;

/// What the tab writes for `state` on the narrowest panel, a piece of text a line, and how far
/// right its contents reach.
fn written(state: &AppState) -> (String, f32) {
    let right = std::cell::Cell::new(0.0);
    let seen = Hand::new(NARROW, 1200.0).settle(&mut |ui: &mut Ui| {
        let _ = show(ui, state, &mut Vec::new());
        right.set(ui.min_rect().right());
    });
    (seen.words.join("\n"), right.get())
}

#[test]
fn the_kind_of_the_scan_in_the_active_viewport_is_told_last_in_the_tab() {
    let (said, _) = written(&showing_a_scan_of("CT", None));
    let heading = said.find(ABOUT_HEADING).expect("the heading is written");
    let display = said.find("Display").expect("the section above it");
    assert!(heading > display, "{said}");
    let after = &said[heading..];
    assert!(after.contains("A CT scan"), "{after}");
    assert!(after.contains("not what yours shows"), "{after}");
}

#[test]
fn nothing_is_told_with_no_scan_shown() {
    let (said, _) = written(&AppState::default());
    assert!(!said.contains(ABOUT_HEADING), "{said}");
}

#[test]
fn the_sentences_wrap_within_the_narrowest_panel() {
    let (said, right) = written(&showing_a_scan_of("CT", None));
    assert!(said.contains(ABOUT_HEADING), "{said}");
    assert!(right <= NARROW + 0.5, "{right} points across");
}
