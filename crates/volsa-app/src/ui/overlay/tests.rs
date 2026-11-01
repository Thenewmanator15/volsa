use volsa_core::{Affine, Patient, SeriesId, SeriesInfo, StudyId, StudyInfo, Voxel, WindowLevel};
use volsa_render::ViewTransform;

use super::*;
use crate::action::Action;

/// A slice whose columns run along `across` and whose rows run along `down`, in the patient.
fn placed(across: [f64; 3], down: [f64; 3]) -> Affine<Voxel, Patient> {
    let normal = [
        across[1] * down[2] - across[2] * down[1],
        across[2] * down[0] - across[0] * down[2],
        across[0] * down[1] - across[1] * down[0],
    ];
    Affine::from_columns(across, down, normal, [0.0, 0.0, 0.0])
}

fn axial() -> Affine<Voxel, Patient> {
    placed([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

fn sagittal() -> Affine<Voxel, Patient> {
    placed([0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
}

/// Left, right, top, bottom.
fn letters(
    placement: &Affine<Voxel, Patient>,
    view: &ViewTransform,
    radiological: bool,
) -> [String; 4] {
    let labels = orientation_labels(placement, view, radiological);
    [labels.left, labels.right, labels.top, labels.bottom]
}

#[test]
fn an_axial_slice_has_the_patients_left_on_the_right_in_radiological_convention() {
    let fit = ViewTransform::fit();
    assert_eq!(letters(&axial(), &fit, true), ["R", "L", "A", "P"]);
    assert_eq!(
        letters(&axial(), &fit, false),
        ["L", "R", "A", "P"],
        "neurological"
    );
}

#[test]
fn a_tilted_slice_is_lettered_by_the_nearest_axis() {
    // Review focus 3: a head tilted forwards a little is still front at the top.
    let tilted = placed([1.0, 0.0, 0.0], [0.0, 0.98, 0.2]);
    assert_eq!(
        letters(&tilted, &ViewTransform::fit(), true),
        ["R", "L", "A", "P"]
    );
}

#[test]
fn a_sagittal_slice_has_front_and_back_across_and_head_at_the_top() {
    let fit = ViewTransform::fit();
    assert_eq!(letters(&sagittal(), &fit, true), ["A", "P", "S", "I"]);
    assert_eq!(
        letters(&sagittal(), &fit, false),
        ["A", "P", "S", "I"],
        "no left or right to swap"
    );
}

#[test]
fn a_coronal_slice_has_left_and_right_across_and_head_at_the_top() {
    let coronal = placed([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    assert_eq!(
        letters(&coronal, &ViewTransform::fit(), true),
        ["R", "L", "S", "I"]
    );
}

#[test]
fn flips_and_turns_move_the_letters_with_the_picture() {
    let fit = ViewTransform::fit();
    assert_eq!(
        letters(&axial(), &fit.flipped(true), true),
        ["L", "R", "A", "P"]
    );
    assert_eq!(
        letters(&axial(), &fit.flipped(false), true),
        ["R", "L", "P", "A"]
    );
    let turned = fit.turned_clockwise();
    assert_eq!(
        letters(&axial(), &turned, true),
        ["P", "A", "R", "L"],
        "left has gone to the top"
    );
    assert_eq!(
        letters(&axial(), &turned, false),
        ["P", "A", "L", "R"],
        "left and right swap wherever they lie"
    );
}

#[test]
fn the_picture_is_mirrored_with_its_letters_in_neurological_convention() {
    let fit = ViewTransform::fit();
    assert_eq!(display_view(Some(&axial()), &fit, true), fit);
    assert_eq!(display_view(Some(&axial()), &fit, false), fit.flipped(true));
    assert_eq!(display_view(Some(&sagittal()), &fit, false), fit);
    assert_eq!(
        display_view(None, &fit, false),
        fit,
        "without geometry nothing is known to mirror"
    );
    let turned = fit.turned_clockwise();
    assert_eq!(
        display_view(Some(&axial()), &turned, false),
        turned.flipped(false)
    );
}

#[test]
fn the_scale_bar_is_the_longest_round_length_that_fits() {
    assert_eq!(scale_bar(0.5, 150.0), Some((100.0, "50 mm".to_owned())));
    let (length, label) = scale_bar(3.0, 150.0).expect("fits");
    assert_eq!(label, "100 mm");
    assert!((length - 33.33).abs() < 0.01, "{length}");
    assert_eq!(scale_bar(1.0, 1.0), Some((1.0, "1 mm".to_owned())));
}

#[test]
fn the_scale_bar_never_overruns_and_is_always_a_round_length() {
    let allowed = ["1 mm", "2 mm", "5 mm", "10 mm", "20 mm", "50 mm", "100 mm"];
    for step in 0..20 {
        let mm_per_px = 0.02 * 1.4_f32.powi(step);
        let (length, label) = scale_bar(mm_per_px, 150.0).expect("something fits");
        assert!(length <= 150.0 && length > 0.0, "{length} at {mm_per_px}");
        assert!(allowed.contains(&label.as_str()), "{label}");
    }
}

#[test]
fn no_scale_bar_is_given_when_none_would_be_true() {
    assert_eq!(
        scale_bar(0.001, 150.0),
        None,
        "even a millimetre is wider than the room"
    );
    assert_eq!(scale_bar(0.0, 150.0), None);
    assert_eq!(scale_bar(f32::NAN, 150.0), None);
    assert_eq!(scale_bar(1.0, 0.0), None);
}

pub(super) fn study() -> StudyInfo {
    StudyInfo {
        id: StudyId("1.2.3".to_owned()),
        date: Some("20210830".to_owned()),
        description: "Head".to_owned(),
        patient_name: Some("Phantom^Test".to_owned()),
        patient_id: Some("PHANTOM-001".to_owned()),
        series: Vec::new(),
    }
}

pub(super) fn series() -> SeriesInfo {
    SeriesInfo {
        id: SeriesId("1.2.3.4".to_owned()),
        number: 4,
        description: "made-up axial".to_owned(),
        modality: "MR".to_owned(),
        rows: 480,
        cols: 400,
        slices: 30,
        spacing_mm: [0.7, 0.7, 7.5],
        voxel_to_patient: Some(axial()),
        problems: Vec::new(),
        ..SeriesInfo::default()
    }
}

fn all_of(corners: &[Vec<String>; 4]) -> String {
    corners
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_corners_say_what_is_shown_and_keep_the_patient_out_unless_asked() {
    let wl = WindowLevel::new(502.4, 984.6);
    let hidden = corner_text(&study(), &series(), 11, Some(71.5), wl, false);
    let text = all_of(&hidden);
    assert!(
        !text.contains("Phantom") && !text.contains("PHANTOM-001"),
        "{text}"
    );
    for expected in [
        "30 August 2021",
        "Head",
        "made-up axial",
        "Slice 12 of 30",
        "71.5 mm",
        "W 985",
        "L 502",
    ] {
        assert!(text.contains(expected), "{expected} in {text}");
    }
    let shown = all_of(&corner_text(&study(), &series(), 11, Some(71.5), wl, true));
    assert!(
        shown.contains("Phantom^Test") && shown.contains("PHANTOM-001"),
        "{shown}"
    );
}

#[test]
fn a_series_without_geometry_says_so_in_its_corner() {
    let wl = WindowLevel::new(0.0, 1.0);
    let text = all_of(&corner_text(&study(), &series(), 0, None, wl, false));
    assert!(
        text.contains("No position data: orientation and scale are unavailable"),
        "{text}"
    );
}

#[test]
fn a_slice_is_placed_by_its_distance_along_the_way_the_slices_face() {
    let stack = Affine::from_columns(
        [0.5, 0.0, 0.0],
        [0.0, 0.5, 0.0],
        [0.0, 0.0, 7.5],
        [-100.0, -120.0, 10.0],
    );
    assert_eq!(slice_position(&stack, 0), 10.0);
    assert_eq!(slice_position(&stack, 4), 40.0);
}

fn quiet() -> ViewportInput {
    ViewportInput {
        window_width: 256.0,
        ..ViewportInput::default()
    }
}

#[test]
fn a_wheel_notch_moves_one_slice() {
    let down = ViewportInput {
        wheel: 1,
        ..quiet()
    };
    assert_eq!(
        input_to_actions(2, &down),
        vec![Action::Scroll {
            viewport: 2,
            delta: 1
        }]
    );
    let up = ViewportInput {
        wheel: -3,
        ..quiet()
    };
    assert_eq!(
        input_to_actions(0, &up),
        vec![Action::Scroll {
            viewport: 0,
            delta: -3
        }]
    );
    assert_eq!(input_to_actions(0, &quiet()), Vec::new());
}

#[test]
fn the_wheel_with_ctrl_zooms_about_the_pointer() {
    let towards = ViewportInput {
        wheel: -2,
        zooming: true,
        pointer: [30.0, -10.0],
        ..quiet()
    };
    let actions = input_to_actions(1, &towards);
    let [
        Action::Zoom {
            viewport: 1,
            factor,
            about,
        },
    ] = actions.as_slice()
    else {
        panic!("{actions:?}");
    };
    assert!(
        (factor - 1.21).abs() < 1e-5,
        "two notches away magnify by 1.1 twice: {factor}"
    );
    assert_eq!(*about, [30.0, -10.0]);
}

#[test]
fn a_right_drag_changes_the_window_at_a_rate_set_by_its_width() {
    let across = ViewportInput {
        right_drag: [10.0, 0.0],
        ..quiet()
    };
    let expected = Action::AdjustWindow {
        viewport: 0,
        d_center: 0.0,
        d_width: 10.0,
    };
    assert_eq!(input_to_actions(0, &across), vec![expected]);
    let wide = ViewportInput {
        right_drag: [0.0, -4.0],
        window_width: 2560.0,
        ..quiet()
    };
    let expected = Action::AdjustWindow {
        viewport: 0,
        d_center: -40.0,
        d_width: 0.0,
    };
    assert_eq!(input_to_actions(0, &wide), vec![expected]);
}

#[test]
fn presses_drags_and_double_clicks_each_do_their_one_thing() {
    let everything = ViewportInput {
        pressed: true,
        double_clicked: true,
        middle_drag: [3.0, 4.0],
        ..quiet()
    };
    let expected = vec![
        Action::SetActive(3),
        Action::Fit { viewport: 3 },
        Action::Pan {
            viewport: 3,
            by: [3.0, 4.0],
        },
    ];
    assert_eq!(input_to_actions(3, &everything), expected);
}

#[test]
fn a_width_or_a_level_is_written_to_the_figures_that_matter() {
    // No decimals from 100, one from 10, and below that three figures.
    for (value, written) in [
        (590.97, "591"),
        (315.6, "316"),
        (100.0, "100"),
        (99.96, "100.0"),
        (12.34, "12.3"),
        (10.0, "10.0"),
        (5.234, "5.23"),
        (0.5234, "0.523"),
        (0.052_34, "0.0523"),
        (0.0, "0"),
        (-600.0, "-600"),
        (-0.5234, "-0.523"),
    ] {
        assert_eq!(window_number(value), written, "{value}");
    }
    assert_eq!(window_number(f32::NAN), "");
    assert_eq!(window_number(f32::INFINITY), "");
    // A map of small values is not written as W 0.
    let tiny = 0.000_000_1;
    assert_eq!(window_number(tiny), "0.000000");
}
