//! Reading what the mouse did over a viewport.

use egui::{Event, MouseWheelUnit, PointerButton, Rect, Ui};

use super::super::overlay::ViewportInput;

/// How far a touchpad must scroll, in points, to count as one notch of a wheel.
const POINTS_PER_NOTCH: f32 = 50.0;

/// What the mouse did over a viewport this frame. `part_notch` carries what a touchpad has
/// scrolled short of a whole notch from one frame to the next.
pub(super) fn gather(
    ui: &Ui,
    response: &egui::Response,
    cell: Rect,
    window_width: f32,
    part_notch: &mut f32,
) -> ViewportInput {
    let mut input = ViewportInput {
        window_width,
        ..ViewportInput::default()
    };
    input.pressed = response.is_pointer_button_down_on() && ui.input(|i| i.pointer.any_pressed());
    input.double_clicked = response.double_clicked();
    let dragged = response.drag_delta();
    if response.dragged_by(PointerButton::Middle) {
        input.middle_drag = [dragged.x, dragged.y];
    }
    if response.dragged_by(PointerButton::Secondary) {
        input.right_drag = [dragged.x, dragged.y];
    }
    if let Some(at) = response.hover_pos() {
        let from_middle = at - cell.center();
        input.pointer = [from_middle.x, from_middle.y];
        (input.wheel, input.zooming) = turned(ui, part_notch);
    }
    input
}

/// How many whole notches the wheel turned towards the person this frame, and whether Ctrl
/// was held. `part_notch` carries what a touchpad has scrolled short of a whole notch from
/// one frame to the next.
pub(in crate::ui) fn turned(ui: &Ui, part_notch: &mut f32) -> (i32, bool) {
    let (turned, with_ctrl) = ui.input(|i| wheel(&i.events));
    *part_notch += turned;
    let whole = part_notch.trunc();
    *part_notch -= whole;
    (notches(whole), with_ctrl)
}

/// How far the wheel turned towards the person, in notches, and whether Ctrl was held.
pub(super) fn wheel(events: &[Event]) -> (f32, bool) {
    let (mut turned, mut with_ctrl) = (0.0, false);
    for event in events {
        if let Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        {
            turned -= match unit {
                MouseWheelUnit::Line => delta.y,
                MouseWheelUnit::Point => delta.y / POINTS_PER_NOTCH,
                MouseWheelUnit::Page => delta.y * 10.0,
            };
            with_ctrl |= modifiers.ctrl || modifiers.command;
        }
    }
    (turned, with_ctrl)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a whole number of notches, far inside the range"
)]
fn notches(whole: f32) -> i32 {
    whole.clamp(-1000.0, 1000.0) as i32
}

#[cfg(test)]
mod tests {
    use egui::{Modifiers, TouchPhase, vec2};

    use super::*;

    fn turn(unit: MouseWheelUnit, y: f32, modifiers: Modifiers) -> Event {
        Event::MouseWheel {
            unit,
            delta: vec2(0.0, y),
            phase: TouchPhase::Move,
            modifiers,
        }
    }

    #[test]
    fn a_wheel_turned_towards_the_person_counts_forwards() {
        let events = [turn(MouseWheelUnit::Line, -1.0, Modifiers::NONE)];
        assert_eq!(wheel(&events), (1.0, false));
        let away = [turn(MouseWheelUnit::Line, 2.0, Modifiers::CTRL)];
        assert_eq!(wheel(&away), (-2.0, true));
    }

    #[test]
    fn a_touchpad_needs_fifty_points_for_a_notch() {
        let events = [
            turn(MouseWheelUnit::Point, -25.0, Modifiers::NONE),
            turn(MouseWheelUnit::Point, -25.0, Modifiers::NONE),
            Event::PointerGone,
        ];
        assert_eq!(wheel(&events), (1.0, false));
        assert_eq!(notches(3.0), 3);
        assert_eq!(notches(-1.0e9), -1000);
    }
}
