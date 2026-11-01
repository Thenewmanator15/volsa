//! Drawing annotations in a plane view: what the pointer makes with each tool, where an
//! annotation is shown, and how it is drawn.

use voxa_core::{Affine, Patient, Point, Voxel};
use voxa_render::PlaneView;
use voxa_volume::{Plane, PlaneKind, plane_of};

use super::shapes::{Drawn, beside, drawn, label_bottom, label_left, shown_on};
use super::tools::{Draft, Pointer, drafted, preview};
use crate::annotations::{Annotation, Shape, Tool};

/// The axial plane through (5, 6, 7) of voxels of 1 mm along the patient's x, y and z.
fn axial() -> Plane {
    let placement: Affine<Voxel, Patient> = Affine::identity();
    let through = Point::new(5.0, 6.0, 7.0);
    plane_of(&placement, [20, 30, 40], PlaneKind::Axial, through, true).expect("a plane")
}

/// The point of the plane this far across and down from its middle.
fn at(across: f64, down: f64) -> Point<Patient> {
    axial().at([across, down])
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// What the pointer makes, one thing after another, starting with nothing begun.
fn after(tool: Tool, pointer: &[Pointer]) -> (Draft, Vec<Shape>) {
    let mut made = Vec::new();
    let mut draft = Draft::Idle;
    for event in pointer {
        let (next, shape) = drafted(tool, draft, *event, &axial());
        draft = next;
        made.extend(shape);
    }
    (draft, made)
}

#[test]
fn a_ruler_is_drawn_by_pressing_at_one_end_and_letting_go_at_the_other() {
    let (a, b) = (at(0.0, 0.0), at(3.0, 4.0));
    let (draft, made) = after(Tool::Ruler, &[Pointer::Pressed(a)]);
    assert_eq!((draft, made), (Draft::RulerFrom(a), vec![]));
    let whole = [
        Pointer::Pressed(a),
        Pointer::Moved(at(1.0, 1.0)),
        Pointer::Released(b),
    ];
    let (draft, made) = after(Tool::Ruler, &whole);
    let ruler = Shape::Ruler {
        a: xyz(a),
        b: xyz(b),
    };
    assert_eq!((draft, made), (Draft::Idle, vec![ruler]));
    // Let go less than a millimetre away, it was a click and not a ruler.
    let (draft, made) = after(
        Tool::Ruler,
        &[Pointer::Pressed(a), Pointer::Released(at(0.4, 0.0))],
    );
    assert_eq!((draft, made), (Draft::Idle, vec![]));
    let (_, made) = after(Tool::Ruler, &[Pointer::Released(b)]);
    assert_eq!(made, [], "letting go with nothing begun makes nothing");
}

#[test]
fn an_angle_is_drawn_with_three_clicks_an_arm_the_vertex_and_the_other_arm() {
    let (a, vertex, b) = (at(5.0, 0.0), at(0.0, 0.0), at(0.0, 5.0));
    let clicks = [
        Pointer::Pressed(a),
        Pointer::Released(a),
        Pointer::Pressed(vertex),
    ];
    let (draft, made) = after(Tool::Angle, &clicks);
    assert_eq!((draft, made), (Draft::AngleVertex(a, vertex), vec![]));
    let all = [
        Pointer::Pressed(a),
        Pointer::Pressed(vertex),
        Pointer::Pressed(b),
    ];
    let (draft, made) = after(Tool::Angle, &all);
    let angle = Shape::Angle {
        a: xyz(a),
        vertex: xyz(vertex),
        b: xyz(b),
    };
    assert_eq!((draft, made), (Draft::Idle, vec![angle]));
}

#[test]
fn a_region_is_drawn_by_dragging_from_its_middle_to_a_corner_of_the_box_round_it() {
    let (middle, corner) = (at(2.0, 1.0), at(6.0, -1.0));
    let (draft, made) = after(
        Tool::Region,
        &[Pointer::Pressed(middle), Pointer::Released(corner)],
    );
    let plane = axial();
    let region = Shape::Region {
        centre: xyz(middle),
        across: plane.across,
        down: plane.down,
        radii_mm: [4.0, 2.0],
        stats: None,
    };
    assert_eq!((draft, made), (Draft::Idle, vec![region]));
    let thin = [Pointer::Pressed(middle), Pointer::Released(at(6.0, 1.2))];
    assert_eq!(
        after(Tool::Region, &thin).1,
        [],
        "a radius under half a millimetre is nothing"
    );
}

#[test]
fn a_marker_is_placed_with_one_click_and_the_navigating_tool_draws_nothing() {
    let here = at(1.0, 1.0);
    let (draft, made) = after(Tool::Marker, &[Pointer::Pressed(here)]);
    assert_eq!(
        (draft, made),
        (Draft::Idle, vec![Shape::Marker { at: xyz(here) }])
    );
    let all = [
        Pointer::Pressed(here),
        Pointer::Moved(here),
        Pointer::Released(at(9.0, 9.0)),
    ];
    assert_eq!(after(Tool::Navigate, &all), (Draft::Idle, vec![]));
}

#[test]
fn what_is_begun_is_given_up_when_the_tool_or_the_series_changes_or_escape_is_pressed() {
    use voxa_core::SeriesId;

    use super::mouse::PlaneInput;
    use super::tools::Drawing;
    let plane = axial();
    let kind = PlaneKind::Axial;
    let pressed = |at: Point<Patient>| PlaneInput {
        pressed_at: Some(at),
        hover_at: Some(at),
        ..PlaneInput::default()
    };
    let let_go = |at: Point<Patient>| PlaneInput {
        released_at: Some(at),
        hover_at: Some(at),
        ..PlaneInput::default()
    };
    let series = SeriesId("1.2.3".to_owned());
    let mut drawing = Drawing::default();
    let begin = |drawing: &mut Drawing| {
        drawing.keep_up(Tool::Ruler, Some(&series), false);
        assert_eq!(drawing.step(kind, &pressed(at(0.0, 0.0)), &plane), None);
        assert!(
            drawing.so_far(kind, &plane).is_some(),
            "a ruler has been begun"
        );
    };
    begin(&mut drawing);
    assert_eq!(
        drawing.so_far(PlaneKind::Coronal, &plane),
        None,
        "only in its own panel"
    );
    drawing.keep_up(Tool::Ruler, Some(&series), false);
    assert!(
        drawing.so_far(kind, &plane).is_some(),
        "nothing changed, nothing is given up"
    );
    drawing.keep_up(Tool::Ruler, Some(&series), true);
    assert_eq!(drawing.so_far(kind, &plane), None, "Escape");
    begin(&mut drawing);
    drawing.keep_up(Tool::Angle, Some(&series), false);
    assert_eq!(drawing.so_far(kind, &plane), None, "another tool");
    begin(&mut drawing);
    drawing.keep_up(Tool::Ruler, None, false);
    assert_eq!(drawing.so_far(kind, &plane), None, "another series");
    begin(&mut drawing);
    let made = drawing.step(kind, &let_go(at(3.0, 4.0)), &plane);
    assert!(matches!(made, Some(Shape::Ruler { .. })), "{made:?}");
    assert_eq!(drawing.so_far(kind, &plane), None, "finished");
    // Let go over another panel, a ruler begun here is not finished there.
    begin(&mut drawing);
    assert_eq!(
        drawing.step(PlaneKind::Sagittal, &let_go(at(3.0, 4.0)), &plane),
        None
    );
}

#[test]
fn what_is_begun_is_shown_as_it_would_be_if_finished_here() {
    let (a, vertex, now) = (at(5.0, 0.0), at(0.0, 0.0), at(0.0, 5.0));
    let line = Shape::Ruler {
        a: xyz(a),
        b: xyz(now),
    };
    assert_eq!(
        preview(Draft::RulerFrom(a), now, &axial()),
        Some(line.clone())
    );
    assert_eq!(
        preview(Draft::AngleArm(a), now, &axial()),
        Some(line),
        "the first arm so far"
    );
    let angle = Shape::Angle {
        a: xyz(a),
        vertex: xyz(vertex),
        b: xyz(now),
    };
    assert_eq!(
        preview(Draft::AngleVertex(a, vertex), now, &axial()),
        Some(angle)
    );
    let Some(Shape::Region { radii_mm, .. }) = preview(Draft::RegionFrom(a), now, &axial()) else {
        panic!("a region so far");
    };
    assert_eq!(radii_mm, [5.0, 5.0]);
    assert_eq!(preview(Draft::Idle, now, &axial()), None);
}

#[test]
fn an_annotation_is_shown_on_a_plane_only_when_it_lies_in_that_planes_layer() {
    let plane = axial();
    let lifted = |by: f64| {
        let on = at(3.0, 0.0);
        [on.x, on.y, on.z + by]
    };
    let ruler = |by: f64| Shape::Ruler {
        a: xyz(at(0.0, 0.0)),
        b: lifted(by),
    };
    assert!(shown_on(&ruler(0.0), &plane, 0.5));
    assert!(
        !shown_on(&ruler(2.0), &plane, 0.5),
        "one end is two millimetres off the plane"
    );
    assert!(
        shown_on(&ruler(2.0), &plane, 3.25),
        "within half of a thick layer"
    );
    assert!(shown_on(&Shape::Marker { at: lifted(0.4) }, &plane, 0.5));
    assert!(!shown_on(&Shape::Marker { at: lifted(-0.6) }, &plane, 0.5));
    let region = |across, down| Shape::Region {
        centre: xyz(at(0.0, 0.0)),
        across,
        down,
        radii_mm: [4.0, 2.0],
        stats: None,
    };
    assert!(shown_on(&region(plane.across, plane.down), &plane, 0.5));
    let upright = region(plane.across, plane.normal());
    assert!(
        !shown_on(&upright, &plane, 0.5),
        "a region drawn in another plane is not in this one"
    );
}

#[test]
fn an_annotation_is_drawn_where_its_points_fall_in_the_panel() {
    let plane = axial();
    let view = PlaneView {
        plane,
        mm_per_px: 0.5,
        offset_mm: [0.0; 2],
    };
    let panel = [200.0, 100.0];
    let ruler = Shape::Ruler {
        a: xyz(at(0.0, 0.0)),
        b: xyz(at(10.0, 0.0)),
    };
    assert_eq!(
        drawn(&ruler, &view, panel),
        Drawn::Line([[100.0, 50.0], [120.0, 50.0]])
    );
    let angle = Shape::Angle {
        a: xyz(at(10.0, 0.0)),
        vertex: xyz(at(0.0, 0.0)),
        b: xyz(at(0.0, 10.0)),
    };
    let arms = [[120.0, 50.0], [100.0, 50.0], [100.0, 70.0]];
    assert_eq!(
        drawn(&angle, &view, panel),
        Drawn::Arms(arms),
        "arm, vertex, arm"
    );
    let region = Shape::Region {
        centre: xyz(at(5.0, 5.0)),
        across: plane.across,
        down: plane.down,
        radii_mm: [4.0, 2.0],
        stats: None,
    };
    let ellipse = Drawn::Ellipse {
        centre: [110.0, 60.0],
        radii: [8.0, 4.0],
    };
    assert_eq!(drawn(&region, &view, panel), ellipse);
    let marker = Shape::Marker {
        at: xyz(at(-5.0, 0.0)),
    };
    assert_eq!(drawn(&marker, &view, panel), Drawn::Dot([90.0, 50.0]));
}

#[test]
fn beside_a_marker_is_its_name_and_beside_the_rest_the_name_and_what_was_measured() {
    let named = |label: &str, shape| Annotation {
        id: 1,
        label: label.to_owned(),
        shape,
        scale: crate::measuring::Scale::Patient,
    };
    let marker = named(
        "Marker 4",
        Shape::Marker {
            at: [1.0, 2.0, 3.0],
        },
    );
    assert_eq!(beside(&marker, "L 1.0, P 2.0, S 3.0 mm"), "Marker 4");
    let ruler = Shape::Ruler {
        a: [0.0; 3],
        b: [3.0, 4.0, 0.0],
    };
    assert_eq!(
        beside(&named("Ruler 1", ruler), "5.0 mm"),
        "Ruler 1: 5.0 mm"
    );
}

#[test]
fn words_beside_an_annotation_are_kept_inside_the_panel() {
    let panel = (4.0, 200.0);
    assert_eq!(
        label_left(100.0, 50.0, panel),
        100.0,
        "there is room: where they belong"
    );
    assert_eq!(
        label_left(180.0, 50.0, panel),
        150.0,
        "moved left to end at the edge"
    );
    assert_eq!(label_left(-20.0, 50.0, panel), 4.0, "nor off the left");
    assert_eq!(
        label_left(90.0, 300.0, panel),
        4.0,
        "longer than the panel: from its left"
    );
}

#[test]
fn words_beside_an_annotation_keep_clear_of_what_is_written_along_the_top() {
    // A panel whose top is at 100, with 26 points along it for its caption; words 14 high.
    assert_eq!(
        label_bottom(200.0, 14.0, 126.0),
        200.0,
        "lower down they are left alone"
    );
    assert_eq!(
        label_bottom(120.0, 14.0, 126.0),
        140.0,
        "moved down to begin under it"
    );
    assert_eq!(label_bottom(140.0, 14.0, 126.0), 140.0);
}

#[test]
fn with_a_measuring_tool_in_hand_a_double_click_on_a_panel_does_not_fit_it() {
    use super::{PlaneInput, handled};
    use crate::action::Action;
    use crate::state::AppState;
    let twice = PlaneInput {
        double_clicked: true,
        window_width: 256.0,
        ..PlaneInput::default()
    };
    let kind = PlaneKind::Axial;
    for (tool, fits) in [
        (Tool::Navigate, true),
        (Tool::Ruler, false),
        (Tool::Angle, false),
        (Tool::Region, false),
        (Tool::Marker, false),
    ] {
        let state = AppState {
            tool,
            ..AppState::default()
        };
        let mut drawing = super::tools::Drawing::default();
        let asked = handled(&state, kind, &twice, (&axial(), None), &mut drawing);
        assert_eq!(asked.contains(&Action::PlaneFit(kind)), fits, "{tool:?}");
    }
}
