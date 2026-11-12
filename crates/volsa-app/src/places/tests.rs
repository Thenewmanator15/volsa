//! A place: what it takes from a viewport, and what of a kept one is trusted.

use volsa_core::WindowLevel;
use volsa_render::{MAX_ZOOM, MIN_ZOOM, ViewTransform};

use super::*;

fn a_place() -> Place {
    Place {
        slice: 12,
        window: Some([40.0, 400.0]),
        zoom: 3.0,
        pan: [25.0, -40.0],
        flip_h: false,
        flip_v: true,
        turns: 3,
    }
}

#[test]
fn a_place_gives_back_the_viewport_it_was_taken_from() {
    let view = ViewTransform::fit()
        .zoomed_about(2.5, [10.0, 4.0])
        .flipped(true)
        .turned_clockwise();
    let shown = ViewportState {
        slice: 9,
        wl: WindowLevel::new(-20.0, 1500.0),
        wl_auto: false,
        view,
        ..ViewportState::default()
    };
    let place = Place::of(&shown);
    assert_eq!(place.slice, 9);
    assert_eq!(place.window(), Some(WindowLevel::new(-20.0, 1500.0)));
    assert_eq!(place.view(), view);
    assert_eq!(place.fitted(30), Some(place), "it fits its own series");
}

#[test]
fn a_window_volsa_chose_is_not_part_of_a_place() {
    let shown = ViewportState {
        wl: WindowLevel::new(300.0, 900.0),
        wl_auto: true,
        ..ViewportState::default()
    };
    assert_eq!(Place::of(&shown).window, None);
    assert_eq!(Place::of(&shown).window(), None);
}

#[test]
fn a_series_just_chosen_is_on_its_middle_slice_fitted_in_volsas_own_window() {
    let fresh = Place::fresh(30);
    assert_eq!((fresh.slice, fresh.window), (15, None));
    assert_eq!(fresh.view(), ViewTransform::fit());
}

#[test]
fn a_place_never_holds_what_is_not_a_number() {
    let mut shown = ViewportState {
        wl_auto: false,
        ..ViewportState::default()
    };
    shown.wl.center = f32::NAN;
    shown.view.zoom = f32::INFINITY;
    shown.view.pan = [f32::NAN, 3.0];
    shown.view.quarter_turns = 7;
    let place = Place::of(&shown);
    assert_eq!(
        (place.window, place.zoom, place.pan),
        (None, 1.0, [0.0, 3.0])
    );
    assert_eq!(place.turns, 3);
    let text = serde_json::to_string(&place).expect("writes");
    assert_eq!(text.matches("null").count(), 1, "only the window: {text}");
    assert_eq!(place.fitted(4).map(|fitted| fitted.slice), Some(0));
}

#[test]
fn a_slice_past_the_end_becomes_the_last_and_a_series_of_no_slices_has_no_place() {
    let far = Place {
        slice: 99,
        ..a_place()
    };
    assert_eq!(far.fitted(30).map(|fitted| fitted.slice), Some(29));
    assert_eq!(a_place().fitted(13).map(|fitted| fitted.slice), Some(12));
    assert_eq!(a_place().fitted(1).map(|fitted| fitted.slice), Some(0));
    assert_eq!(a_place().fitted(0), None);
}

#[test]
fn numbers_that_are_not_numbers_or_out_of_all_reach_are_refused() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (zoom, pan) = (bad, [0.0, bad]);
        let refused = [
            Place { zoom, ..a_place() },
            Place { pan, ..a_place() },
            Place {
                window: Some([bad, 400.0]),
                ..a_place()
            },
            Place {
                window: Some([40.0, bad]),
                ..a_place()
            },
        ];
        for place in refused {
            assert_eq!(place.fitted(30), None, "{place:?}");
        }
    }
    let zooms = [MAX_ZOOM * 2.0, MIN_ZOOM / 2.0, 0.0, -1.0];
    for zoom in zooms {
        assert_eq!(Place { zoom, ..a_place() }.fitted(30), None, "{zoom}");
    }
    let (pan, turns) = ([2.0e5, 0.0], 4);
    assert_eq!(Place { pan, ..a_place() }.fitted(30), None);
    assert_eq!(Place { turns, ..a_place() }.fitted(30), None);
}

#[test]
fn a_window_too_narrow_to_show_anything_is_widened_as_any_window_is() {
    let narrow = Place {
        window: Some([40.0, -3.0]),
        ..a_place()
    };
    let fitted = narrow.fitted(30).expect("fits");
    assert_eq!(fitted.window, Some([40.0, 1.0]));
}
