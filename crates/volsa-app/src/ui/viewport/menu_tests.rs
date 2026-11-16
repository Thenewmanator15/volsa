//! The menu of a slice: a click with the other button opens it with its items, each item does
//! what its action does, and a drag with that button sets the window and opens no menu.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Ui, pos2, vec2};
use volsa_core::SeriesId;

use super::menu::ITEMS;
use super::tools_tests::{SIZE, holding};
use super::{Drafts, Slices, show};
use crate::action::Action;
use crate::annotations::Tool;
use crate::measuring::SliceValues;
use crate::state::AppState;
use crate::ui::commands::{Request, TagsOf};
use crate::ui::hand::{Hand, Seen};

/// Where the slice is clicked: off its middle, so the menu opens over the slice and its items
/// lie apart from where it was opened.
const OPENED_AT: Pos2 = pos2(60.0, 60.0);

/// A viewport of the window as the tests see it, and what it asked for.
#[derive(Default)]
struct Window {
    drafts: Drafts,
    actions: Vec<Action>,
    asked: Vec<Request>,
}

impl Window {
    /// Draw the viewports of `state` filling the window, with no graphics device and so no
    /// picture, and keep what they asked for.
    fn draw(&mut self, ui: &mut Ui, state: &AppState) {
        let frame = |_: &SeriesId, _: usize| None;
        let value = |_: &SeriesId, _: usize, _: (usize, usize)| None;
        let read = |_: &SeriesId, _: usize, _: &mut dyn FnMut(SliceValues<'_>)| {};
        let slices = Slices {
            frame: &frame,
            value: &value,
            read: &read,
            labels: None,
            volume: None,
        };
        let area = Rect::from_min_size(Pos2::ZERO, vec2(SIZE, SIZE));
        let held = (&mut [][..], &mut self.drafts);
        let asked = show(
            ui,
            Some(area),
            state,
            &slices,
            held,
            None,
            &mut self.actions,
        );
        self.asked.extend(asked);
    }
}

/// The other button pressed or let go at `at`.
fn other_button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Right-click the slice and say what is drawn once the menu has had a pass to take its place.
fn right_click(hand: &Hand, state: &AppState, window: &mut Window) -> Seen {
    let mut draw = |ui: &mut Ui| window.draw(ui, state);
    let _ = hand.settle(&mut draw);
    let press = vec![
        Event::PointerMoved(OPENED_AT),
        other_button(OPENED_AT, true),
    ];
    let _ = hand.pass(press, &mut draw);
    let _ = hand.pass(vec![other_button(OPENED_AT, false)], &mut draw);
    hand.pass(Vec::new(), &mut draw)
}

/// Open the menu and choose the item showing `words`, and say what the viewport did and asked.
fn choose(words: &str) -> Window {
    let (hand, state, mut window) = (
        Hand::new(SIZE, SIZE),
        holding(Tool::Navigate),
        Window::default(),
    );
    let seen = right_click(&hand, &state, &mut window);
    let item = seen.text_bounds(words).expect("in the menu");
    window.actions.clear();
    hand.click(item.center(), &mut |ui| window.draw(ui, &state));
    let after = hand.pass(Vec::new(), &mut |ui| window.draw(ui, &state));
    assert!(
        after.text_bounds(words).is_none(),
        "the menu closes once chosen"
    );
    window
}

#[test]
fn a_right_click_on_a_slice_opens_its_menu_with_every_item() {
    let (hand, state) = (Hand::new(SIZE, SIZE), holding(Tool::Navigate));
    let mut window = Window::default();
    let before = hand.settle(&mut |ui| window.draw(ui, &state));
    assert!(
        before.text_bounds("Reset view").is_none(),
        "closed at first"
    );
    let seen = right_click(&hand, &state, &mut window);
    let shown: Vec<&str> = ITEMS.iter().map(|(_, words)| *words).collect();
    assert_eq!(
        shown,
        [
            "Fit",
            "Mirror across",
            "Mirror down",
            "Turn",
            "Reset view",
            "Auto brightness",
            "Save this slice as a picture…",
            "Show tags",
        ]
    );
    let mut tops = Vec::new();
    for words in shown {
        let at = seen.text_bounds(words);
        let at = at.unwrap_or_else(|| panic!("{words} is not in the menu: {:?}", seen.words));
        tops.push(at.top());
    }
    assert!(tops.is_sorted(), "in order, from the top down: {tops:?}");
    // Every item is an action of the list, which the search finds too.
    for (id, _) in ITEMS {
        assert!(crate::ui::commands::find(id).is_some(), "{id}");
    }
}

#[test]
fn fit_and_reset_in_the_menu_do_what_their_actions_do() {
    let fit = choose("Fit");
    assert!(
        fit.actions.contains(&Action::Fit { viewport: 0 }),
        "{:?}",
        fit.actions
    );
    let reset = choose("Reset view");
    assert!(
        reset.actions.contains(&Action::ResetView(0)),
        "{:?}",
        reset.actions
    );
    let across = choose("Mirror across");
    let mirrored = Action::Flip {
        viewport: 0,
        horizontal: true,
    };
    assert!(across.actions.contains(&mirrored), "{:?}", across.actions);
    let auto = choose("Auto brightness");
    let preset = Action::ApplyPreset {
        viewport: 0,
        preset: 0,
    };
    assert!(auto.actions.contains(&preset), "{:?}", auto.actions);
}

#[test]
fn saving_and_the_tags_are_asked_of_the_window() {
    let save = choose("Save this slice as a picture…");
    assert_eq!(save.asked, [Request::SaveSlice]);
    let tags = choose("Show tags");
    assert_eq!(tags.asked, [Request::Tags(TagsOf::Shown)]);
}

#[test]
fn a_right_drag_sets_the_window_and_opens_no_menu() {
    let (hand, state) = (Hand::new(SIZE, SIZE), holding(Tool::Navigate));
    let mut window = Window::default();
    let mut draw = |ui: &mut Ui| window.draw(ui, &state);
    let _ = hand.settle(&mut draw);
    let (from, to) = (OPENED_AT, OPENED_AT + vec2(80.0, 40.0));
    let _ = hand.pass(
        vec![Event::PointerMoved(from), other_button(from, true)],
        &mut draw,
    );
    let _ = hand.pass(
        vec![Event::PointerMoved(from + vec2(40.0, 20.0))],
        &mut draw,
    );
    let _ = hand.pass(vec![Event::PointerMoved(to)], &mut draw);
    let _ = hand.pass(vec![other_button(to, false)], &mut draw);
    let seen = hand.pass(Vec::new(), &mut draw);
    for (_, words) in ITEMS {
        assert!(seen.text_bounds(words).is_none(), "{words} after a drag");
    }
    let windowed = |action: &Action| matches!(action, Action::AdjustWindow { viewport: 0, .. });
    assert!(window.actions.iter().any(windowed), "{:?}", window.actions);
    assert_eq!(window.asked, Vec::<Request>::new());
}
