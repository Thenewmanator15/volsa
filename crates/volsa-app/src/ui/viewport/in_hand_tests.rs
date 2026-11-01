//! What the viewports do with a measuring tool in hand when the window changes round them: a
//! half drawn measurement does not wait for the slices to come back, and a double-click that
//! finishes a shape does not also fit the picture.

use egui::{Pos2, Rect, Ui, vec2};
use volsa_core::SeriesId;

use super::tools_tests::{Rig, SIZE, holding, over};
use super::{Drafts, Slices, show};
use crate::action::Action;
use crate::annotations::{Shape, Tool};
use crate::measuring::SliceValues;
use crate::state::AppState;
use crate::ui::hand::Hand;

/// Draw the viewports of `state` in `area` as the window does, with no graphics device and so
/// no picture. With no area, the arrangement shown has no slices.
fn drawn(
    ui: &mut Ui,
    area: Option<Rect>,
    state: &AppState,
    (drafts, actions): (&mut Drafts, &mut Vec<Action>),
) {
    let frame = |_: &SeriesId, _: usize| None;
    let value = |_: &SeriesId, _: usize, _: (usize, usize)| None;
    let read = |_: &SeriesId, _: usize, _: &mut dyn FnMut(SliceValues<'_>)| {};
    let slices = Slices {
        frame: &frame,
        value: &value,
        read: &read,
    };
    show(ui, area, state, &slices, (&mut [], drafts), None, actions);
}

fn whole() -> Rect {
    Rect::from_min_size(Pos2::ZERO, vec2(SIZE, SIZE))
}

#[test]
fn a_half_drawn_angle_is_given_up_while_the_slices_are_out_of_sight() {
    // Review Focus 4: the person switches the arrangement to the planes or to 3D, and back,
    // with an angle half drawn. It is not finished on return.
    let hand = Hand::new(SIZE, SIZE);
    let (first, second, third) = (over(0.0, 0.0), over(1.0, 2.0), over(3.0, 0.0));
    for between in [Some(whole()), None] {
        let state = holding(Tool::Angle);
        let mut rig = Rig::default();
        rig.click(&hand, &state, 0, first);
        rig.click(&hand, &state, 0, second);
        let mut asked = Vec::new();
        let _ = hand.pass(Vec::new(), &mut |ui| {
            drawn(ui, between, &state, (&mut rig.drafts, &mut asked));
        });
        rig.click(&hand, &state, 0, third);
        // Slices still in sight keep what was begun; out of sight, it is given up.
        let finished = matches!(rig.added()[..], [Shape::Angle { .. }]);
        assert_eq!(finished, between.is_some(), "{:?}", rig.actions);
    }
}

#[test]
fn with_a_measuring_tool_in_hand_a_double_click_does_not_fit_the_picture() {
    let at = over(1.0, 1.0);
    for (tool, fits) in [
        (Tool::Navigate, true),
        (Tool::Ruler, false),
        (Tool::Angle, false),
        (Tool::Region, false),
        (Tool::Marker, false),
    ] {
        let state = holding(tool);
        let mut drafts = Drafts::default();
        let mut actions = Vec::new();
        let mut draw = |ui: &mut Ui| drawn(ui, Some(whole()), &state, (&mut drafts, &mut actions));
        // A window of its own for each, that has seen the viewport once, so that the clicks
        // are not counted with those of the tool before.
        let hand = Hand::new(SIZE, SIZE);
        let _ = hand.settle(&mut draw);
        // Two clicks one after another are a double-click.
        hand.click(at, &mut draw);
        hand.click(at, &mut draw);
        let asked = actions.contains(&Action::Fit { viewport: 0 });
        assert_eq!(asked, fits, "{tool:?}: {actions:?}");
    }
}
