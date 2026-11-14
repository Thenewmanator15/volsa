//! A series carried over a viewport and let go there.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Sense, pos2, vec2};
use volsa_core::SeriesId;

use super::dropped::{self, Carried};
use crate::action::Action;
use crate::ui::hand::Hand;

/// Where the made-up viewport is drawn.
fn cell() -> Rect {
    Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 200.0))
}

/// Carry a made-up series and let it go at `at`; what the viewport did.
fn let_go_at(at: Pos2) -> Vec<Action> {
    let hand = Hand::new(400.0, 400.0);
    let mut actions = Vec::new();
    let mut draw = |ui: &mut egui::Ui| {
        let response = ui.interact(cell(), ui.id().with("cell"), Sense::click_and_drag());
        actions.extend(dropped::take(ui, &response, cell(), 2));
    };
    let carried = Carried {
        series: SeriesId("made-up".to_owned()),
        name: "T1, axial".to_owned(),
    };
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let _ = hand.pass(
        vec![Event::PointerMoved(pos2(5.0, 5.0)), button(true)],
        &mut draw,
    );
    egui::DragAndDrop::set_payload(&hand.ctx, carried);
    let _ = hand.pass(vec![Event::PointerMoved(at)], &mut draw);
    let _ = hand.pass(vec![button(false)], &mut draw);
    actions
}

#[test]
fn a_series_let_go_over_a_viewport_is_shown_in_it() {
    let shown = Action::SelectSeries {
        viewport: 2,
        series: SeriesId("made-up".to_owned()),
    };
    assert_eq!(let_go_at(cell().center()), [shown]);
}

#[test]
fn a_series_let_go_elsewhere_changes_nothing() {
    assert_eq!(let_go_at(pos2(350.0, 350.0)), []);
}
