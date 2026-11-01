//! The help window's topics: listed above the keys, each closed until it is asked for, and the
//! one asked for opened and brought into view.

use egui::{Event, Rect, Ui};

use super::*;
use crate::ui::hand::{Hand, Seen};
use crate::ui::scrolled::Scrolled;

/// A window of the smallest size, where the topics and the keys do not fit together.
fn smallest() -> Hand {
    Hand::new(640.0, 480.0)
}

/// What is asked of the help before a pass is drawn.
#[derive(Clone, Copy)]
enum Asked {
    /// The help, with no topic: the `?` key.
    Bare,
    /// The help, at this topic: a result chosen in search.
    At(&'static str),
}

/// The help shown in a window of its own, a pass at a time.
struct Window {
    /// The window the help is drawn in.
    hand: Hand,
    /// Whether the help is open.
    open: bool,
    /// What is to be asked of the help before the next pass, if anything.
    asked: Option<Asked>,
    /// Where the part of the list that can be seen was in the last pass.
    shown: Rect,
}

impl Window {
    /// The help, shut, in `hand`'s window.
    fn new(hand: Hand) -> Self {
        Self {
            hand,
            open: false,
            asked: None,
            shown: Rect::NOTHING,
        }
    }

    /// Ask for the help, at a topic or not, before the next pass.
    fn ask(&mut self, asked: Asked) {
        self.asked = Some(asked);
    }

    /// One pass, with these events, and what it drew.
    fn pass(&mut self, events: Vec<Event>) -> Seen {
        let Self {
            hand,
            open,
            asked,
            shown,
        } = self;
        hand.pass(events, &mut |ui: &mut Ui| {
            // Pictures are of things at rest, not part-way through opening.
            ui.ctx().all_styles_mut(|style| style.animation_time = 0.0);
            match asked.take() {
                Some(Asked::Bare) => ask(ui.ctx(), open, None),
                Some(Asked::At(topic)) => ask(ui.ctx(), open, Some(topic)),
                None => {}
            }
            if let Some(list) = show(ui.ctx(), open) {
                *shown = Scrolled::of(&list).shown;
            }
        })
    }

    /// Passes enough for the window to have its size, its place and its scroll, and what the
    /// last drew.
    fn settle(&mut self) -> Seen {
        let mut seen = Seen::default();
        for _ in 0..5 {
            seen = self.pass(Vec::new());
        }
        seen
    }
}

#[test]
fn the_topics_are_listed_above_the_keys_each_closed() {
    let mut window = Window::new(Hand::new(900.0, 700.0));
    window.ask(Asked::Bare);
    let seen = window.settle();
    let keys = seen
        .text_bounds(KEYS_HEADING)
        .expect("the keys have a heading");
    for topic in &TOPICS {
        let title = seen.text_bounds(topic.title);
        let title = title.unwrap_or_else(|| panic!("{} is not listed", topic.id));
        assert!(
            title.bottom() <= keys.top(),
            "{} is under the keys",
            topic.id
        );
        assert!(
            !seen.words.iter().any(|word| word == topic.body),
            "{} is open",
            topic.id
        );
    }
    assert!(seen.text_bounds(TOPICS_HEADING).is_some());
}

#[test]
fn a_topic_asked_for_opens_and_is_in_sight_even_in_the_smallest_window() {
    // The last topic is the furthest down: with the others closed above it, it is in sight only
    // if the list is scrolled to it, as its words take more room than the window leaves.
    let mut window = Window::new(smallest());
    window.ask(Asked::At("not-for"));
    let seen = window.settle();
    let topic = TOPICS
        .iter()
        .find(|topic| topic.id == "not-for")
        .expect("listed");
    let body = seen
        .text_bounds(topic.body)
        .expect("the topic asked for is open");
    let title = seen.text_bounds(topic.title).expect("and listed");
    assert!(
        window.shown.contains_rect(body),
        "{body:?} is not in {:?}",
        window.shown
    );
    assert!(
        window.shown.contains_rect(title),
        "{title:?} is not in {:?}",
        window.shown
    );
    for other in TOPICS.iter().filter(|other| other.id != topic.id) {
        assert!(
            !seen.words.iter().any(|word| word == other.body),
            "{} is open",
            other.id
        );
    }
}

#[test]
fn another_topic_asked_for_while_the_help_is_open_is_brought_into_sight_too() {
    let mut window = Window::new(smallest());
    window.ask(Asked::At("measure"));
    let _ = window.settle();
    window.ask(Asked::At("labels"));
    let seen = window.settle();
    let labels = TOPICS
        .iter()
        .find(|topic| topic.id == "labels")
        .expect("listed");
    let title = seen.text_bounds(labels.title).expect("listed");
    let body = seen.text_bounds(labels.body).expect("opened");
    assert!(
        window.shown.contains_rect(title),
        "{title:?} is not in {:?}",
        window.shown
    );
    assert!(
        window.shown.contains_rect(body),
        "{body:?} is not in {:?}",
        window.shown
    );
}

#[test]
fn a_topic_asked_for_can_be_shut_again_by_a_click_on_its_title() {
    let mut window = Window::new(Hand::new(900.0, 700.0));
    window.ask(Asked::At("sides"));
    let seen = window.settle();
    let sides = TOPICS
        .iter()
        .find(|topic| topic.id == "sides")
        .expect("listed");
    assert!(
        seen.words.iter().any(|word| word == sides.body),
        "it is open"
    );
    let title = seen.text_bounds(sides.title).expect("listed");
    let press = |pressed| Event::PointerButton {
        pos: title.center(),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = window.pass(vec![Event::PointerMoved(title.center()), press(true)]);
    let _ = window.pass(vec![press(false)]);
    let seen = window.settle();
    assert!(
        !seen.words.iter().any(|word| word == sides.body),
        "it is still open"
    );
}

#[test]
fn asking_for_the_help_opens_or_shuts_it_and_a_topic_always_opens_it() {
    let ctx = egui::Context::default();
    let mut open = false;
    ask(&ctx, &mut open, None);
    assert!(open);
    ask(&ctx, &mut open, None);
    assert!(!open, "asking again shuts it");
    ask(&ctx, &mut open, Some("measure"));
    assert!(open);
    ask(&ctx, &mut open, Some("labels"));
    assert!(open, "a topic does not shut it");
}

#[test]
fn a_topic_asked_for_and_not_shown_is_not_kept_for_later() {
    // Asked for, then shut before the window could draw it: opening the help again, bare,
    // shows no topic open.
    let hand = Hand::new(900.0, 700.0);
    let mut open = false;
    let _ = hand.pass(Vec::new(), &mut |ui: &mut Ui| {
        ask(ui.ctx(), &mut open, Some("measure"));
        open = false;
        let _ = show(ui.ctx(), &mut open);
    });
    open = true;
    let seen = hand.settle(&mut |ui: &mut Ui| {
        ui.ctx().all_styles_mut(|style| style.animation_time = 0.0);
        let _ = show(ui.ctx(), &mut open);
    });
    assert!(
        seen.text_bounds(TOPICS[0].title).is_some(),
        "the topics are listed"
    );
    let measure = TOPICS
        .iter()
        .find(|topic| topic.id == "measure")
        .expect("listed");
    assert!(
        !seen.words.iter().any(|word| word == measure.body),
        "it opened"
    );
}

#[test]
fn the_help_stays_inside_a_narrow_area_with_every_topic_listed() {
    // The pictures keep at least 200 points; a heading does not wrap, so a title must be short
    // enough for the room that leaves.
    for wide in [200.0, 240.0, 300.0] {
        let hand = Hand::new(640.0, 480.0);
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(wide, 400.0));
        let mut open = true;
        let seen = hand.settle(&mut |ui: &mut Ui| {
            crate::ui::floating::keep_to(ui.ctx(), area);
            let _ = show(ui.ctx(), &mut open);
        });
        for topic in &TOPICS {
            let title = seen.text_bounds(topic.title);
            let title = title.unwrap_or_else(|| panic!("{} is not listed at {wide}", topic.id));
            assert!(
                title.right() <= area.right() + 0.5,
                "{} runs out of {wide} points: {title:?}",
                topic.id
            );
        }
    }
}
