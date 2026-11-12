//! A slice is shown the way round the convention has it however its file stores it, and the
//! person's own flips and turns are made to that.

use volsa_core::{Affine, Patient, Voxel};
use volsa_render::ViewTransform;

use super::*;

/// A slice whose columns run along `across` and whose rows run along `down`, in the patient.
fn placed(across: [f64; 3], down: [f64; 3]) -> Affine<Voxel, Patient> {
    let normal = [
        across[1] * down[2] - across[2] * down[1],
        across[2] * down[0] - across[0] * down[2],
        across[0] * down[1] - across[1] * down[0],
    ];
    Affine::from_columns(across, down, normal, [0.0, 0.0, 0.0])
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

const LEFT: [f64; 3] = [1.0, 0.0, 0.0];
const BACK: [f64; 3] = [0.0, 1.0, 0.0];
const HEAD: [f64; 3] = [0.0, 0.0, 1.0];

fn other_way(way: [f64; 3]) -> [f64; 3] {
    way.map(|part| -part)
}

/// The eight ways round a file can store a slice whose sides run along `one` and `other`.
fn every_way_round(one: [f64; 3], other: [f64; 3]) -> Vec<Affine<Voxel, Patient>> {
    let mut ways = Vec::new();
    for (across, down) in [(one, other), (other, one)] {
        for across in [across, other_way(across)] {
            for down in [down, other_way(down)] {
                ways.push(placed(across, down));
            }
        }
    }
    ways
}

/// Every view a person can make with flips and turns alone.
fn every_view() -> Vec<ViewTransform> {
    let mut views = Vec::new();
    for flip_h in [false, true] {
        for flip_v in [false, true] {
            for quarter_turns in 0..4 {
                views.push(ViewTransform {
                    flip_h,
                    flip_v,
                    quarter_turns,
                    ..ViewTransform::fit()
                });
            }
        }
    }
    views
}

#[test]
fn a_slice_from_above_has_the_patients_left_on_the_right_however_its_file_stores_it() {
    let fit = ViewTransform::fit();
    for stored in every_way_round(LEFT, BACK) {
        let said = format!("{:?}", stored.columns());
        assert_eq!(letters(&stored, &fit, true), ["R", "L", "A", "P"], "{said}");
        assert_eq!(
            letters(&stored, &fit, false),
            ["L", "R", "A", "P"],
            "{said}"
        );
    }
    // A patient lying face down, as a scanner stores the slice: rows to the patient's right
    // and columns to the front. It is shown as any other slice from above is.
    let face_down = placed(other_way(LEFT), other_way(BACK));
    assert_eq!(letters(&face_down, &fit, true), ["R", "L", "A", "P"]);
}

#[test]
fn a_slice_from_the_front_or_the_side_is_the_way_round_the_plane_views_have_it() {
    let fit = ViewTransform::fit();
    for stored in every_way_round(LEFT, HEAD) {
        assert_eq!(letters(&stored, &fit, true), ["R", "L", "S", "I"]);
        assert_eq!(letters(&stored, &fit, false), ["L", "R", "S", "I"]);
    }
    for stored in every_way_round(BACK, HEAD) {
        // The front of the head on the left and the head at the top, in both conventions.
        assert_eq!(letters(&stored, &fit, true), ["A", "P", "S", "I"]);
        assert_eq!(letters(&stored, &fit, false), ["A", "P", "S", "I"]);
    }
}

#[test]
fn the_persons_own_flips_and_turns_do_to_every_slice_what_they_do_to_one_stored_upright() {
    let upright = placed(LEFT, BACK);
    for stored in every_way_round(LEFT, BACK) {
        for own in every_view() {
            for radiological in [true, false] {
                assert_eq!(
                    letters(&stored, &own, radiological),
                    letters(&upright, &own, radiological),
                    "{:?} through {own:?}",
                    stored.columns()
                );
            }
        }
    }
    // And what they do to one stored upright is what they always did.
    let fit = ViewTransform::fit();
    assert_eq!(display_view(Some(&upright), &fit, true), fit);
    let turned = fit.turned_clockwise();
    assert_eq!(letters(&upright, &turned, true), ["P", "A", "R", "L"]);
    assert_eq!(
        letters(&upright, &fit.flipped(true), true),
        ["L", "R", "A", "P"]
    );
}

#[test]
fn the_persons_zoom_and_where_they_moved_the_picture_are_kept() {
    let face_down = placed(other_way(LEFT), other_way(BACK));
    let own = ViewTransform {
        zoom: 2.5,
        pan: [12.0, -7.0],
        ..ViewTransform::fit()
    };
    let shown = display_view(Some(&face_down), &own, true);
    assert_eq!((shown.zoom, shown.pan), (2.5, [12.0, -7.0]));
    assert_eq!(shown.quarter_turns, 2, "turned half round, not mirrored");
    assert!(!shown.flip_h && !shown.flip_v);
}

#[test]
fn a_slice_with_no_geometry_is_drawn_as_its_file_holds_it() {
    let own = ViewTransform::fit().turned_clockwise();
    assert_eq!(display_view(None, &own, true), own);
    assert_eq!(display_view(None, &own, false), own);
}
