//! Where each series was left, in the state: asking for what is kept, putting a series back,
//! and asking for a place to be written only once what is kept has been read.

use volsa_core::{StudyId, WindowLevel};
use volsa_render::ViewTransform;

use super::scene_tests::{corners, select};
use super::tests::{frames_around, loaded};
use super::*;
use crate::places::{Place, PlacesAction};
use crate::testing::{FINE_SCAN, FIRST_ECHO, SCOUT, SECOND_ECHO, THICK_SCAN, id, two_echoes_files};

/// The three-series export read through: Volsa has opened the most detailed series itself.
fn opened() -> AppState {
    let mut state = loaded();
    reduce(&mut state, Action::IndexDone);
    assert_eq!(state.viewports[0].series, Some(id(FINE_SCAN)));
    state
}

/// A worker reports what is kept for the export that is open.
fn read(state: &mut AppState, places: &[(&str, Place)], first: Option<&str>) -> Vec<Effect> {
    let places = places.iter().map(|(series, place)| (id(series), *place));
    let report = PlacesAction::Loaded {
        opened: state.opened,
        places: places.collect(),
        first: first.map(id),
    };
    reduce(state, Action::Places(report))
}

/// A place nobody would be at by chance.
fn a_place() -> Place {
    Place {
        slice: 7,
        window: Some([40.0, 400.0]),
        zoom: 2.0,
        pan: [10.0, -5.0],
        flip_h: true,
        flip_v: false,
        turns: 1,
    }
}

fn study(state: &AppState) -> StudyId {
    state.index.as_ref().expect("indexed").studies[0].id.clone()
}

/// What an action asked to be written, if anything.
fn stored(effects: &[Effect]) -> Option<&Effect> {
    let mut stores = effects
        .iter()
        .filter(|effect| matches!(effect, Effect::StorePlaces { .. }));
    let store = stores.next();
    assert_eq!(stores.next(), None, "one write for one action");
    store
}

fn scroll(state: &mut AppState, delta: i32) {
    let viewport = 0;
    reduce(state, Action::Scroll { viewport, delta });
}

#[test]
fn an_export_read_through_asks_for_what_is_kept_for_its_series_and_studies() {
    let mut state = loaded();
    let effects = reduce(&mut state, Action::IndexDone);
    let asked = Effect::LoadPlaces {
        opened: state.opened,
        series: [THICK_SCAN, FINE_SCAN, SCOUT].map(id).to_vec(),
        studies: vec![study(&state)],
    };
    assert!(effects.contains(&asked), "{effects:?}");
    assert_eq!(state.places.automatic, Some(id(FINE_SCAN)));
    assert!(!state.places.read, "not until a worker says");
}

#[test]
fn a_kept_place_is_put_back_on_a_series_still_as_choosing_it_left_it() {
    let mut state = opened();
    let effects = read(&mut state, &[(FINE_SCAN, a_place())], None);
    assert_eq!(effects, frames_around(FINE_SCAN, 7));
    let shown = &state.viewports[0];
    assert_eq!(shown.slice, 7);
    assert_eq!(shown.wl, WindowLevel::new(40.0, 400.0));
    assert!(
        !shown.wl_auto && !shown.wl_pending,
        "the person's own window"
    );
    assert_eq!(shown.view, a_place().view());
    assert_eq!(shown.view.quarter_turns, 1);
    // It is where the file has it, so there is nothing to write when it leaves.
    assert_eq!(stored(&select(&mut state, 1, THICK_SCAN)), None);
}

#[test]
fn a_window_that_was_volsas_own_choice_is_not_kept_and_is_chosen_again() {
    let mut state = opened();
    let place = Place {
        window: None,
        ..a_place()
    };
    read(&mut state, &[(FINE_SCAN, place)], None);
    let shown = &state.viewports[0];
    assert!(shown.wl_auto && shown.wl_pending, "still Volsa's to choose");
    assert_eq!(Place::of(shown).window, None);
}

#[test]
fn a_kept_place_does_not_take_the_place_of_what_the_person_has_done() {
    for touch in [
        Action::Scroll {
            viewport: 0,
            delta: 3,
        },
        Action::SetWindow {
            viewport: 0,
            wl: WindowLevel::new(10.0, 20.0),
        },
        Action::Rotate { viewport: 0 },
        Action::Pan {
            viewport: 0,
            by: [4.0, 0.0],
        },
    ] {
        let mut state = opened();
        reduce(&mut state, touch.clone());
        let before = state.viewports[0].clone();
        let effects = read(&mut state, &[(FINE_SCAN, a_place())], None);
        assert_eq!(effects, Vec::new(), "{touch:?}");
        assert_eq!(state.viewports[0], before, "{touch:?}");
        // What the person did is what is written when the series leaves.
        let effects = select(&mut state, 0, THICK_SCAN);
        let Some(Effect::StorePlaces { places, .. }) = stored(&effects) else {
            panic!("{effects:?}");
        };
        assert_eq!(*places, [(id(FINE_SCAN), Place::of(&before))], "{touch:?}");
    }
}

#[test]
fn nothing_is_written_before_what_is_kept_has_been_read() {
    let mut state = opened();
    scroll(&mut state, 5);
    let keep = Action::Places(PlacesAction::Keep);
    assert_eq!(stored(&reduce(&mut state, keep.clone())), None);
    assert_eq!(stored(&select(&mut state, 0, THICK_SCAN)), None);
    scroll(&mut state, 2);
    let another = std::path::PathBuf::from("another export");
    assert_eq!(stored(&reduce(&mut state, Action::OpenPath(another))), None);
    // And what is read for the export before is not taken for this one's.
    let for_another = PlacesAction::Loaded {
        opened: state.opened - 1,
        places: Vec::new(),
        first: None,
    };
    reduce(&mut state, Action::Places(for_another));
    assert!(!state.places.read);
    assert_eq!(stored(&reduce(&mut state, keep)), None);
}

#[test]
fn a_series_that_leaves_its_viewport_has_its_place_written_and_comes_back_to_it() {
    let mut state = opened();
    assert_eq!(read(&mut state, &[], None), Vec::new(), "nothing to write");
    scroll(&mut state, 3);
    let wl = WindowLevel::new(50.0, 350.0);
    reduce(&mut state, Action::SetWindow { viewport: 0, wl });
    let left = Place {
        slice: 35,
        window: Some([50.0, 350.0]),
        ..Place::fresh(64)
    };
    let effects = select(&mut state, 0, THICK_SCAN);
    let written = Effect::StorePlaces {
        places: vec![(id(FINE_SCAN), left)],
        first: Some((study(&state), id(THICK_SCAN))),
    };
    assert_eq!(stored(&effects), Some(&written));
    // Chosen again, it is as it was left; the series left untouched has no place to write.
    let effects = select(&mut state, 0, FINE_SCAN);
    assert_eq!(Place::of(&state.viewports[0]), left);
    assert!(!state.viewports[0].wl_auto);
    let written = Effect::StorePlaces {
        places: Vec::new(),
        first: Some((study(&state), id(FINE_SCAN))),
    };
    assert_eq!(stored(&effects), Some(&written));
    assert_eq!(effects.last(), Some(&written));
    assert!(effects.contains(&frames_around(FINE_SCAN, 35)[0]));
}

#[test]
fn opening_another_export_writes_where_every_series_shown_was_left() {
    let mut state = opened();
    read(&mut state, &[], None);
    reduce(
        &mut state,
        Action::SetLayout(crate::state::Layout::TwoAcross),
    );
    select(&mut state, 1, THICK_SCAN);
    reduce(&mut state, Action::Rotate { viewport: 1 });
    select(&mut state, 0, FINE_SCAN);
    scroll(&mut state, -2);
    let another = std::path::PathBuf::from("another export");
    let effects = reduce(&mut state, Action::OpenPath(another));
    let turned = Place {
        turns: 1,
        ..Place::fresh(30)
    };
    let scrolled = Place {
        slice: 30,
        ..Place::fresh(64)
    };
    let written = Effect::StorePlaces {
        places: vec![(id(FINE_SCAN), scrolled), (id(THICK_SCAN), turned)],
        first: None,
    };
    assert_eq!(stored(&effects), Some(&written));
    assert_eq!(state.places, crate::places::Places::default());
}

#[test]
fn keeping_writes_what_has_changed_once_and_nothing_when_nothing_has() {
    let mut state = opened();
    read(&mut state, &[], None);
    let keep = Action::Places(PlacesAction::Keep);
    assert_eq!(
        reduce(&mut state, keep.clone()),
        Vec::new(),
        "as it was opened"
    );
    scroll(&mut state, 1);
    let scrolled = Place {
        slice: 33,
        ..Place::fresh(64)
    };
    let written = Effect::StorePlaces {
        places: vec![(id(FINE_SCAN), scrolled)],
        first: None,
    };
    assert_eq!(reduce(&mut state, keep.clone()), [written]);
    assert_eq!(reduce(&mut state, keep), Vec::new(), "written already");
}

#[test]
fn the_series_kept_as_the_studys_first_is_opened_in_place_of_volsas_own_choice() {
    let mut state = opened();
    let effects = read(&mut state, &[(THICK_SCAN, a_place())], Some(THICK_SCAN));
    let shown = &state.viewports[0];
    assert_eq!(shown.series, Some(id(THICK_SCAN)));
    assert_eq!(shown.slice, 7, "and as it was left");
    assert_eq!(state.scene.series, Some(id(THICK_SCAN)));
    assert!(effects.contains(&frames_around(THICK_SCAN, 7)[0]));
    assert_eq!(stored(&effects), None, "the file holds it already");
}

#[test]
fn a_series_the_person_chose_is_still_what_is_shown_and_becomes_the_studys_first() {
    // Chosen while the export was still being read.
    let mut state = loaded();
    select(&mut state, 0, SCOUT);
    reduce(&mut state, Action::IndexDone);
    assert_eq!(state.places.automatic, None);
    let effects = read(&mut state, &[], Some(THICK_SCAN));
    assert_eq!(state.viewports[0].series, Some(id(SCOUT)));
    let written = Effect::StorePlaces {
        places: Vec::new(),
        first: Some((study(&state), id(SCOUT))),
    };
    assert_eq!(effects, [written]);
    // Chosen after Volsa's own choice, before what is kept was read.
    let mut state = opened();
    select(&mut state, 0, SCOUT);
    read(&mut state, &[], Some(THICK_SCAN));
    assert_eq!(state.viewports[0].series, Some(id(SCOUT)));
    // Volsa's own choice, scrolled through already, stays too.
    let mut state = opened();
    scroll(&mut state, 1);
    read(&mut state, &[], Some(THICK_SCAN));
    assert_eq!(state.viewports[0].series, Some(id(FINE_SCAN)));
}

#[test]
fn a_first_series_that_is_no_longer_in_the_export_is_not_looked_for() {
    let mut state = opened();
    let effects = read(&mut state, &[("9.9.9", a_place())], Some("9.9.9"));
    assert_eq!(effects, Vec::new());
    assert_eq!(state.viewports[0].series, Some(id(FINE_SCAN)));
    assert_eq!(
        (state.places.first.clone(), state.places.kept.len()),
        (None, 0)
    );
}

#[test]
fn numbers_that_cannot_be_a_place_are_refused_and_a_slice_past_the_end_is_the_last() {
    let refused = [
        Place {
            zoom: f32::INFINITY,
            ..a_place()
        },
        Place {
            pan: [f32::NAN, 0.0],
            ..a_place()
        },
        Place {
            window: Some([f32::NEG_INFINITY, 400.0]),
            ..a_place()
        },
        Place {
            turns: 9,
            ..a_place()
        },
    ];
    for place in refused {
        let mut state = opened();
        let before = state.viewports[0].clone();
        assert_eq!(read(&mut state, &[(FINE_SCAN, place)], None), Vec::new());
        assert_eq!(state.viewports[0], before, "{place:?}");
        assert!(state.places.kept.is_empty());
    }
    let mut state = opened();
    let far = Place {
        slice: usize::MAX,
        ..a_place()
    };
    let effects = read(&mut state, &[(FINE_SCAN, far)], None);
    assert_eq!(effects, frames_around(FINE_SCAN, 63));
    assert_eq!(state.viewports[0].slice, 63);
}

#[test]
fn a_linked_viewport_put_back_takes_the_planes_position_with_it() {
    let mut state = opened();
    let (series, corners) = (id(FINE_SCAN), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    assert!(crate::planes::position_in_viewport(&state, 0).is_some());
    let before = state.planes.position;
    read(&mut state, &[(FINE_SCAN, a_place())], None);
    assert_eq!(state.viewports[0].slice, 7);
    assert_ne!(state.planes.position, before);
    assert!(
        crate::planes::position_in_viewport(&state, 0).is_some(),
        "the position is on the slice shown"
    );
}

#[test]
fn stepping_to_another_volume_keeps_the_slice_and_the_view_it_was_stepped_from() {
    let mut state = AppState::default();
    let index = crate::testing::indexed(two_echoes_files());
    reduce(&mut state, Action::IndexProgress(index));
    reduce(&mut state, Action::IndexDone);
    let (first, other) = (id(FIRST_ECHO), id(SECOND_ECHO));
    let elsewhere = Place {
        slice: 0,
        ..a_place()
    };
    read(&mut state, &[(SECOND_ECHO, elsewhere)], None);
    select(&mut state, 0, FIRST_ECHO);
    scroll(&mut state, 1);
    let was = state.viewports[0].clone();
    let effects = reduce(&mut state, Action::StepVolume { viewport: 0, by: 1 });
    let shown = &state.viewports[0];
    assert_eq!(shown.series, Some(other));
    assert_eq!((shown.slice, shown.view), (2, ViewTransform::fit()));
    // The volume left has its place written, as any series that leaves its viewport has.
    let Some(Effect::StorePlaces { places, .. }) = stored(&effects) else {
        panic!("{effects:?}");
    };
    assert_eq!(*places, [(first, Place::of(&was))]);
}
