//! The box covers the window while it is open: the press that closes it, by being outside
//! the box, does not also reach what is under it.

use egui::{Context, Event, Pos2, Rect, vec2};

use super::box_tests::{opened, window};
use super::*;
use crate::annotations::Tool;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;
use crate::ui::planes::{self, PlanesView};

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

/// The window as it draws a frame: the plane views, and then the search box over them.
struct Frame {
    ctx: Context,
    view: PlanesView,
}

impl Frame {
    /// One pass, with these events: what the plane views asked for.
    fn pass(&mut self, search: &mut Search, state: &AppState, events: Vec<Event>) -> Vec<Action> {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(SCREEN[0], SCREEN[1]));
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..egui::RawInput::default()
        };
        let mut actions = Vec::new();
        let mut output = self.ctx.run_ui(input, |ui| {
            let from = (None, None, None);
            planes::show(ui, cells(), state, from, &mut self.view, &mut actions);
            let _ = search.show(ui.ctx(), state);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        actions
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
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let mut asked = frame.pass(
        search,
        state,
        vec![Event::PointerMoved(from), button(from, true)],
    );
    asked.extend(frame.pass(search, state, vec![Event::PointerMoved(to)]));
    asked.extend(frame.pass(search, state, vec![button(to, false)]));
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
        let mut frame = Frame {
            ctx: window(),
            view: PlanesView::new(None),
        };
        let mut shut = Search::default();
        for _ in 0..2 {
            frame.pass(&mut shut, &state, Vec::new());
        }
        let control = drag(&mut frame, &mut shut, &state, (from, to));
        assert_eq!(
            added(&control),
            1,
            "{tool:?} marks with the box shut: {control:?}"
        );
        // With the box open, the press that closes it is the box's, and nothing is marked.
        let mut frame = Frame {
            ctx: window(),
            view: PlanesView::new(None),
        };
        let mut search = opened(&frame.ctx, &state);
        for _ in 0..2 {
            frame.pass(&mut search, &state, Vec::new());
        }
        assert!(search.open);
        let asked = drag(&mut frame, &mut search, &state, (from, to));
        assert!(!search.open, "the press outside closed the box");
        assert_eq!(added(&asked), 0, "{tool:?} marked under the box: {asked:?}");
        // And the box being shut, the next press is the plane view's again.
        let after = drag(&mut frame, &mut search, &state, (from, to));
        assert_eq!(added(&after), 1, "{tool:?} marks again: {after:?}");
    }
}
