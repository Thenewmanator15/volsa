//! A drawing begun in a panel belongs to the layer the panel showed then, and a region is
//! drawn along its own half-widths whichever way the panel's run.

use volsa_core::{Affine, Patient, Point, SeriesId, Voxel};
use volsa_render::PlaneView;
use volsa_volume::{Plane, PlaneKind, plane_of};

use super::mouse::PlaneInput;
use super::shapes::{Drawn, drawn};
use super::tools::Drawing;
use crate::annotations::{Shape, Tool};

/// The axial plane through this height of voxels of 1 mm along the patient's x, y and z.
fn axial_at(height: f64) -> Plane {
    let placement: Affine<Voxel, Patient> = Affine::identity();
    let through = Point::new(5.0, 6.0, height);
    plane_of(&placement, [20, 30, 40], PlaneKind::Axial, through, true).expect("a plane")
}

fn pressed(at: Point<Patient>) -> PlaneInput {
    PlaneInput {
        pressed_at: Some(at),
        hover_at: Some(at),
        ..PlaneInput::default()
    }
}

fn let_go(at: Point<Patient>) -> PlaneInput {
    PlaneInput {
        released_at: Some(at),
        hover_at: Some(at),
        ..PlaneInput::default()
    }
}

fn hovering(at: Point<Patient>) -> PlaneInput {
    PlaneInput {
        hover_at: Some(at),
        ..PlaneInput::default()
    }
}

/// A drawing with `tool` in hand on a made-up series.
fn drawing_with(tool: Tool) -> Drawing {
    let mut drawing = Drawing::default();
    drawing.keep_up(tool, Some(&SeriesId("1.2.3".to_owned())), false);
    drawing
}

#[test]
fn an_angle_begun_on_one_layer_is_given_up_when_the_panel_shows_another() {
    let kind = PlaneKind::Axial;
    let (here, next) = (axial_at(7.0), axial_at(8.0));
    let mut drawing = drawing_with(Tool::Angle);
    assert_eq!(
        drawing.step(kind, &pressed(here.at([10.0, 0.0])), &here),
        None
    );
    assert!(drawing.so_far(kind, &here).is_some(), "an arm is placed");
    // The wheel turns: the panel shows the next layer, with the pointer where it was.
    assert_eq!(
        drawing.so_far(kind, &next),
        None,
        "nothing is drawn on a layer it was not begun on"
    );
    assert_eq!(
        drawing.step(kind, &hovering(next.at([10.0, 0.0])), &next),
        None
    );
    // The next two presses are an arm and a vertex of a new angle, not the end of the old.
    assert_eq!(
        drawing.step(kind, &pressed(next.at([0.0, 0.0])), &next),
        None
    );
    assert_eq!(
        drawing.step(kind, &pressed(next.at([0.0, 5.0])), &next),
        None
    );
    let made = drawing.step(kind, &pressed(next.at([5.0, 5.0])), &next);
    let Some(Shape::Angle { a, vertex, b }) = made else {
        panic!("{made:?}");
    };
    let heights = [a, vertex, b].map(|[_, _, z]| z);
    assert_eq!(
        heights, [8.0; 3],
        "every point on the layer it was drawn on"
    );
}

#[test]
fn a_ruler_or_a_region_held_while_the_layer_changes_is_not_finished_on_the_new_layer() {
    let kind = PlaneKind::Axial;
    let (here, next) = (axial_at(7.0), axial_at(9.0));
    for tool in [Tool::Ruler, Tool::Region] {
        let mut drawing = drawing_with(tool);
        assert_eq!(
            drawing.step(kind, &pressed(here.at([0.0, 0.0])), &here),
            None
        );
        let made = drawing.step(kind, &let_go(next.at([6.0, 4.0])), &next);
        assert_eq!(made, None, "{tool:?}");
        assert_eq!(drawing.so_far(kind, &next), None, "{tool:?}");
    }
    // On the layer it was begun on, it is finished as ever, however the panel was moved
    // within that layer meanwhile.
    let mut drawing = drawing_with(Tool::Ruler);
    assert_eq!(
        drawing.step(kind, &pressed(here.at([0.0, 0.0])), &here),
        None
    );
    let moved_aside = Plane {
        centre: here.at([12.0, -3.0]),
        ..here
    };
    let end = here.at([6.0, 4.0]);
    assert!(drawing.step(kind, &let_go(end), &moved_aside).is_some());
}

#[test]
fn a_region_is_drawn_along_its_own_half_widths_however_the_panel_is_turned() {
    let plane = axial_at(7.0);
    let view = PlaneView {
        plane,
        mm_per_px: 0.5,
        offset_mm: [0.0; 2],
    };
    let panel = [200.0, 100.0];
    let centre = plane.at([5.0, 5.0]);
    let region = |across: [f64; 3], down: [f64; 3]| Shape::Region {
        centre: [centre.x, centre.y, centre.z],
        across,
        down,
        radii_mm: [4.0, 2.0],
        stats: None,
    };
    // How far the outline reaches across and down the panel, in points.
    let reach = |shape: &Shape| {
        let Drawn::Outline(round) = drawn(shape, &view, panel) else {
            panic!("a region is an outline");
        };
        let span = |part: usize| {
            let along = round.iter().map(|at| at[part]);
            let (least, most) = along.fold((f32::MAX, f32::MIN), |(least, most), at| {
                (least.min(at), most.max(at))
            });
            (most - least).round()
        };
        [span(0), span(1)]
    };
    // Drawn in the panel itself: 4 mm across and 2 mm down are 16 and 8 points wide.
    assert_eq!(reach(&region(plane.across, plane.down)), [16.0, 8.0]);
    // Drawn on a slice whose rows run down the panel, as a scan turned in its plane is
    // stored: the same region now reaches 2 mm across the panel and 4 mm down it.
    assert_eq!(reach(&region(plane.down, plane.across)), [8.0, 16.0]);
}

#[test]
fn the_planes_say_labels_a_plugin_made_were_added_by_you() {
    let mut state = crate::state::AppState::default();
    assert_eq!(super::parts::labels_note(&state), "Labels: made elsewhere");
    let added = volsa_labels::Added {
        name: "Made-up strip".to_owned(),
        ..volsa_labels::Added::default()
    };
    state.labels.made = Some(volsa_labels::Made::default());
    assert_eq!(super::parts::labels_note(&state), "Labels: made by Volsa");
    state.labels.made = Some(volsa_labels::Made {
        added: Some(added),
        ..volsa_labels::Made::default()
    });
    assert_eq!(
        super::parts::labels_note(&state),
        "Labels: added by you, not part of Volsa"
    );
}
