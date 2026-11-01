//! Measuring by keyboard as the window answers it: which keys the point takes from the slices,
//! that none is taken while a text field or the search box has the keyboard, how the point is
//! drawn, and that an arrow goes the way it points on the screen.

use egui::{Event, Key, Ui};

use super::by_keys_tests::{keyed, point, press, same, tap};
use super::tools_tests::{Rig, over};
use super::tools_tests::{SIZE, holding};
use super::{Drafts, Slices, show};
use crate::action::Action;
use crate::annotations::Shape;
use crate::annotations::Tool;
use crate::measuring::SliceValues;
use crate::reducer::reduce;
use crate::ui::hand::Hand;
use crate::ui::hand::Seen;
use crate::ui::keys;

/// What the window asks for in the last of these passes, with `tool` in hand: the viewport as it
/// is drawn, then the keys as the window answers them. The first pass gives the viewport the
/// keyboard, as Tab does, if `tab` is true.
fn window(tool: Tool, tab: bool, passes: Vec<Vec<Event>>) -> Vec<Action> {
    let state = holding(tool);
    let hand = Hand::new(SIZE, SIZE);
    let mut drafts = Drafts::default();
    let mut asked = Vec::new();
    for (at, events) in passes.into_iter().enumerate() {
        asked.clear();
        let mut draw = |ui: &mut Ui| {
            let here = ui.id().with(("viewport", 0_usize));
            if tab && at == 0 {
                ui.memory_mut(|memory| memory.request_focus(here));
            }
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE, SIZE));
            let frame = |_: &volsa_core::SeriesId, _: usize| None;
            let value = |_: &volsa_core::SeriesId, _: usize, _: (usize, usize)| None;
            let read = |_: &volsa_core::SeriesId, _: usize, _: &mut dyn FnMut(SliceValues<'_>)| {};
            let slices = Slices {
                frame: &frame,
                value: &value,
                read: &read,
            };
            show(
                ui,
                Some(area),
                &state,
                &slices,
                (&mut [], &mut drafts),
                None,
                &mut asked,
            );
            asked.extend(keys::answered(ui.ctx(), &state).0);
        };
        let _ = hand.pass(events, &mut draw);
    }
    asked
}

/// What the window asks for when this key is pressed with the viewport holding the keyboard
/// and `tool` in hand.
fn asked_by(tool: Tool, pressed: Key) -> Vec<Action> {
    window(
        tool,
        true,
        vec![Vec::new(), Vec::new(), tap(pressed, false)],
    )
}

/// A press and release of the main button at the middle of the viewport, as two passes.
fn click() -> Vec<Vec<Event>> {
    let at = egui::pos2(200.0, 200.0);
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![
        vec![Event::PointerMoved(at), button(true)],
        vec![button(false)],
    ]
}

#[test]
fn a_click_never_gives_the_viewport_the_keyboard_so_a_mouse_user_keeps_the_slices_on_the_arrows() {
    let scroll = Action::Scroll {
        viewport: 0,
        delta: 1,
    };
    for tool in [Tool::Ruler, Tool::Marker] {
        let passes = [vec![Vec::new()], click(), vec![tap(Key::ArrowDown, false)]].concat();
        assert_eq!(
            window(tool, false, passes),
            std::slice::from_ref(&scroll),
            "{tool:?}"
        );
    }
}

#[test]
fn a_press_of_the_mouse_gives_the_arrows_back_to_the_slices_until_the_next_tab_into_the_viewport() {
    let scroll = Action::Scroll {
        viewport: 0,
        delta: 1,
    };
    let down = tap(Key::ArrowDown, false);
    // Tab to the viewport, measure with the mouse, press Down: the slice steps.
    let after_a_press = [vec![Vec::new(), Vec::new()], click(), vec![down.clone()]].concat();
    assert_eq!(
        window(Tool::Ruler, true, after_a_press),
        std::slice::from_ref(&scroll)
    );
    // Without the press, Down moves the point.
    let no_press = vec![Vec::new(), Vec::new(), down.clone()];
    assert_eq!(window(Tool::Ruler, true, no_press), []);
    // Esc leaves the viewport (nothing is begun) and the next Tab into it engages the point again.
    // A second window pass requests the keyboard again as Tab does.
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Marker);
    let mut rig = keyed(&hand, &state);
    rig.click(&hand, &state, 0, egui::pos2(200.0, 200.0));
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert!(
        !seen.lines.iter().any(|[from, to]| from.distance(*to) > 4.0),
        "no cross after a press: {:?}",
        seen.lines
    );
    let _ = press(&mut rig, (&hand, &state), &[Key::Escape]);
    assert!(!rig.has_keys);
    rig.wants_keys = true;
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert!(
        seen.lines.iter().any(|[from, to]| from.distance(*to) > 4.0),
        "the cross is back: {:?}",
        seen.lines
    );
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    assert_eq!(
        rig.added().len(),
        2,
        "the click placed one, Enter the other: {:?}",
        rig.actions
    );
}

#[test]
fn up_and_down_move_the_point_not_the_slice_with_a_tool_in_hand_and_the_page_keys_change_the_slice()
{
    let scrolled = |delta: i32| Action::Scroll { viewport: 0, delta };
    for tool in [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker] {
        assert_eq!(asked_by(tool, Key::ArrowDown), [], "{tool:?}");
        assert_eq!(asked_by(tool, Key::ArrowUp), [], "{tool:?}");
        assert_eq!(asked_by(tool, Key::PageDown), [scrolled(10)], "{tool:?}");
        assert_eq!(asked_by(tool, Key::PageUp), [scrolled(-10)], "{tool:?}");
    }
    // With the Navigate tool the arrows step through the slices, as they did.
    assert_eq!(asked_by(Tool::Navigate, Key::ArrowDown), [scrolled(1)]);
    assert_eq!(asked_by(Tool::Navigate, Key::ArrowUp), [scrolled(-1)]);
}

#[test]
fn page_down_gives_up_a_half_made_ruler() {
    let hand = Hand::new(SIZE, SIZE);
    let mut state = holding(Tool::Ruler);
    let mut rig = keyed(&hand, &state);
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter, Key::ArrowRight]);
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 10,
        },
    );
    assert_eq!(state.tool, Tool::Ruler, "the tool stays in hand");
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    assert_eq!(rig.added().len(), 0, "{:?}", rig.actions);
}

/// Where `other`, which draws something else that may have the keyboard, is drawn in a pass.
#[derive(Clone, Copy, PartialEq)]
enum Other {
    /// After the viewport was given the keyboard in the pass: a window that has opened.
    After,
    /// Before the viewport is drawn, and the viewport is not given the keyboard again: a field
    /// that has it.
    Before,
}

/// How many steps the viewport took from two keys pressed in the second of two passes, and how
/// many of the key presses were left in the input, with `other` drawn in each pass.
fn taken_while(order: Other, other: &mut dyn FnMut(&mut Ui)) -> (usize, usize) {
    let state = holding(Tool::Ruler);
    let hand = Hand::new(SIZE, SIZE);
    let (mut steps, mut left) = (usize::MAX, 0);
    let both = [tap(Key::ArrowRight, false), tap(Key::Enter, false)].concat();
    for events in [Vec::new(), both] {
        let _ = hand.pass(events, &mut |ui: &mut Ui| {
            if order == Other::Before {
                other(ui);
            }
            let cell = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE, SIZE));
            let here = ui.id().with(("viewport", 0_usize));
            let viewport = ui.interact(cell, here, egui::Sense::click_and_drag());
            if order == Other::After {
                ui.memory_mut(|memory| memory.request_focus(here));
            }
            steps = super::by_keys::take(ui, &viewport, &state, 0).steps.len();
            left = ui.input(|input| {
                let events = input.events.iter();
                events
                    .filter(|event| matches!(event, Event::Key { pressed: true, .. }))
                    .count()
            });
            if order == Other::After {
                other(ui);
            }
        });
    }
    (steps, left)
}

#[test]
fn no_key_is_taken_while_the_search_box_or_a_name_field_has_the_keyboard() {
    // With nothing else asking for the keyboard the viewport takes both keys.
    assert_eq!(taken_while(Other::After, &mut |_| {}), (2, 0));
    // The search box is open: it has the keyboard from the pass it was opened in.
    let mut search = crate::ui::search::Search::default();
    search.open();
    let state = holding(Tool::Ruler);
    let found = taken_while(Other::After, &mut |ui| {
        let _ = search.show(ui.ctx(), &state);
    });
    assert_eq!(found, (0, 2), "the search box");
    // A measurement's name is being typed over.
    let mut name = String::from("Ruler 1");
    let found = taken_while(Other::Before, &mut |ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut name).id(egui::Id::new("a name")));
        field.request_focus();
    });
    assert_eq!(found, (0, 2), "a name field");
}

#[test]
fn the_point_is_drawn_in_the_accent_colour_only_while_the_viewport_holds_the_keyboard() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Ruler);
    let long_through = |seen: &Seen, at: egui::Pos2| {
        seen.lines.iter().any(|[from, to]| {
            let middle = egui::pos2(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y));
            (middle - at).length() < 0.5 && (from.distance(*to) > 4.0)
        })
    };
    let mut rig = keyed(&hand, &state);
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert!(long_through(&seen, over(2.0, 2.0)), "{:?}", seen.lines);
    let seen = press(&mut rig, (&hand, &state), &[Key::ArrowRight]);
    assert!(long_through(&seen, over(3.0, 2.0)), "{:?}", seen.lines);
    let (apart, mut without) = (Hand::new(SIZE, SIZE), Rig::default());
    let seen = without.frame(&apart, &state, 0, Vec::new());
    assert!(!long_through(&seen, over(2.0, 2.0)), "{:?}", seen.lines);
}

#[test]
fn an_arrow_moves_the_point_the_way_it_points_on_the_screen_whichever_way_the_picture_is_turned() {
    let hand = Hand::new(SIZE, SIZE);
    for turns in 0..4 {
        for arrow in [
            Key::ArrowRight,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowUp,
        ] {
            let mut state = holding(Tool::Marker);
            for _ in 0..turns {
                reduce(&mut state, Action::Rotate { viewport: 0 });
            }
            let mut rig = keyed(&hand, &state);
            let _ = press(&mut rig, (&hand, &state), &[arrow, Key::Enter]);
            let [Shape::Marker { at }] = rig.added()[..] else {
                panic!("{turns} turns, {arrow:?}: {:?}", rig.actions);
            };
            // The mouse, clicking a pixel's width from the middle the way the arrow points.
            let (dx, dy) = match arrow {
                Key::ArrowRight => (100.0, 0.0),
                Key::ArrowLeft => (-100.0, 0.0),
                Key::ArrowDown => (0.0, 100.0),
                _ => (0.0, -100.0),
            };
            // The middle pixel turns about the middle of the picture with the picture.
            let (mut x, mut y) = (50.0_f32, 50.0_f32);
            for _ in 0..turns {
                (x, y) = (-y, x);
            }
            let clicked = egui::pos2(200.0 + x + dx, 200.0 + y + dy);
            let (hand, mut by_hand) = (Hand::new(SIZE, SIZE), Rig::default());
            by_hand.click(&hand, &state, 0, clicked);
            let [Shape::Marker { at: there }] = by_hand.added()[..] else {
                panic!("{turns} turns, {arrow:?}: {:?}", by_hand.actions);
            };
            assert!(
                same(point(*at), point(*there)),
                "{turns} turns, {arrow:?}: {at:?} {there:?}"
            );
        }
    }
}
