//! Measuring on a slice with the mouse: what presses, moves and releases over a viewport make
//! with the tool in hand, that what is half drawn is given up when the scan or slice shown
//! changes, and what is written beside what was measured.

use std::cell::Cell;

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Sense, pos2, vec2};
use voxa_core::{Point, SeriesId};
use voxa_volume::distance_mm;

use super::tools::{self, Drafts};
use crate::action::Action;
use crate::annotations::{Shape, Tool};
use crate::measuring::{Scale, SliceValues};
use crate::reducer::reduce;
use crate::state::{AppState, Layout};
use crate::testing::{FINE_SCAN, THICK_SCAN, id, showing_a_picture, shown, three_series};
use crate::ui::hand::{Hand, Seen};

/// The viewport's size in these tests: the 4 x 4 slices of the 30-slice series fill it, a
/// hundred points to a pixel.
pub(super) const SIZE: f32 = 400.0;

/// What a slice holds in these tests: a 4 x 4 slice of 7s.
static SEVENS: [f32; 16] = [7.0; 16];

/// The 30-slice series shown in the first viewport, its slice decoded, its volume still being
/// built, with `tool` in hand.
pub(super) fn holding(tool: Tool) -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    reduce(&mut state, Action::SetTool(tool));
    assert_eq!(state.tool, tool, "taken up on the slice");
    state
}

/// The main button pressed or let go at a point.
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Where the middle of a pixel of the 30-slice series is in the viewport.
pub(super) fn over(col: f32, row: f32) -> Pos2 {
    pos2(50.0 + 100.0 * col, 50.0 + 100.0 * row)
}

/// A viewport as the window keeps it between frames, and what it asked for.
pub(super) struct Rig {
    /// Where the viewport is.
    cell: Rect,
    pub(super) drafts: Drafts,
    pub(super) actions: Vec<Action>,
    /// How many times a slice's values were read.
    reads: Cell<usize>,
}

impl Default for Rig {
    fn default() -> Self {
        Self::wide(SIZE)
    }
}

impl Rig {
    /// A viewport `width` points wide and [`SIZE`] tall, at the window's corner.
    fn wide(width: f32) -> Self {
        Self {
            cell: Rect::from_min_size(Pos2::ZERO, vec2(width, SIZE)),
            drafts: Drafts::default(),
            actions: Vec::new(),
            reads: Cell::new(0),
        }
    }

    /// One frame of viewport `at` of `state`, with these events, as the window draws it.
    pub(super) fn frame(
        &mut self,
        hand: &Hand,
        state: &AppState,
        at: usize,
        events: Vec<Event>,
    ) -> Seen {
        let cell = self.cell;
        let (drafts, actions, reads) = (&mut self.drafts, &mut self.actions, &self.reads);
        let read = |_: &SeriesId, _: usize, with: &mut dyn FnMut(SliceValues<'_>)| {
            reads.set(reads.get() + 1);
            with(SliceValues {
                values: &SEVENS,
                size: [4, 4],
            });
        };
        hand.pass(events, &mut |ui| {
            drafts.keep_up(state, false);
            let id = ui.id().with(("viewport", at));
            let response = ui.interact(cell, id, Sense::click_and_drag());
            let pane = super::pane(state, at, cell);
            actions.extend(tools::measure(ui, &pane, &response, (drafts, &read), &[]));
        })
    }

    /// A press at `from`, the pointer moved to `to`, and there let go: three frames, after one
    /// with nothing happening, in which the viewport finds its place.
    fn drag(&mut self, hand: &Hand, state: &AppState, (from, to): (Pos2, Pos2)) {
        let _ = self.frame(hand, state, 0, Vec::new());
        let down = vec![Event::PointerMoved(from), button(from, true)];
        let _ = self.frame(hand, state, 0, down);
        let _ = self.frame(hand, state, 0, vec![Event::PointerMoved(to)]);
        let _ = self.frame(hand, state, 0, vec![button(to, false)]);
    }

    /// A click on viewport `at` at a point.
    pub(super) fn click(&mut self, hand: &Hand, state: &AppState, at: usize, on: Pos2) {
        let _ = self.frame(hand, state, at, Vec::new());
        let down = vec![Event::PointerMoved(on), button(on, true)];
        let _ = self.frame(hand, state, at, down);
        let _ = self.frame(hand, state, at, vec![button(on, false)]);
    }

    /// What the viewport asked to add, so far.
    pub(super) fn added(&self) -> Vec<&Shape> {
        let added = self.actions.iter().filter_map(|action| match action {
            Action::AnnotationAdd(shape) => Some(shape),
            _ => None,
        });
        added.collect()
    }
}

fn point([x, y, z]: [f64; 3]) -> Point<voxa_core::Patient> {
    Point::new(x, y, z)
}

#[test]
fn a_ruler_is_drawn_on_a_slice_from_where_it_is_pressed_to_where_it_is_let_go() {
    let state = holding(Tool::Ruler);
    let (hand, mut rig) = (Hand::new(SIZE, SIZE), Rig::default());
    rig.drag(&hand, &state, (over(0.0, 1.0), over(3.0, 1.0)));
    let [Shape::Ruler { a, b }] = rig.added()[..] else {
        panic!("{:?}", rig.actions);
    };
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let on_slice = |col: f64| placement.apply(Point::new(col, 1.0, 15.0));
    assert!(distance_mm(point(*a), on_slice(0.0)) < 1.0e-9, "{a:?}");
    assert!(distance_mm(point(*b), on_slice(3.0)) < 1.0e-9, "{b:?}");
    assert!(
        (distance_mm(point(*a), point(*b)) - 2.7).abs() < 1.0e-9,
        "three columns"
    );
    assert_eq!(rig.reads.get(), 0, "a ruler reads no values");
}

#[test]
fn a_half_drawn_angle_is_dropped_when_the_scan_changes() {
    // Review Focus 4: the scan shown changes, as a card, search or [ and ] change it, while an
    // angle is half drawn. Nothing is added, to the series it was begun on or to the new one.
    let hand = Hand::new(SIZE, SIZE);
    let (first, second, third) = (over(0.0, 0.0), over(1.0, 2.0), over(3.0, 0.0));
    let mut state = holding(Tool::Angle);
    let mut rig = Rig::default();
    rig.click(&hand, &state, 0, first);
    rig.click(&hand, &state, 0, second);
    assert_eq!(rig.added(), [] as [&Shape; 0], "half drawn");
    let (viewport, series) = (0, id(FINE_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    reduce(&mut state, Action::SetTool(Tool::Angle));
    assert_eq!(state.tool, Tool::Angle, "taken up again on the new scan");
    rig.click(&hand, &state, 0, third);
    assert_eq!(rig.added(), [] as [&Shape; 0], "{:?}", rig.actions);
    for action in std::mem::take(&mut rig.actions) {
        reduce(&mut state, action);
    }
    assert_eq!(state.annotations.items, [], "nothing on the new scan");
    // The same three presses with nothing changed in between make an angle.
    let state = holding(Tool::Angle);
    let mut rig = Rig::default();
    for on in [first, second, third] {
        rig.click(&hand, &state, 0, on);
    }
    assert!(
        matches!(rig.added()[..], [Shape::Angle { .. }]),
        "{:?}",
        rig.actions
    );
}

#[test]
fn a_half_drawn_angle_is_dropped_when_the_slice_changes_or_escape_is_pressed() {
    let hand = Hand::new(SIZE, SIZE);
    let (first, second, third) = (over(0.0, 0.0), over(1.0, 2.0), over(3.0, 0.0));
    for change in ["slice", "escape"] {
        let mut state = holding(Tool::Angle);
        let mut rig = Rig::default();
        rig.click(&hand, &state, 0, first);
        rig.click(&hand, &state, 0, second);
        if change == "slice" {
            reduce(
                &mut state,
                Action::Scroll {
                    viewport: 0,
                    delta: 1,
                },
            );
            assert_eq!(state.tool, Tool::Angle, "the tool stays in hand");
        } else {
            rig.drafts.keep_up(&state, true);
        }
        rig.click(&hand, &state, 0, third);
        assert_eq!(
            rig.added(),
            [] as [&Shape; 0],
            "{change}: {:?}",
            rig.actions
        );
    }
}

#[test]
fn a_press_on_a_viewport_that_is_not_the_active_one_draws_nothing() {
    let mut state = holding(Tool::Marker);
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    let (viewport, series) = (1, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    reduce(&mut state, Action::SetActive(0));
    reduce(&mut state, Action::SetTool(Tool::Marker));
    let (hand, mut rig) = (Hand::new(SIZE, SIZE), Rig::default());
    rig.click(&hand, &state, 1, over(1.0, 1.0));
    assert_eq!(
        rig.added(),
        [] as [&Shape; 0],
        "the press only makes it active"
    );
    rig.click(&hand, &state, 0, over(1.0, 1.0));
    assert!(
        matches!(rig.added()[..], [Shape::Marker { .. }]),
        "{:?}",
        rig.actions
    );
}

#[test]
fn a_region_drawn_on_a_slice_reads_its_values_once_as_it_is_finished() {
    let state = holding(Tool::Region);
    let (hand, mut rig) = (Hand::new(SIZE, SIZE), Rig::default());
    rig.drag(&hand, &state, (over(1.5, 1.5), over(2.5, 2.3)));
    let [
        Shape::Region {
            stats: held,
            radii_mm,
            ..
        },
    ] = rig.added()[..]
    else {
        panic!("{:?}", rig.actions);
    };
    let inside = held.as_ref().expect("on the slice");
    assert_eq!(
        (inside.mean, inside.sd, inside.min, inside.max),
        (7.0, 0.0, 7.0, 7.0)
    );
    assert!(
        (radii_mm[0] - 0.9).abs() < 1.0e-9,
        "a column across: {radii_mm:?}"
    );
    assert_eq!(rig.reads.get(), 1, "read once, not each frame");
}

#[test]
fn a_press_off_the_picture_begins_nothing() {
    // A square picture in a viewport twice as wide as tall fills its middle, between bands.
    let mut state = showing_a_picture(Some([0.5, 0.5]), None);
    reduce(&mut state, Action::SetTool(Tool::Marker));
    let (hand, mut rig) = (Hand::new(2.0 * SIZE, SIZE), Rig::wide(2.0 * SIZE));
    rig.click(&hand, &state, 0, pos2(SIZE, SIZE / 2.0));
    assert_eq!(rig.added().len(), 1, "on the picture");
    rig.click(&hand, &state, 0, pos2(5.0, SIZE / 2.0));
    assert_eq!(
        rig.added().len(),
        1,
        "beside it, in its viewport: {:?}",
        rig.actions
    );
}

#[test]
fn what_is_measured_is_written_beside_it_on_its_slice_and_says_where_its_millimetres_come_from() {
    let mut state = showing_a_picture(Some([0.5, 0.5]), None);
    let on_row = |col: f64| [col * 0.5, 1.5, 0.0];
    let ruler = Shape::Ruler {
        a: on_row(2.0),
        b: on_row(12.0),
    };
    reduce(&mut state, Action::AnnotationAdd(ruler));
    assert_eq!(state.annotations.items[0].scale, Scale::PixelSpacing);
    let (hand, mut rig) = (Hand::new(SIZE, SIZE), Rig::default());
    let _ = rig.frame(&hand, &state, 0, Vec::new());
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    let said = "Ruler 1: 5.0 mm (from the file's pixel spacing)";
    assert!(
        seen.words.iter().any(|word| word == said),
        "{:?}",
        seen.words
    );
    // In the patient, nothing is said after it; and on another slice it is not drawn.
    let mut state = holding(Tool::Navigate);
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let at = info
        .voxel_to_patient
        .expect("placed")
        .apply(Point::new(1.0, 1.0, 15.0));
    reduce(
        &mut state,
        Action::AnnotationAdd(Shape::Marker {
            at: [at.x, at.y, at.z],
        }),
    );
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert!(
        seen.words.iter().any(|word| word == "Marker 1"),
        "{:?}",
        seen.words
    );
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 1,
        },
    );
    let seen = rig.frame(&hand, &state, 0, Vec::new());
    assert!(
        !seen.words.iter().any(|word| word == "Marker 1"),
        "{:?}",
        seen.words
    );
}
