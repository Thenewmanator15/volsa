//! What the mouse asks of the 3D scene.

use egui::{PointerButton, Rect, Ui};
use voxa_core::{Patient, Point};

use super::super::overlay::{WINDOW_PER_PIXEL, ZOOM_PER_NOTCH};
use super::super::viewport;
use crate::action::Action;

/// What the mouse did over the scene since the last frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SceneInput {
    /// How far the pointer moved with the main button held.
    pub left_drag: [f32; 2],
    /// How far the pointer moved with the middle button held.
    pub middle_drag: [f32; 2],
    /// How far the pointer moved with the right button held.
    pub right_drag: [f32; 2],
    /// Whether Shift was held, which makes a drag with the main button move the scene.
    pub shift: bool,
    /// Wheel notches turned: positive towards the person.
    pub wheel: i32,
    /// Where the pointer is, in screen pixels from the middle of the scene.
    pub pointer: [f32; 2],
    /// Whether any button went down.
    pub pressed: bool,
    /// The point of the patient under the pointer when the main button was double-clicked
    /// on something drawn.
    pub double_clicked_surface: Option<Point<Patient>>,
    /// The width of the window in use, which sets how fast a drag changes it.
    pub window_width: f32,
}

/// What the mouse asks for. What the 2D viewport already does is done the same way here: a
/// drag with the middle button moves the picture, and a drag with the right button changes
/// the window of the active viewport, across for width and down for centre. New here: a drag
/// with the main button turns the scene (or moves it, with Shift), the wheel zooms about the
/// pointer, turning it away magnifying, and a double-click on something drawn puts that
/// point in the middle. What a single click does depends on the tool in hand: see
/// [`clicked`](super::tools::clicked).
pub fn scene_input_to_actions(active: usize, input: &SceneInput) -> Vec<Action> {
    let mut actions = Vec::new();
    let moved = |[x, y]: [f32; 2]| x.abs() > 0.0 || y.abs() > 0.0;
    if moved(input.left_drag) && input.shift {
        actions.push(Action::ScenePan {
            by: input.left_drag,
        });
    } else if moved(input.left_drag) {
        actions.push(Action::SceneOrbit {
            by: input.left_drag,
        });
    }
    if moved(input.middle_drag) {
        actions.push(Action::ScenePan {
            by: input.middle_drag,
        });
    }
    if input.wheel != 0 {
        let factor = ZOOM_PER_NOTCH.powi(input.wheel.saturating_neg());
        actions.push(Action::SceneZoom {
            factor,
            about: input.pointer,
        });
    }
    if moved(input.right_drag) {
        let per_pixel = input.window_width * WINDOW_PER_PIXEL;
        let [across, down] = input.right_drag;
        let (d_center, d_width) = (down * per_pixel, across * per_pixel);
        actions.push(Action::AdjustWindow {
            viewport: active,
            d_center,
            d_width,
        });
    }
    if let Some(point) = input.double_clicked_surface {
        actions.push(Action::SceneCentre(point));
    }
    actions
}

/// What the mouse did over the scene this frame.
pub(super) fn gather(
    ui: &Ui,
    response: &egui::Response,
    area: Rect,
    pointed_at: Option<Point<Patient>>,
    part_notch: &mut f32,
) -> SceneInput {
    let dragged = response.drag_delta();
    let dragged_with = |button| {
        if response.dragged_by(button) {
            [dragged.x, dragged.y]
        } else {
            [0.0; 2]
        }
    };
    let mut input = SceneInput {
        left_drag: dragged_with(PointerButton::Primary),
        middle_drag: dragged_with(PointerButton::Middle),
        right_drag: dragged_with(PointerButton::Secondary),
        shift: ui.input(|i| i.modifiers.shift),
        pressed: response.is_pointer_button_down_on() && ui.input(|i| i.pointer.any_pressed()),
        double_clicked_surface: pointed_at,
        ..SceneInput::default()
    };
    if let Some(at) = response.hover_pos() {
        let from_middle = at - area.center();
        input.pointer = [from_middle.x, from_middle.y];
        (input.wheel, _) = viewport::wheel_turned(ui, part_notch);
    }
    input
}

#[cfg(test)]
mod tests {
    use super::super::super::overlay::{ViewportInput, input_to_actions};
    use super::*;

    fn asked(input: SceneInput) -> Vec<Action> {
        scene_input_to_actions(2, &input)
    }

    #[test]
    fn a_drag_with_the_main_button_turns_the_scene() {
        let drag = SceneInput {
            left_drag: [4.0, -2.0],
            ..SceneInput::default()
        };
        assert_eq!(asked(drag), [Action::SceneOrbit { by: [4.0, -2.0] }]);
    }

    #[test]
    fn with_shift_or_the_middle_button_a_drag_moves_it() {
        let with_shift = SceneInput {
            left_drag: [4.0, -2.0],
            shift: true,
            ..SceneInput::default()
        };
        assert_eq!(asked(with_shift), [Action::ScenePan { by: [4.0, -2.0] }]);
        let middle = SceneInput {
            middle_drag: [1.0, 3.0],
            ..SceneInput::default()
        };
        assert_eq!(asked(middle), [Action::ScenePan { by: [1.0, 3.0] }]);
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_and_away_magnifies() {
        let away = SceneInput {
            wheel: -2,
            pointer: [30.0, -10.0],
            ..SceneInput::default()
        };
        let [Action::SceneZoom { factor, about }] = asked(away)[..] else {
            panic!("{:?}", asked(away));
        };
        assert!((factor - 1.21).abs() < 1e-5, "{factor}");
        assert_eq!(about, [30.0, -10.0]);
        let towards = SceneInput {
            wheel: 1,
            ..SceneInput::default()
        };
        let [Action::SceneZoom { factor, .. }] = asked(towards)[..] else {
            panic!("{:?}", asked(towards));
        };
        assert!(factor < 1.0, "{factor}");
    }

    #[test]
    fn a_right_drag_changes_the_window_exactly_as_in_the_2d_viewport() {
        let drag = SceneInput {
            right_drag: [10.0, 0.0],
            window_width: 256.0,
            ..SceneInput::default()
        };
        let (viewport, d_center, d_width) = (2, 0.0, 10.0);
        assert_eq!(
            asked(drag),
            [Action::AdjustWindow {
                viewport,
                d_center,
                d_width
            }]
        );
        let slanted = [7.0, -3.0];
        let here = SceneInput {
            right_drag: slanted,
            window_width: 900.0,
            ..SceneInput::default()
        };
        let there = ViewportInput {
            right_drag: slanted,
            window_width: 900.0,
            ..ViewportInput::default()
        };
        assert_eq!(asked(here), input_to_actions(2, &there));
    }

    #[test]
    fn a_double_click_on_a_surface_centres_on_it_and_on_nothing_does_nothing() {
        let point = Point::new(1.0, 2.0, 3.0);
        let on_it = SceneInput {
            double_clicked_surface: Some(point),
            ..SceneInput::default()
        };
        assert_eq!(asked(on_it), [Action::SceneCentre(point)]);
        let only_pressed = SceneInput {
            pressed: true,
            ..SceneInput::default()
        };
        assert_eq!(asked(only_pressed), []);
    }
}
