//! The box covers the window while it is open: the press that closes it, by being outside
//! the box, does not also reach what is under it, and the box answers the mouse again when
//! it is opened after that.

use egui::{Context, Event, Pos2, Rect, vec2};

use super::box_tests::{opened, typed, window};
use super::entries::Target;
use super::*;
use crate::annotations::Tool;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;
use crate::ui::planes::{self, PlanesView};
use crate::ui::seen::{self, Seen};

/// The window these passes are in: wide, with the three plane views along its lower half.
const SCREEN: [f32; 2] = [900.0, 600.0];

/// The state with the tool in hand.
fn holding(tool: Tool) -> AppState {
    let mut state = thick_scan_ready();
    reduce(&mut state, Action::SetTool(tool));
    state
}

/// The three plane views, side by side below where the box hangs.
fn cells() -> [Rect; 3] {
    let at = |left: f32| Rect::from_min_size(Pos2::new(left, 200.0), vec2(290.0, 380.0));
    [at(5.0), at(305.0), at(605.0)]
}

/// What a pass of the window gave: what the plane views asked for, what the box chose, and
/// what was drawn.
struct Pass {
    actions: Vec<Action>,
    chosen: Option<Target>,
    seen: Seen,
}

/// The window as it draws a frame: the plane views, and then the search box over them.
struct Frame {
    ctx: Context,
    view: PlanesView,
}

impl Frame {
    fn new() -> Self {
        Self {
            ctx: window(),
            view: PlanesView::new(None),
        }
    }

    /// One pass, with these events.
    fn pass(&mut self, search: &mut Search, state: &AppState, events: Vec<Event>) -> Pass {
        let view = &mut self.view;
        let (given, seen) = seen::pass(&self.ctx, SCREEN, events, |ui| {
            let mut actions = Vec::new();
            let from = (None, None, None);
            planes::show(ui, cells(), state, from, view, &mut actions);
            (actions, search.show(ui.ctx(), state))
        });
        let (actions, chosen) = given;
        Pass {
            actions,
            chosen,
            seen,
        }
    }

    /// A few passes with nothing happening, for the box to have its size and its field the
    /// keyboard.
    fn settle(&mut self, search: &mut Search, state: &AppState) -> Pass {
        let mut last = self.pass(search, state, Vec::new());
        for _ in 0..2 {
            last = self.pass(search, state, Vec::new());
        }
        last
    }
}

/// The main button pressed or let go at a point.
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// A press at `from`, the pointer moved to `to`, and then let go: three passes. What the
/// plane views asked for in all of them.
fn drag(
    frame: &mut Frame,
    search: &mut Search,
    state: &AppState,
    (from, to): (Pos2, Pos2),
) -> Vec<Action> {
    let down = vec![Event::PointerMoved(from), button(from, true)];
    let mut asked = frame.pass(search, state, down).actions;
    asked.extend(
        frame
            .pass(search, state, vec![Event::PointerMoved(to)])
            .actions,
    );
    asked.extend(frame.pass(search, state, vec![button(to, false)]).actions);
    asked
}

fn added(actions: &[Action]) -> usize {
    actions
        .iter()
        .filter(|action| matches!(action, Action::AnnotationAdd(_)))
        .count()
}

#[test]
fn a_click_away_from_the_box_closes_it_and_does_not_place_a_point_under_it() {
    for tool in [Tool::Marker, Tool::Ruler] {
        let state = holding(tool);
        let (from, to) = (Pos2::new(120.0, 350.0), Pos2::new(220.0, 420.0));
        // With the box shut, the same press places the mark: the rig can see a mark.
        let mut frame = Frame::new();
        let mut shut = Search::default();
        frame.settle(&mut shut, &state);
        let control = drag(&mut frame, &mut shut, &state, (from, to));
        assert_eq!(
            added(&control),
            1,
            "{tool:?} marks with the box shut: {control:?}"
        );
        // With the box open, the press that closes it is the box's, and nothing is marked.
        let mut frame = Frame::new();
        let mut search = opened(&frame.ctx, &state);
        frame.settle(&mut search, &state);
        assert!(search.open);
        let asked = drag(&mut frame, &mut search, &state, (from, to));
        assert!(!search.open, "the press outside closed the box");
        assert_eq!(added(&asked), 0, "{tool:?} marked under the box: {asked:?}");
        // And the box being shut, the next press is the plane view's again.
        let after = drag(&mut frame, &mut search, &state, (from, to));
        assert_eq!(added(&after), 1, "{tool:?} marks again: {after:?}");
    }
}

#[test]
fn the_box_answers_the_mouse_when_it_is_opened_after_a_click_away() {
    for tool in [Tool::Marker, Tool::Ruler] {
        let state = holding(tool);
        let (from, to) = (Pos2::new(120.0, 350.0), Pos2::new(220.0, 420.0));
        let mut frame = Frame::new();
        let mut search = opened(&frame.ctx, &state);
        frame.settle(&mut search, &state);
        // The press away from the box closes it.
        drag(&mut frame, &mut search, &state, (from, to));
        assert!(!search.open, "the click away closed the box");
        frame.settle(&mut search, &state);
        // Opened again, a result is found and can be clicked, as it could the first time.
        search.open();
        frame.settle(&mut search, &state);
        frame.pass(&mut search, &state, vec![typed("ruler")]);
        let found = frame.pass(&mut search, &state, Vec::new());
        let named = |(said, _): &&(String, Rect)| said.contains("Measure a length");
        let (_, row) = found
            .seen
            .words
            .iter()
            .find(named)
            .expect("the result is drawn");
        let at = row.center();
        let pressed = frame.pass(
            &mut search,
            &state,
            vec![Event::PointerMoved(at), button(at, true)],
        );
        let released = frame.pass(&mut search, &state, vec![button(at, false)]);
        let chosen = pressed.chosen.or(released.chosen);
        assert_eq!(chosen, Some(Target::Command("measure.ruler")), "{tool:?}");
        assert!(!search.open, "choosing a result closes the box");
    }
}
