//! What the mouse does over a plane view.

use egui::{PointerButton, Rect, Ui};
use voxa_core::{Patient, Point};
use voxa_render::PlaneView;
use voxa_volume::PlaneKind;

use super::super::overlay::{WINDOW_PER_PIXEL, ZOOM_PER_NOTCH};
use super::super::viewport::wheel_turned;
use crate::action::Action;

/// What the mouse did over a plane's panel this frame, in the plane's own terms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaneInput {
    /// The point of the patient under the pointer while the main button is held on the panel.
    pub held_at: Option<Point<Patient>>,
    /// The point under the pointer when the main button went down on the panel this frame.
    pub pressed_at: Option<Point<Patient>>,
    /// The point under the pointer when the main button came up this frame, if it had gone
    /// down on the panel or the pointer is over it.
    pub released_at: Option<Point<Patient>>,
    /// The point under the pointer while it is over the panel or dragging from it.
    pub hover_at: Option<Point<Patient>>,
    /// Wheel notches turned: positive towards the person, which moves on to later layers.
    pub wheel: i32,
    /// Whether Ctrl was held as the wheel turned, which zooms instead.
    pub zooming: bool,
    /// Where on the plane the pointer is, in millimetres across and down from its centre.
    pub pointer_mm: [f64; 2],
    /// How far the pointer moved over the plane with the middle button held, in millimetres.
    pub middle_drag_mm: [f64; 2],
    /// How far the pointer moved with the right button held, in points.
    pub right_drag: [f32; 2],
    /// Whether the main button was double-clicked.
    pub double_clicked: bool,
    /// The width of the window in use, which sets how fast a drag changes it.
    pub window_width: f32,
}

/// What the mouse asks for over a plane: the main button moves the position, the wheel moves
/// through the layers (or zooms about the pointer with Ctrl), a middle drag moves the
/// picture, a right drag changes the window of the active viewport, which the planes are
/// shown through, and a double-click shows the whole plane again.
pub fn plane_input_to_actions(kind: PlaneKind, active: usize, input: &PlaneInput) -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(at) = input.held_at {
        actions.push(Action::SetPosition(at));
    }
    if input.double_clicked {
        actions.push(Action::PlaneFit(kind));
    }
    if input.wheel != 0 && input.zooming {
        // Turning the wheel away magnifies.
        let factor = f64::from(ZOOM_PER_NOTCH).powi(input.wheel.saturating_neg());
        let about_mm = input.pointer_mm;
        actions.push(Action::PlaneZoom {
            kind,
            factor,
            about_mm,
        });
    } else if input.wheel != 0 {
        let steps = input.wheel;
        actions.push(Action::PlaneScroll { kind, steps });
    }
    let [across, down] = input.middle_drag_mm;
    if across != 0.0 || down != 0.0 {
        // The picture follows the pointer: its middle then shows what was to the other side.
        let by_mm = [-across, -down];
        actions.push(Action::PlanePan { kind, by_mm });
    }
    let [across, down] = input.right_drag;
    if across != 0.0 || down != 0.0 {
        let per_pixel = input.window_width * WINDOW_PER_PIXEL;
        actions.push(Action::AdjustWindow {
            viewport: active,
            d_center: down * per_pixel,
            d_width: across * per_pixel,
        });
    }
    actions
}

/// What the mouse did over a plane's panel this frame. `part_notch` carries what a touchpad
/// has scrolled short of a whole notch from one frame to the next.
pub(super) fn gather(
    ui: &Ui,
    response: &egui::Response,
    cell: Rect,
    view: &PlaneView,
    part_notch: &mut f32,
) -> PlaneInput {
    let size = [cell.width(), cell.height()];
    let in_panel = |at: egui::Pos2| [at.x - cell.left(), at.y - cell.top()];
    let mut input = PlaneInput::default();
    let patient = |at: egui::Pos2| view.screen_to_patient(size, in_panel(at));
    let (main_down, went_down, came_up, latest) = ui.input(|i| {
        let pointer = &i.pointer;
        let main = PointerButton::Primary;
        (
            pointer.button_down(main),
            pointer.button_pressed(main),
            pointer.button_released(main),
            pointer.latest_pos(),
        )
    });
    let on_panel = response.is_pointer_button_down_on();
    if on_panel && main_down {
        input.held_at = response.interact_pointer_pos().map(patient);
    }
    if on_panel && went_down {
        input.pressed_at = response.interact_pointer_pos().map(patient);
    }
    let over = response.contains_pointer();
    if came_up && (over || response.drag_stopped()) {
        input.released_at = latest.map(patient);
    }
    if over || response.dragged() {
        input.hover_at = latest.map(patient);
    }
    input.double_clicked = response.double_clicked();
    let dragged = response.drag_delta();
    if response.dragged_by(PointerButton::Middle) {
        let mm = |points: f32| f64::from(points) * view.mm_per_px;
        input.middle_drag_mm = [mm(dragged.x), mm(dragged.y)];
    }
    if response.dragged_by(PointerButton::Secondary) {
        input.right_drag = [dragged.x, dragged.y];
    }
    if let Some(at) = response.hover_pos() {
        input.pointer_mm = view.on_plane(size, in_panel(at));
        (input.wheel, input.zooming) = wheel_turned(ui, part_notch);
    }
    input
}
