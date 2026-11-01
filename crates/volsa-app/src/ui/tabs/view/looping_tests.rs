//! The switch that makes the slices start again after the last: an action of the list, drawn
//! as the other switches of the tab are, and named as it is.

use super::super::super::commands;
use super::super::super::hand::Hand;
use super::*;
use crate::testing::showing_a_scan_of;
use crate::ui::commands::{Does, Place};
use crate::ui::panel::Tab;

#[test]
fn starting_again_after_the_last_slice_is_an_action_of_the_list_that_switches_it() {
    let looped = commands::find("slice.loop").expect("listed");
    assert_eq!(looped.place, Place::Tab(Tab::View));
    let state = showing_a_scan_of("MR", None);
    let Does::Act(act) = looped.does else {
        panic!("it changes the state");
    };
    // The slices start again unless they were told not to; the switch turns that over, and
    // leaves the speed as it was.
    let off = Action::CineSet {
        viewport: 0,
        fps: Cine::DEFAULT_FPS,
        looping: false,
    };
    assert_eq!(act(&state), Some(off));
    let mut stopped = state;
    let _ = crate::reduce(
        &mut stopped,
        Action::CineSet {
            viewport: 0,
            fps: 9.0,
            looping: false,
        },
    );
    let on = Action::CineSet {
        viewport: 0,
        fps: 9.0,
        looping: true,
    };
    assert_eq!(act(&stopped), Some(on));
}

#[test]
fn the_switch_is_drawn_in_the_words_of_its_name() {
    let state = showing_a_scan_of("MR", None);
    let name = commands::find("slice.loop").expect("listed").name;
    let hand = Hand::new(320.0, 700.0);
    let seen = hand.settle(&mut |ui| {
        let _ = show(ui, &state, &mut Vec::new());
    });
    assert!(
        seen.words.iter().any(|word| word == name),
        "{:?}",
        seen.words
    );
}
