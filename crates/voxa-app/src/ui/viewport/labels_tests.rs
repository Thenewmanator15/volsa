//! What is measured on a slice is written clear of the writing in the slice's corners and of
//! the rest of what is measured.

use egui::{Pos2, Rect, Sense, pos2, vec2};
use voxa_core::{Point, SeriesId};

use super::by_keys::Keyed;
use super::{annotate, tools};
use crate::action::Action;
use crate::annotations::Shape;
use crate::measuring::SliceValues;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::testing::{THICK_SCAN, id, shown, three_series};
use crate::ui::hand::Hand;

/// The viewport's size in these tests: the 4 x 4 slices of the 30-slice series fill it, a
/// hundred points to a pixel.
const SIZE: f32 = 400.0;

/// The 30-slice series shown in the first viewport, with four markers on the slice shown, all
/// at the middle of the pixel in its top left corner, and their names.
fn four_markers_by_a_corner() -> (AppState, Vec<String>) {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let at = placement.apply(Point::new(0.0, 0.0, 15.0));
    for _ in 0..4 {
        let marker = Shape::Marker {
            at: [at.x, at.y, at.z],
        };
        reduce(&mut state, Action::AnnotationAdd(marker));
    }
    let names = state
        .annotations
        .items
        .iter()
        .map(|item| item.label.clone());
    let names = names.collect();
    (state, names)
}

#[test]
fn measurements_on_a_slice_are_written_apart_and_clear_of_the_writing_in_its_corners() {
    let (state, names) = four_markers_by_a_corner();
    let cell = Rect::from_min_size(Pos2::ZERO, vec2(SIZE, SIZE));
    let hand = Hand::new(SIZE, SIZE);
    let mut drafts = tools::Drafts::default();
    let read = |_: &SeriesId, _: usize, _: &mut dyn FnMut(SliceValues<'_>)| {};
    let mut corners = Vec::new();
    // As the window draws a slice: its writing, and then what is measured on it, told where
    // that writing stands.
    let seen = hand.settle(&mut |ui| {
        let id = ui.id().with(("viewport", 0));
        let response = ui.interact(cell, id, Sense::click_and_drag());
        let pane = super::pane(&state, 0, cell);
        corners = annotate::overlays(ui, &pane, None);
        let _ = tools::measure(
            ui,
            &pane,
            &response,
            (&mut drafts, &read),
            (&corners, &Keyed::default()),
        );
    });
    // The rects told are where the corners' writing stands: the slice's place is in one.
    let slice = seen
        .ink_bounds("Slice 16 of 30")
        .expect("written in a corner");
    assert!(
        corners
            .iter()
            .any(|rect| rect.expand(1.5).contains_rect(slice)),
        "{slice:?} is not in {corners:?}"
    );
    let labels: Vec<Rect> = names
        .iter()
        .map(|name| seen.ink_bounds(name).expect("written"))
        .collect();
    for (one, label) in labels.iter().enumerate() {
        for corner in &corners {
            assert!(
                !label.intersects(*corner),
                "{} is over {corner:?}",
                names[one]
            );
        }
        for other in &labels[one + 1..] {
            assert!(!label.intersects(*other), "{label:?} meets {other:?}");
        }
    }
    // Every one of them was moved aside from where it would stand, over the corner's writing
    // or the label before it, and is joined to its marker.
    let from_the_marker = seen.lines.iter().filter(|line| line[0] == pos2(50.0, 50.0));
    assert_eq!(from_the_marker.count(), 4, "{:?}", seen.lines);
}
