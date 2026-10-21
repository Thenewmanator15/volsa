//! What the window does when its graphics card stops answering.

use std::cell::RefCell;
use std::rc::Rc;

use voxa_render::DeviceWatch;

use super::*;
use crate::state::NoticeKind;
use crate::testing::{a_device, lose};

/// What a card says in a look, as the actions it gives.
fn said(card: &mut Card) -> Vec<Action> {
    let mut said = Vec::new();
    card.look(|action| said.push(action));
    said
}

/// A way of showing words that shows none, and keeps the words it was asked to show, so that
/// no test ever opens a box.
fn shown_to_nobody() -> (Show, Rc<RefCell<Vec<String>>>) {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let keep = Rc::clone(&asked);
    let show: Show = Box::new(move |text| keep.borrow_mut().push(text.to_owned()));
    (show, asked)
}

#[test]
fn a_card_that_answers_says_nothing_and_asks_for_no_box() {
    let Some((device, _queue)) = a_device() else {
        return;
    };
    let (show, asked) = shown_to_nobody();
    let mut card = Card::new(Some(DeviceWatch::on(&device, || {})), show);
    assert_eq!(said(&mut card), []);
    assert_eq!(said(&mut card), []);
    assert_eq!(asked.borrow().len(), 0);
}

#[test]
fn a_window_with_no_card_has_nothing_to_say_of_one() {
    let (show, asked) = shown_to_nobody();
    assert_eq!(said(&mut Card::new(None, show)), []);
    assert_eq!(asked.borrow().len(), 0);
}

#[test]
fn a_card_that_stops_answering_is_said_to_have_once_in_the_words_given() {
    let Some((device, _queue)) = a_device() else {
        return;
    };
    let (show, _) = shown_to_nobody();
    let mut card = Card::new(Some(DeviceWatch::on(&device, || {})), show);
    assert_eq!(said(&mut card), []);
    lose(&device);
    let [Action::Post(notice)] = &said(&mut card)[..] else {
        panic!("one notice was to be posted");
    };
    assert_eq!(
        notice.text,
        "The graphics card stopped answering. Close Voxa and open it again. \
         Nothing you made has been lost."
    );
    assert_eq!(notice.kind, NoticeKind::Error);
    assert_eq!(notice.key, Some(KEY));
    assert_eq!(notice.details, Vec::<String>::new());
    assert_eq!(said(&mut card), [], "not said again");
}

#[test]
fn a_message_box_of_the_system_is_asked_for_once_with_the_same_words() {
    let Some((device, _queue)) = a_device() else {
        return;
    };
    let (show, asked) = shown_to_nobody();
    let mut card = Card::new(Some(DeviceWatch::on(&device, || {})), show);
    lose(&device);
    for _ in 0..3 {
        let _ = said(&mut card);
    }
    let words = [
        "The graphics card stopped answering. Close Voxa and open it again. \
         Nothing you made has been lost.",
    ];
    assert_eq!(*asked.borrow(), words);
}

#[test]
fn the_sentence_is_written_in_the_log_as_it_is_given() {
    let line = crate::log::line_for(&stopped());
    assert_eq!(line, format!("error [graphics]: {STOPPED}"));
}
