//! A series of pictures in colour in the state: shown, with no window and no 3D view.

use volsa_core::WindowLevel;

use super::scene_tests::select;
use super::*;
use crate::scene::SceneStatus;
use crate::testing::{COLOUR, colour_files, id, indexed, three_series_files};

fn on_the_colour_pictures() -> AppState {
    let files = [three_series_files(), colour_files()].concat();
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(files)));
    select(&mut state, 0, COLOUR);
    state
}

#[test]
fn pictures_in_colour_have_no_window_to_set() {
    let mut state = on_the_colour_pictures();
    let before = state.viewports[0].clone();
    let wl = WindowLevel::new(321.0, 123.0);
    reduce(&mut state, Action::SetWindow { viewport: 0, wl });
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 1,
        },
    );
    reduce(
        &mut state,
        Action::AdjustWindow {
            viewport: 0,
            d_center: 12.0,
            d_width: 7.0,
        },
    );
    let after = &state.viewports[0];
    assert_eq!((after.wl, after.wl_auto), (before.wl, before.wl_auto));
    // The view is still the person's to change.
    reduce(&mut state, Action::Rotate { viewport: 0 });
    assert_ne!(state.viewports[0].view, before.view);
}

#[test]
fn pictures_in_colour_have_no_3d_view_and_say_why() {
    let state = on_the_colour_pictures();
    assert_eq!(state.scene.series, Some(id(COLOUR)));
    let SceneStatus::Unavailable(lines) = &state.scene.status else {
        panic!("expected no 3D view, got {:?}", state.scene.status);
    };
    assert_eq!(lines[0], "Pictures in colour have no 3D view.");
}
