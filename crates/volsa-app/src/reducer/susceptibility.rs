//! Showing a susceptibility map in place of the phase series in the scene: asking for one,
//! taking it in when it is ready, and going back to the phase, as the person asks or as the
//! scene moves to another series.

use volsa_core::SeriesId;

use super::post;
use crate::action::{Action, Effect};
use crate::scene::MapState;
use crate::state::{AppState, Notice, NoticeKind};
use crate::susceptibility::{can_go_back, can_make_map, facts};

/// The key of the notice that says a map could not be made, or why one cannot be now.
const KEY: &str = "susceptibility";

/// Ask for a susceptibility map of the phase series in the scene, if one can be made now; if
/// not, say why. The phase is shown until the map is ready.
pub(super) fn make(state: &mut AppState) -> Vec<Effect> {
    if let Err(why) = can_make_map(state) {
        post(state, KEY, Some(warning(why.to_owned())));
        return Vec::new();
    }
    let series = state.scene.series.clone();
    let listed = series.as_ref().and_then(|series| state.series(series));
    let Some((series, facts)) = series.clone().zip(listed.map(|listed| facts(&listed.info))) else {
        return Vec::new();
    };
    post(state, KEY, None);
    state.scene.map = MapState::Making;
    vec![Effect::MakeSusceptibility { series, facts }]
}

/// Give up the map shown and show the phase again: its volume is built again the usual way,
/// and the scene is seen as it was.
pub(super) fn back(state: &mut AppState) -> Vec<Effect> {
    if can_go_back(state).is_err() {
        return Vec::new();
    }
    let before = state.scene.clone();
    let mut effects = super::scene::rebuild(state);
    // The measurements of the series are the ones in hand: read again, they would be put
    // before themselves.
    effects.retain(|effect| !matches!(effect, Effect::LoadAnnotations { .. }));
    let scene = &mut state.scene;
    if scene.series == before.series {
        scene.camera = before.camera;
        scene.untouched = before.untouched;
        scene.corners = before.corners;
        scene.frame = before.frame;
        scene.look = before.look_before_map.unwrap_or(before.look);
        scene.look_before_map = None;
        scene.cuts = before.cuts;
        scene.slice_shown = before.slice_shown;
        scene.keeps_view = true;
    }
    effects
}

/// Leave a map shown, or being made, as the scene starts afresh: the map is given up, and the
/// viewports that showed it are given their own series' slices again, with a window of their
/// own chosen for them as for a series just opened. Called before the scene changes.
pub(super) fn leave(state: &mut AppState) -> Vec<Effect> {
    let was = std::mem::take(&mut state.scene.map);
    if let Some(look) = state.scene.look_before_map.take() {
        state.scene.look = look;
    }
    if was == MapState::Off {
        return Vec::new();
    }
    let mut effects = vec![Effect::DropSusceptibility];
    if let Some(series) = state
        .scene
        .series
        .clone()
        .filter(|_| was == MapState::Shown)
    {
        forget_windows(state, &series);
        for viewport in &mut state.viewports {
            if viewport.series.as_ref() == Some(&series) {
                viewport.wl_auto = true;
                viewport.wl_pending = true;
            }
        }
        effects.extend(asked_again(state, &series));
    }
    effects
}

/// What the worker making a map reports, as a volume is reported: the map ready, which is then
/// shown in place of the phase, or why it could not be made. A report about another series
/// is out of date and is ignored.
pub(super) fn arrived(state: &mut AppState, report: Action) -> Vec<Effect> {
    let ours = |state: &AppState, series: &SeriesId| state.scene.series.as_ref() == Some(series);
    match report {
        Action::VolumeReady { series, .. } if ours(state, &series) => shown(state, &series),
        Action::VolumeFailed { series, reason } if ours(state, &series) => {
            state.scene.map = MapState::Off;
            post(state, KEY, Some(warning(reason)));
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Show the map of `series`: every viewport showing that series is given the map's window,
/// and its slice is asked for again, so that what is said of it is said of the map. The windows
/// worked out from the phase's slices are forgotten.
fn shown(state: &mut AppState, series: &SeriesId) -> Vec<Effect> {
    state.scene.map = MapState::Shown;
    // A surface of a map would be the box of zeros round the head: what is drawn into the
    // field is shown as a projection of the brightest values instead.
    let scene = &mut state.scene;
    scene.look_before_map.get_or_insert(scene.look);
    scene.look = volsa_render::Look::Mip;
    forget_windows(state, series);
    let window = crate::susceptibility::window();
    for viewport in &mut state.viewports {
        if viewport.series.as_ref() == Some(series) {
            viewport.wl = window;
            viewport.wl_auto = false;
            viewport.wl_pending = false;
        }
    }
    let mut effects = asked_again(state, series);
    // The curve the planes are drawn smooth from runs through the phase: one through the map
    // is asked for.
    if state.planes.smooth {
        effects.push(Effect::BuildSmooth {
            series: series.clone(),
        });
    }
    effects
}

/// Forget the windows worked out for the slices of `series`: they were worked out from values
/// that are no longer the ones shown.
fn forget_windows(state: &mut AppState, series: &SeriesId) {
    state.windows.retain(|(of, _), _| of != series);
}

/// Ask again for the slice each viewport showing `series` shows.
fn asked_again(state: &AppState, series: &SeriesId) -> Vec<Effect> {
    let showing = state
        .viewports
        .iter()
        .filter(|viewport| viewport.series.as_ref() == Some(series));
    showing
        .map(|viewport| Effect::RequestFrames {
            series: series.clone(),
            around: viewport.slice,
        })
        .collect()
}

/// A warning that says `text`.
fn warning(text: String) -> Notice {
    Notice {
        kind: NoticeKind::Warning,
        text,
        details: Vec::new(),
        key: Some(KEY),
    }
}

#[cfg(test)]
mod tests;
