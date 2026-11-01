//! What is written over a plane view, and what the mouse does to it.

use voxa_core::{Affine, Patient, Point, Voxel};
use voxa_render::PlaneView;
use voxa_volume::{Plane, PlaneKind, plane_of};

use super::mouse::{PlaneInput, plane_input_to_actions};
use super::overlay::{
    Room, crosshair, crossing, interpolated_note, panel_note, plane_caption, plane_letters,
    value_under,
};
use crate::action::Action;

/// Voxels of 1 mm along the patient's x, y and z, 20 by 30 by 40 of them.
fn straight() -> (Affine<Voxel, Patient>, [u32; 3]) {
    let placement =
        Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    (placement, [20, 30, 40])
}

fn plane(kind: PlaneKind, through: [f64; 3], radiological: bool) -> Plane {
    let (placement, dims) = straight();
    let [x, y, z] = through;
    plane_of(&placement, dims, kind, Point::new(x, y, z), radiological).expect("a plane")
}

fn caption(kind: PlaneKind, through: [f64; 3]) -> String {
    let [x, y, z] = through;
    plane_caption(kind, &plane(kind, through, true), Point::new(x, y, z))
}

#[test]
fn a_plane_is_captioned_with_its_name_and_where_it_lies_from_the_scanners_origin() {
    assert_eq!(
        caption(PlaneKind::Axial, [5.0, 6.0, 7.0]),
        "Axial · S 7.0 mm"
    );
    assert_eq!(
        caption(PlaneKind::Axial, [5.0, 6.0, -3.3]),
        "Axial · I 3.3 mm"
    );
    assert_eq!(
        caption(PlaneKind::Coronal, [5.0, -6.0, 7.0]),
        "Coronal · A 6.0 mm"
    );
    assert_eq!(
        caption(PlaneKind::Sagittal, [5.0, 6.0, 7.0]),
        "Sagittal · L 5.0 mm"
    );
    assert_eq!(
        caption(PlaneKind::Sagittal, [-5.0, 6.0, 7.0]),
        "Sagittal · R 5.0 mm"
    );
    // Which way the picture faces does not change where the plane is.
    let through = Point::new(5.0, 6.0, 7.0);
    let other_way = plane(PlaneKind::Axial, [5.0, 6.0, 7.0], false);
    assert_eq!(
        plane_caption(PlaneKind::Axial, &other_way, through),
        "Axial · S 7.0 mm"
    );
}

#[test]
fn the_edges_of_each_plane_are_lettered_as_radiology_has_them() {
    let edges = |kind, radiological| {
        let labels = plane_letters(&plane(kind, [5.0, 6.0, 7.0], radiological));
        [labels.left, labels.right, labels.top, labels.bottom].join("")
    };
    assert_eq!(edges(PlaneKind::Axial, true), "RLAP");
    assert_eq!(edges(PlaneKind::Axial, false), "LRAP");
    assert_eq!(edges(PlaneKind::Coronal, true), "RLSI");
    assert_eq!(edges(PlaneKind::Coronal, false), "LRSI");
    assert_eq!(edges(PlaneKind::Sagittal, true), "APSI");
    assert_eq!(edges(PlaneKind::Sagittal, false), "APSI");
}

#[test]
fn each_of_the_other_two_planes_is_a_line_across_a_panel() {
    assert_eq!(
        crossing(PlaneKind::Axial),
        (PlaneKind::Coronal, PlaneKind::Sagittal)
    );
    assert_eq!(
        crossing(PlaneKind::Coronal),
        (PlaneKind::Axial, PlaneKind::Sagittal)
    );
    assert_eq!(
        crossing(PlaneKind::Sagittal),
        (PlaneKind::Axial, PlaneKind::Coronal)
    );
}

#[test]
fn the_cross_through_the_position_leaves_the_position_itself_clear() {
    let axial = plane(PlaneKind::Axial, [5.0, 6.0, 7.0], true);
    let view = PlaneView {
        plane: axial,
        mm_per_px: 0.5,
        offset_mm: [0.0; 2],
    };
    let panel = [200.0, 100.0];
    // 10 mm to the side of the middle and 5 mm below it: 20 points right, 10 down.
    let none = Room::default();
    let cross = crosshair(&view, panel, axial.at([10.0, 5.0]), none);
    assert_eq!(
        cross.level,
        [[[0.0, 60.0], [114.0, 60.0]], [[126.0, 60.0], [200.0, 60.0]]]
    );
    assert_eq!(
        cross.upright,
        [
            [[120.0, 0.0], [120.0, 54.0]],
            [[120.0, 66.0], [120.0, 100.0]]
        ]
    );
    // A position past the side of the panel still shows how high it is, as one line.
    let off_the_side = crosshair(&view, panel, axial.at([80.0, 5.0]), none);
    assert_eq!(off_the_side.level, [[[0.0, 60.0], [200.0, 60.0]]]);
    assert_eq!(off_the_side.upright, [] as [[[f32; 2]; 2]; 0]);
    let nowhere = crosshair(&view, panel, axial.at([800.0, 500.0]), none);
    assert!(nowhere.level.is_empty() && nowhere.upright.is_empty());
    // Room is left at the edges for what is written there: the letters, the scale bar.
    let room = Room {
        sides: 26.0,
        top: 26.0,
        bottom: 48.0,
    };
    // In a panel 200 points high the same position is 20 right of the middle and 10 below.
    let tall = [200.0, 200.0];
    let kept_in = crosshair(&view, tall, axial.at([10.0, 5.0]), room);
    assert_eq!(
        kept_in.level,
        [
            [[26.0, 110.0], [114.0, 110.0]],
            [[126.0, 110.0], [174.0, 110.0]]
        ]
    );
    assert_eq!(
        kept_in.upright,
        [
            [[120.0, 26.0], [120.0, 104.0]],
            [[120.0, 116.0], [120.0, 152.0]]
        ]
    );
    let by_the_bar = crosshair(&view, tall, axial.at([10.0, 40.0]), room);
    assert!(
        by_the_bar.level.is_empty(),
        "a line through the scale bar is not drawn"
    );
    assert_eq!(by_the_bar.upright, [[[120.0, 26.0], [120.0, 152.0]]]);
}

#[test]
fn a_picture_across_coarse_layers_says_that_it_is_interpolated() {
    let note = interpolated_note([0.5, 0.5, 3.0], 0.5, 3.0).expect("a note");
    assert_eq!(note, "Interpolated: layers 3.0 mm apart");
    assert_eq!(
        interpolated_note([0.5, 0.5, 3.0], 0.5, 0.5),
        None,
        "the scan's own plane"
    );
    assert_eq!(
        interpolated_note([1.0, 1.0, 1.2], 1.0, 1.2),
        None,
        "near enough the same"
    );
}

const KIND: PlaneKind = PlaneKind::Coronal;

/// What the mouse asks for over the coronal plane while the third viewport is the active one.
fn asked(input: &PlaneInput) -> Vec<Action> {
    plane_input_to_actions(KIND, 2, input)
}

fn quiet() -> PlaneInput {
    PlaneInput {
        window_width: 256.0,
        ..PlaneInput::default()
    }
}

#[test]
fn the_main_button_moves_the_position_and_the_wheel_moves_through_the_layers() {
    assert_eq!(asked(&quiet()), []);
    let at = Point::new(1.0, 2.0, 3.0);
    let held = PlaneInput {
        held_at: Some(at),
        ..quiet()
    };
    assert_eq!(asked(&held), [Action::SetPosition(at)]);
    let wheel = |wheel, zooming| PlaneInput {
        wheel,
        zooming,
        pointer_mm: [4.0, -2.0],
        ..quiet()
    };
    let kind = KIND;
    // Towards the person is on to later layers, as in the 2D viewport.
    assert_eq!(
        asked(&wheel(1, false)),
        [Action::PlaneScroll { kind, steps: 1 }]
    );
    assert_eq!(
        asked(&wheel(-3, false)),
        [Action::PlaneScroll { kind, steps: -3 }]
    );
    let [
        Action::PlaneZoom {
            factor, about_mm, ..
        },
    ] = asked(&wheel(-1, true))[..]
    else {
        panic!("Ctrl with the wheel zooms");
    };
    assert!(
        (factor - 1.1).abs() < 1.0e-6 && about_mm == [4.0, -2.0],
        "away magnifies: {factor}"
    );
}

#[test]
fn the_other_buttons_move_the_picture_and_change_the_window() {
    let kind = KIND;
    let dragged = PlaneInput {
        middle_drag_mm: [3.0, 1.0],
        ..quiet()
    };
    // The picture follows the pointer, so its middle shows what was to the other side.
    assert_eq!(
        asked(&dragged),
        [Action::PlanePan {
            kind,
            by_mm: [-3.0, -1.0]
        }]
    );
    let window = PlaneInput {
        right_drag: [8.0, -4.0],
        ..quiet()
    };
    let expected = Action::AdjustWindow {
        viewport: 2,
        d_center: -4.0,
        d_width: 8.0,
    };
    assert_eq!(
        asked(&window),
        [expected],
        "the window of the active viewport"
    );
    let twice = PlaneInput {
        double_clicked: true,
        ..quiet()
    };
    assert_eq!(asked(&twice), [Action::PlaneFit(kind)]);
}

#[test]
fn a_smoothed_picture_says_so_in_few_words() {
    let coarse = interpolated_note([0.5, 0.5, 3.0], 0.5, 3.0);
    assert_eq!(
        panel_note(coarse.clone(), false),
        coarse,
        "plain, it is as it was"
    );
    let both = panel_note(coarse, true);
    let said = "Interpolated and smoothed: layers 3.0 mm apart";
    assert_eq!(both.as_deref(), Some(said));
    let smooth_alone = panel_note(None, true);
    assert_eq!(
        smooth_alone.as_deref(),
        Some("Smoothed: a curve, not more detail")
    );
    assert_eq!(panel_note(None, false), None);
}

#[test]
fn the_curve_is_asked_for_only_while_smoothing_is_wanted() {
    use std::cell::Cell;
    use std::sync::Arc;

    use voxa_core::Volume;

    let mut view = super::PlanesView::new(None);
    let volume = Volume::new([1, 1, 2], vec![1.0; 2], Affine::identity()).expect("adds up");
    let volume = Arc::new(crate::testing::held_of(&volume));
    let asked = Cell::new(0);
    let take = || {
        asked.set(asked.get() + 1);
        None
    };
    let nothing = super::Smoothing::Nothing;
    assert_eq!(view.smooth(None, None, None, take), nothing);
    assert_eq!(asked.get(), 0, "not wanted, not asked for");
    assert_eq!(view.smooth(None, Some(&volume), None, take), nothing);
    assert_eq!(view.smooth(None, Some(&volume), None, take), nothing);
    assert_eq!(asked.get(), 2, "looked for each frame until it is there");
}

/// A state whose scene shows the made-up single slice of this modality.
fn scene_of(modality: &str, rescale_type: Option<&str>) -> crate::state::AppState {
    let mut state = crate::testing::showing_a_scan_of(modality, rescale_type);
    state.scene.series = Some(crate::testing::id(crate::testing::KIND_SCAN));
    state
}

/// A volume of two voxels along x, holding -12 and 8, one millimetre apart, held as a
/// series' volume is.
fn two_voxels() -> voxa_core::HeldVolume {
    let placement =
        Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    let volume = voxa_core::Volume::new([2, 1, 1], vec![-12.0, 8.0], placement).expect("adds up");
    crate::testing::held_of(&volume)
}

#[test]
fn the_value_under_the_pointer_on_a_plane_is_read_from_the_volume_in_the_scans_unit() {
    let scan = two_voxels();
    let at = Some(Point::new(0.0, 0.0, 0.0));
    let said = |state: &crate::state::AppState, at| value_under(state, Some(&scan), at);
    assert_eq!(
        said(&scene_of("CT", None), at).as_deref(),
        Some("Value -12 HU")
    );
    let unspecified = scene_of("CT", Some("US"));
    assert_eq!(
        said(&unspecified, at).as_deref(),
        Some("Value -12 (no unit)")
    );
    let named = scene_of("CT", Some("MGML"));
    assert_eq!(said(&named, at).as_deref(), Some("Value -12 MGML"));
    let mr = scene_of("MR", None);
    assert_eq!(said(&mr, at).as_deref(), Some("Value -12 (no unit)"));
}

#[test]
fn between_two_voxels_the_value_is_the_nearest_voxels_never_a_blend() {
    // The planes may be drawn smooth, but a value read off them is one a voxel holds, as it is
    // on a slice: -12 and 8 are in the volume, and -2 between them is in neither.
    let scan = two_voxels();
    let state = scene_of("CT", None);
    let said = |x: f64| value_under(&state, Some(&scan), Some(Point::new(x, 0.0, 0.0)));
    assert_eq!(said(0.4).as_deref(), Some("Value -12 HU"));
    assert_eq!(said(0.6).as_deref(), Some("Value 8 HU"));
    // Within half a voxel beyond an edge the edge voxel is read; further, nothing is.
    assert_eq!(said(-0.4).as_deref(), Some("Value -12 HU"));
    assert_eq!(said(1.4).as_deref(), Some("Value 8 HU"));
    assert_eq!(said(-0.6), None);
    assert_eq!(said(1.6), None);
    assert_eq!(said(f64::NAN), None);
}

#[test]
fn off_the_volume_or_with_nothing_to_read_the_plane_says_no_value() {
    let scan = two_voxels();
    let state = scene_of("CT", None);
    let away = Some(Point::new(40.0, 0.0, 0.0));
    assert_eq!(value_under(&state, Some(&scan), away), None, "outside it");
    let at = Some(Point::new(0.0, 0.0, 0.0));
    assert_eq!(value_under(&state, None, at), None, "no volume yet");
    assert_eq!(value_under(&state, Some(&scan), None), None, "no pointer");
    let unplaced = crate::state::AppState::default();
    assert_eq!(
        value_under(&unplaced, Some(&scan), at),
        None,
        "no scan in the scene"
    );
}

#[test]
fn the_value_is_written_in_the_top_right_corner_of_a_panel_and_nothing_without_one() {
    use crate::ui::hand::Hand;
    let cell = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(300.0, 300.0));
    let hand = Hand::new(300.0, 300.0);
    let line = Some("Value -12 HU".to_owned());
    let seen = hand.settle(&mut |ui| super::write_value(ui, cell, line.clone()));
    // Each is written twice, the second time as a shadow beneath.
    assert_eq!(seen.words, ["Value -12 HU", "Value -12 HU"]);
    let at = seen.text_bounds("Value -12 HU").expect("written");
    assert!(
        at.right() > cell.center().x && at.top() < cell.center().y / 2.0,
        "in the top right corner: {at:?}"
    );
    assert!(cell.contains_rect(at));
    let seen = hand.settle(&mut |ui| super::write_value(ui, cell, None));
    assert_eq!(seen.words, Vec::<String>::new());
}
