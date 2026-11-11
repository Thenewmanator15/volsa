//! Saved views in the state: saving, applying, deleting, and loading those kept from before.

use voxa_core::WindowLevel;
use voxa_render::Look;

use super::scene_tests::{build, ready, select};
use super::tests::{frames_around, loaded};
use super::*;
use crate::testing::{FINE_SCAN, LOCALISER, THICK_SCAN, id, with_odd_series};
use crate::views::{MAX_VIEWS, SavedView, ViewsAction};

fn save(state: &mut AppState, name: &str) -> Vec<Effect> {
    reduce(
        state,
        Action::Views(ViewsAction::Save {
            name: name.to_owned(),
        }),
    )
}

fn stored(state: &AppState) -> Vec<Effect> {
    vec![Effect::StoreViews {
        series: id(THICK_SCAN),
        views: state.views.clone(),
    }]
}

#[test]
fn choosing_a_series_asks_for_the_views_saved_for_it() {
    let mut state = loaded();
    let effects = select(&mut state, 0, THICK_SCAN);
    let views = Effect::LoadViews {
        series: id(THICK_SCAN),
    };
    let annotations = Effect::LoadAnnotations {
        series: id(THICK_SCAN),
    };
    let expected = [
        frames_around(THICK_SCAN, 15),
        vec![build(THICK_SCAN), views, annotations],
    ]
    .concat();
    assert_eq!(effects, expected);
    // A series with no 3D view has no views to load.
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(with_odd_series()));
    assert_eq!(
        select(&mut state, 0, LOCALISER),
        frames_around(LOCALISER, 1)
    );
}

#[test]
fn views_loaded_for_the_scenes_series_are_taken_and_others_ignored() {
    let mut state = ready();
    let kept = vec![SavedView::of(
        "Kept",
        1,
        &state.scene,
        WindowLevel::new(1.0, 2.0),
    )];
    reduce(
        &mut state,
        Action::Views(ViewsAction::Loaded {
            series: id(FINE_SCAN),
            views: kept.clone(),
        }),
    );
    assert!(state.views.is_empty(), "those belong to another series");
    reduce(
        &mut state,
        Action::Views(ViewsAction::Loaded {
            series: id(THICK_SCAN),
            views: kept.clone(),
        }),
    );
    assert_eq!(state.views, kept);
    let too_many = vec![kept[0].clone(); MAX_VIEWS + 5];
    reduce(
        &mut state,
        Action::Views(ViewsAction::Loaded {
            series: id(THICK_SCAN),
            views: too_many,
        }),
    );
    assert_eq!(
        state.views.len(),
        MAX_VIEWS,
        "a file with more than can be kept is cut down"
    );
}

#[test]
fn a_view_saved_before_the_kept_ones_arrive_is_kept_with_them() {
    let mut state = ready();
    save(&mut state, "Just now");
    let just_now = state.views.clone();
    let before = |name: &str| SavedView::of(name, 1, &ready().scene, WindowLevel::new(1.0, 2.0));
    let arrive = |views: &[SavedView]| {
        Action::Views(ViewsAction::Loaded {
            series: id(THICK_SCAN),
            views: views.to_vec(),
        })
    };
    reduce(&mut state, arrive(&[before("Kept")]));
    let both = [vec![before("Kept")], just_now.clone()].concat();
    assert_eq!(state.views, both, "after the ones from before");
    // The file may hold the new one already, if it was written before it was read.
    reduce(&mut state, arrive(&both));
    assert_eq!(state.views, both, "and not twice");
    // With as many kept as there is room for, it is the oldest that goes.
    let full: Vec<SavedView> = (0..MAX_VIEWS)
        .map(|at| before(&format!("Kept {at}")))
        .collect();
    state.views.clone_from(&just_now);
    reduce(&mut state, arrive(&full));
    assert_eq!(state.views.len(), MAX_VIEWS);
    assert_eq!(state.views.last(), just_now.last(), "the new one stays");
    assert_eq!(state.views.first(), full.get(1), "the oldest has gone");
}

#[test]
fn a_saved_view_brings_back_the_camera_the_look_the_window_and_the_cuts() {
    let mut state = ready();
    reduce(&mut state, Action::SceneOrbit { by: [30.0, 12.0] });
    reduce(&mut state, Action::SceneLook(Look::Mip));
    reduce(&mut state, Action::CutAdd);
    let wl = WindowLevel::new(321.0, 123.0);
    reduce(&mut state, Action::SetWindow { viewport: 0, wl });
    let effects = save(&mut state, "As it was");
    assert_eq!(
        effects,
        stored(&state),
        "what is saved is written to the file"
    );
    assert_eq!(state.views.len(), 1);
    let as_it_was = state.scene.clone();

    reduce(&mut state, Action::SceneOrbit { by: [-200.0, 90.0] });
    reduce(
        &mut state,
        Action::SceneZoom {
            factor: 3.0,
            about: [10.0, 10.0],
        },
    );
    reduce(&mut state, Action::SceneLook(Look::Vr));
    reduce(&mut state, Action::CutRemove { cut: 0 });
    reduce(
        &mut state,
        Action::SetWindow {
            viewport: 0,
            wl: WindowLevel::new(5.0, 6.0),
        },
    );
    assert!(
        reduce(&mut state, Action::Views(ViewsAction::Apply(0))).is_empty(),
        "nothing to write"
    );
    assert_eq!(state.scene.camera, as_it_was.camera, "exactly where it was");
    assert_eq!(
        (state.scene.look, &state.scene.cuts),
        (Look::Mip, &as_it_was.cuts)
    );
    assert_eq!(state.viewports[0].wl, wl);
    assert!(
        !state.viewports[0].wl_auto && !state.scene.untouched,
        "it is the person's view"
    );
}

#[test]
fn a_view_with_no_name_is_numbered_and_the_twenty_first_drops_the_first() {
    let mut state = ready();
    save(&mut state, "");
    save(&mut state, "  ");
    let names: Vec<&str> = state.views.iter().map(|view| view.name.as_str()).collect();
    assert_eq!(names, ["View 1", "View 2"]);
    for _ in 2..MAX_VIEWS {
        save(&mut state, "");
    }
    assert_eq!(state.views.len(), MAX_VIEWS);
    let effects = save(&mut state, "One more");
    assert_eq!(state.views.len(), MAX_VIEWS);
    assert_eq!(state.views[0].name, "View 2", "the oldest went");
    assert_eq!(state.views[MAX_VIEWS - 1].name, "One more");
    assert_eq!(effects, stored(&state));
}

#[test]
fn a_view_is_deleted_by_its_place_and_one_that_is_not_there_is_left_alone() {
    let mut state = ready();
    save(&mut state, "A");
    save(&mut state, "B");
    let before = state.clone();
    assert!(
        reduce(&mut state, Action::Views(ViewsAction::Delete(5))).is_empty(),
        "no such view"
    );
    assert!(
        reduce(&mut state, Action::Views(ViewsAction::Apply(5))).is_empty(),
        "no such view"
    );
    assert_eq!(state, before);
    let effects = reduce(&mut state, Action::Views(ViewsAction::Delete(0)));
    assert_eq!(state.views.len(), 1);
    assert_eq!(state.views[0].name, "B");
    assert_eq!(effects, stored(&state));
}

#[test]
fn a_view_can_only_be_saved_of_a_volume_that_is_there() {
    let mut state = loaded();
    assert!(save(&mut state, "Nothing yet").is_empty(), "no series");
    select(&mut state, 0, THICK_SCAN);
    assert!(save(&mut state, "Still building").is_empty(), "no volume");
    assert_eq!(state.views, []);
}

#[test]
fn another_series_starts_with_no_views_until_its_own_are_loaded() {
    let mut state = ready();
    save(&mut state, "Of the first");
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(state.views, []);
}
