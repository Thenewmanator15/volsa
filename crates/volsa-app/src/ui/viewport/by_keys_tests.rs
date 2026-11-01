//! Measuring on a slice by keyboard: with a measuring tool in hand and the viewport holding the
//! keyboard, the arrows move a point on the slice and Enter places it, as a press and a release
//! do with the mouse; what the keys leave to the slices, to Escape and to the text fields; and
//! what a screen reader is told.

use egui::{Event, Key, Modifiers};
use volsa_core::{Patient, Point};
use volsa_volume::distance_mm;

use super::said::said;
use super::tools_tests::{Rig, SIZE, holding, over};
use crate::action::Action;
use crate::annotations::{Shape, Tool};
use crate::reducer::reduce;
use crate::state::{AppState, NoticeKind};
use crate::testing::{THICK_SCAN, id};
use crate::ui::hand::{Hand, Seen};

/// The words a screen reader is told with a ruler in hand, before the point has been moved.
const HOW_TO_RULE: &str = "Ruler: arrows move the point, Enter places it";

/// A key pressed, with Shift held or not.
pub(super) fn key(key: Key, shift: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        },
    }
}

/// A key pressed and let go again, with Shift held or not.
pub(super) fn tap(pressed: Key, shift: bool) -> Vec<Event> {
    let down = key(pressed, shift);
    let Event::Key {
        key,
        physical_key,
        repeat,
        modifiers,
        ..
    } = down.clone()
    else {
        unreachable!("a key");
    };
    let up = Event::Key {
        key,
        physical_key,
        pressed: false,
        repeat,
        modifiers,
    };
    vec![down, up]
}

/// A viewport that holds the keyboard, in a window that has seen it for two frames.
pub(super) fn keyed(hand: &Hand, state: &AppState) -> Rig {
    let mut rig = Rig::default();
    rig.wants_keys = true;
    let _ = rig.frame(hand, state, 0, Vec::new());
    let _ = rig.frame(hand, state, 0, Vec::new());
    assert!(rig.has_keys, "the viewport holds the keyboard");
    rig
}

/// Press these keys one frame after another, and say what the last frame drew.
pub(super) fn press(rig: &mut Rig, (hand, state): (&Hand, &AppState), keys: &[Key]) -> Seen {
    let mut seen = Seen::default();
    for pressed in keys {
        seen = rig.frame(hand, state, 0, tap(*pressed, false));
    }
    seen
}

/// Where the middle of a pixel of the 30-slice series' slice 15 is, in millimetres.
fn pixel(state: &AppState, col: f64, row: f64) -> Point<Patient> {
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    placement.apply(Point::new(col, row, 15.0))
}

pub(super) fn point([x, y, z]: [f64; 3]) -> Point<Patient> {
    Point::new(x, y, z)
}

/// Whether two places are the same to a nanometre.
pub(super) fn same(a: Point<Patient>, b: Point<Patient>) -> bool {
    distance_mm(a, b) < 1.0e-9
}

/// What the mouse makes of a click at each of these places of the viewport, or of a press at
/// the first and a release at the second, with the tool in hand.
fn by_mouse(tool: Tool, places: &[egui::Pos2]) -> Vec<Shape> {
    let state = holding(tool);
    let (hand, mut rig) = (Hand::new(SIZE, SIZE), Rig::default());
    match places {
        [from, to] if matches!(tool, Tool::Ruler | Tool::Region) => {
            let _ = rig.frame(&hand, &state, 0, Vec::new());
            let down = vec![
                Event::PointerMoved(*from),
                Event::PointerButton {
                    pos: *from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ];
            let _ = rig.frame(&hand, &state, 0, down);
            let _ = rig.frame(&hand, &state, 0, vec![Event::PointerMoved(*to)]);
            let up = Event::PointerButton {
                pos: *to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            };
            let _ = rig.frame(&hand, &state, 0, vec![up]);
        }
        _ => places
            .iter()
            .for_each(|at| rig.click(&hand, &state, 0, *at)),
    }
    rig.added().into_iter().cloned().collect()
}

#[test]
fn a_ruler_is_drawn_with_enter_and_arrows_and_is_the_ruler_the_mouse_draws() {
    let state = holding(Tool::Ruler);
    let hand = Hand::new(SIZE, SIZE);
    let mut rig = keyed(&hand, &state);
    let with = (&hand, &state);
    // The point begins at the middle, (2, 2). Shift takes it ten pixels, to the edge.
    let _ = rig.frame(&hand, &state, 0, tap(Key::ArrowLeft, true));
    let _ = press(&mut rig, with, &[Key::ArrowUp, Key::Enter]);
    assert_eq!(rig.added().len(), 0, "begun, not finished");
    let _ = press(&mut rig, with, &[Key::ArrowRight; 3]);
    let _ = press(&mut rig, with, &[Key::Enter]);
    let [Shape::Ruler { a, b }] = rig.added()[..] else {
        panic!("{:?}", rig.actions);
    };
    assert!(same(point(*a), pixel(&state, 0.0, 1.0)), "{a:?}");
    assert!(same(point(*b), pixel(&state, 3.0, 1.0)), "{b:?}");
    let [Shape::Ruler { a: from, b: to }] =
        by_mouse(Tool::Ruler, &[over(0.0, 1.0), over(3.0, 1.0)])[..]
    else {
        panic!("the mouse draws one ruler");
    };
    assert!(same(point(*a), point(from)) && same(point(*b), point(to)));
    // It is the same annotation, in the list, which can be undone.
    let mut state = state;
    for action in std::mem::take(&mut rig.actions) {
        reduce(&mut state, action);
    }
    assert_eq!(state.annotations.items.len(), 1);
    reduce(&mut state, Action::Undo);
    assert_eq!(state.annotations.items, []);
}

#[test]
fn an_angle_is_three_enters() {
    let hand = Hand::new(SIZE, SIZE);
    // Arm at (2, 2), vertex at (0, 0), other arm at (3, 0).
    let state = holding(Tool::Angle);
    let mut rig = keyed(&hand, &state);
    let with = (&hand, &state);
    let _ = press(
        &mut rig,
        with,
        &[Key::Enter, Key::ArrowLeft, Key::ArrowLeft],
    );
    let _ = press(&mut rig, with, &[Key::ArrowUp, Key::ArrowUp, Key::Enter]);
    assert_eq!(rig.added().len(), 0, "two of three");
    let _ = press(&mut rig, with, &[Key::ArrowRight; 3]);
    let _ = press(&mut rig, with, &[Key::Enter]);
    let [Shape::Angle { a, vertex, b }] = rig.added()[..] else {
        panic!("{:?}", rig.actions);
    };
    let places = [over(2.0, 2.0), over(0.0, 0.0), over(3.0, 0.0)];
    let [
        Shape::Angle {
            a: x,
            vertex: y,
            b: z,
        },
    ] = by_mouse(Tool::Angle, &places)[..]
    else {
        panic!("the mouse draws one angle");
    };
    for (ours, theirs) in [(a, &x), (vertex, &y), (b, &z)] {
        assert!(same(point(*ours), point(*theirs)), "{ours:?} {theirs:?}");
    }
}

#[test]
fn a_region_is_two_enters_and_holds_what_the_slice_holds_inside_it() {
    let hand = Hand::new(SIZE, SIZE);
    // From (1, 1) to (2, 2).
    let state = holding(Tool::Region);
    let mut rig = keyed(&hand, &state);
    let with = (&hand, &state);
    let _ = press(&mut rig, with, &[Key::ArrowLeft, Key::ArrowUp, Key::Enter]);
    let _ = press(
        &mut rig,
        with,
        &[Key::ArrowRight, Key::ArrowDown, Key::Enter],
    );
    let [
        Shape::Region {
            centre,
            radii_mm,
            stats,
            ..
        },
    ] = rig.added()[..]
    else {
        panic!("{:?}", rig.actions);
    };
    let [
        Shape::Region {
            centre: middle,
            radii_mm: radii,
            ..
        },
    ] = by_mouse(Tool::Region, &[over(1.0, 1.0), over(2.0, 2.0)])[..]
    else {
        panic!("the mouse draws one region");
    };
    assert!(same(point(*centre), point(middle)));
    assert!((radii_mm[0] - radii[0]).abs() < 1.0e-9 && (radii_mm[1] - radii[1]).abs() < 1.0e-9);
    assert_eq!(stats.as_ref().map(|inside| inside.mean), Some(7.0));
}

#[test]
fn a_marker_is_one_enter_at_the_middle_of_the_slice() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Marker);
    let mut rig = keyed(&hand, &state);
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    let [Shape::Marker { at }] = rig.added()[..] else {
        panic!("{:?}", rig.actions);
    };
    assert!(same(point(*at), pixel(&state, 2.0, 2.0)), "{at:?}");
}

#[test]
fn the_point_begins_where_the_pointer_last_was_and_never_leaves_the_slice() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Marker);
    let mut rig = keyed(&hand, &state);
    // The pointer is over the slice's top right pixel; the first Enter is placed there.
    let _ = rig.frame(&hand, &state, 0, vec![Event::PointerMoved(over(3.0, 0.0))]);
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    // The arrows held against the edges stay on the slice.
    let _ = press(&mut rig, (&hand, &state), &[Key::ArrowRight; 5]);
    let _ = press(&mut rig, (&hand, &state), &[Key::ArrowUp; 5]);
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    let _ = rig.frame(&hand, &state, 0, tap(Key::ArrowLeft, true));
    let _ = rig.frame(&hand, &state, 0, tap(Key::ArrowDown, true));
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    let at = |shape: &Shape| match shape {
        Shape::Marker { at } => point(*at),
        other => panic!("{other:?}"),
    };
    let marks: Vec<Point<Patient>> = rig.added().into_iter().map(at).collect();
    let wanted = [(3.0, 0.0), (3.0, 0.0), (0.0, 3.0)];
    assert_eq!(marks.len(), wanted.len(), "{:?}", rig.actions);
    for (mark, (col, row)) in marks.iter().zip(wanted) {
        assert!(same(*mark, pixel(&state, col, row)), "{mark:?} {col} {row}");
    }
}

#[test]
fn a_ruler_too_short_adds_nothing_and_says_so() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Ruler);
    let mut rig = keyed(&hand, &state);
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter, Key::Enter]);
    assert_eq!(rig.added().len(), 0);
    let [Action::Post(notice)] = &rig.actions[..] else {
        panic!("{:?}", rig.actions);
    };
    assert!(notice.text.contains("too short"), "{}", notice.text);
    assert_eq!(notice.kind, NoticeKind::Info);
}

#[test]
fn finishing_a_measurement_posts_the_words_the_list_gives_it_and_the_viewport_says_them() {
    let hand = Hand::new(SIZE, SIZE);
    let mut state = crate::testing::thick_scan_ready();
    crate::testing::shown(&mut state);
    reduce(&mut state, Action::SetTool(Tool::Ruler));
    assert_eq!(state.tool, Tool::Ruler, "the volume is ready to measure on");
    let mut rig = keyed(&hand, &state);
    let label = said(&state, 0);
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert_eq!(
        seen.value_of(&label),
        Some(HOW_TO_RULE),
        "{:?}",
        seen.values
    );
    let _ = press(&mut rig, (&hand, &state), &[Key::ArrowLeft, Key::Enter]);
    let seen = press(
        &mut rig,
        (&hand, &state),
        &[Key::ArrowRight, Key::ArrowRight],
    );
    let said_now = seen.value_of(&label).expect("a value");
    assert_eq!(said_now, "1.8 mm from the start; point at column 4, row 3");
    let _ = press(&mut rig, (&hand, &state), &[Key::Enter]);
    for action in std::mem::take(&mut rig.actions) {
        reduce(&mut state, action);
    }
    assert_eq!(state.annotations.items.len(), 1, "added");
    assert_eq!(state.tool, Tool::Ruler, "{:?}", state.tool);
    // The frame after the one that finished it knows the name the list gave it.
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    let [Action::Post(notice)] = &rig.actions[..] else {
        panic!("{:?}", rig.actions);
    };
    assert_eq!(notice.text, "Ruler 1: 1.8 mm");
    assert_eq!(notice.kind, NoticeKind::Info);
    assert_eq!(seen.value_of(&label), Some("Ruler 1: 1.8 mm"));
}

#[test]
fn escape_gives_up_a_half_made_ruler_and_keeps_the_point_a_second_escape_leaves_it() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Ruler);
    let mut rig = keyed(&hand, &state);
    let with = (&hand, &state);
    let _ = press(&mut rig, with, &[Key::Enter, Key::ArrowRight, Key::Escape]);
    assert!(
        rig.has_keys,
        "the first Escape leaves the point where it is"
    );
    // What was begun is gone: this Enter begins another, and nothing is added.
    let _ = press(&mut rig, with, &[Key::Enter, Key::ArrowRight]);
    assert_eq!(rig.added().len(), 0, "{:?}", rig.actions);
    let _ = press(&mut rig, with, &[Key::Escape]);
    assert!(rig.has_keys, "that Escape gave up the second ruler");
    let _ = press(&mut rig, with, &[Key::Escape]);
    assert!(!rig.has_keys, "the second Escape leaves the point");
    assert_eq!(rig.added().len(), 0);
}

#[test]
fn the_tool_and_how_to_use_it_are_said_again_when_the_keyboard_comes_back_to_the_viewport() {
    let hand = Hand::new(SIZE, SIZE);
    let state = holding(Tool::Ruler);
    let mut rig = keyed(&hand, &state);
    let label = said(&state, 0);
    let seen = press(&mut rig, (&hand, &state), &[Key::ArrowRight]);
    let moved = seen.value_of(&label).expect("a value");
    assert!(moved.starts_with("Point at column"), "{moved}");
    // Esc with nothing begun leaves the viewport; Tab brings the keyboard back.
    let _ = press(&mut rig, (&hand, &state), &[Key::Escape]);
    assert!(!rig.has_keys);
    rig.wants_keys = true;
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert_eq!(
        seen.value_of(&label),
        Some(HOW_TO_RULE),
        "{:?}",
        seen.values
    );
}
