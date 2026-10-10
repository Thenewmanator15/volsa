//! Actions about the 3D scene, and keeping the scene on the series the active viewport shows.

use voxa_core::{SeriesId, SeriesProblem};
use voxa_render::{Camera, CutPlane, MAX_CUTS, MAX_MM_PER_PX, MIN_MM_PER_PX, NamedView};
use voxa_volume::can_be_volume;

use crate::action::{Action, Effect};
use crate::scene::{Scene, SceneStatus};
use crate::state::AppState;

pub(super) fn reduce(state: &mut AppState, action: Action) {
    let scene = &mut state.scene;
    match action {
        Action::SceneResized { viewport } => resized(scene, viewport),
        Action::SceneOrbit { by } => move_camera(scene, |camera| camera.orbited(by)),
        Action::ScenePan { by } => move_camera(scene, |camera| camera.panned(by)),
        Action::SceneZoom { factor, about } => {
            move_camera(scene, |camera| camera.zoomed_about(factor, about));
        }
        Action::SceneCentre(point) => move_camera(scene, |camera| camera.centred_on(point)),
        Action::SceneView(view) => {
            let facing = facing(scene, view);
            move_camera(scene, |_| facing);
        }
        Action::SceneLook(look) => scene.look = look,
        Action::SceneUploaded { halved } => scene.halved = halved,
        Action::ShowSliceInScene(shown) => scene.slice_shown = shown,
        Action::CutAdd if scene.cuts.len() < MAX_CUTS => {
            scene.cuts.push(CutPlane::facing(&scene.camera));
        }
        Action::CutMove { cut, by_mm } => change_cut(scene, cut, |plane| plane.moved(by_mm)),
        Action::CutTurn { cut, by } => {
            let camera = scene.camera;
            change_cut(scene, cut, |plane| plane.turned(&camera, by));
        }
        Action::CutRemove { cut } if cut < scene.cuts.len() => {
            scene.cuts.remove(cut);
        }
        about_the_volume => built(scene, about_the_volume),
    }
}

/// What the worker building the scene's volume reports. A report about any other series, or
/// one that comes when the scene is not building, is out of date and is ignored.
fn built(scene: &mut Scene, report: Action) {
    let building = |scene: &Scene, series: &SeriesId| {
        scene.series.as_ref() == Some(series)
            && matches!(scene.status, SceneStatus::Building { .. })
    };
    match report {
        Action::VolumeProgress { series, done, of } if building(scene, &series) => {
            scene.status = SceneStatus::Building { done, of };
        }
        Action::VolumeReady { series, corners } if building(scene, &series) => {
            scene.status = SceneStatus::Ready;
            scene.camera = Camera::fitted(NamedView::Anterior, &corners, scene.viewport);
            scene.corners = Some(corners);
            scene.untouched = true;
        }
        Action::VolumeFailed { series, reason } if building(scene, &series) => {
            scene.status = SceneStatus::Unavailable(vec![reason]);
        }
        _ => {}
    }
}

/// Show the whole volume again, from the way the camera already faces.
pub(super) fn refit(state: &mut AppState) {
    let scene = &mut state.scene;
    if let Some(corners) = scene.frame.or(scene.corners) {
        scene.camera = scene.camera.refitted(&corners, scene.viewport);
    }
}

/// Keep the scene on the series the active viewport shows: when that is no longer the series
/// in the scene, the scene starts afresh for it.
pub(super) fn follow(state: &mut AppState) -> Vec<Effect> {
    let shown = shown_in_the_active_viewport(state);
    if shown == state.scene.series {
        Vec::new()
    } else {
        start(state, shown)
    }
}

/// Start the scene afresh for the series the active viewport shows, even if it is the one
/// already there: its volume is built again.
pub(super) fn rebuild(state: &mut AppState) -> Vec<Effect> {
    let shown = shown_in_the_active_viewport(state);
    start(state, shown)
}

fn shown_in_the_active_viewport(state: &AppState) -> Option<SeriesId> {
    state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.clone())
}

/// What is said last of a series with no 3D view.
const AS_A_STACK: &str = "It is shown as a plain stack.";

/// An empty scene for `series`: building, if it can be a volume, and otherwise saying in
/// plain words why it cannot. Only the size of the picture is kept.
fn start(state: &mut AppState, series: Option<SeriesId>) -> Vec<Effect> {
    let fresh = Scene {
        viewport: state.scene.viewport,
        ..Scene::default()
    };
    // The views saved of the series before are not this one's.
    state.views.clear();
    let listed = series.as_ref().and_then(|id| state.series(id));
    let info = listed.map(|listed| listed.info.clone());
    let Some((series, info)) = series.zip(info) else {
        state.scene = fresh;
        return Vec::new();
    };
    let (status, effects) = match can_be_volume(&info) {
        Ok(()) => {
            let building = SceneStatus::Building {
                done: 0,
                of: info.slices,
            };
            let volume = Effect::BuildVolume {
                series: series.clone(),
            };
            let views = Effect::LoadViews {
                series: series.clone(),
            };
            let annotations = Effect::LoadAnnotations {
                series: series.clone(),
            };
            (building, vec![volume, views, annotations])
        }
        Err(why) => {
            // Why, each reason once, and then what is done about it, once.
            let problems = info.problems.iter().map(SeriesProblem::what);
            let said = std::iter::once(why.to_string()).chain(problems);
            let lines = said.chain([AS_A_STACK.to_owned()]).collect();
            (SceneStatus::Unavailable(lines), Vec::new())
        }
    };
    state.scene = Scene {
        series: Some(series),
        status,
        ..fresh
    };
    effects
}

/// A new size for the scene's picture. While the camera is still Voxa's own choice the volume
/// is fitted to it again. A camera the person has moved keeps what it shows: as much of the
/// patient across the shorter side of the picture as before, so that a scene given a smaller
/// part of the window is the same view, smaller, and not a corner of it. A size that is not
/// a size is ignored.
fn resized(scene: &mut Scene, viewport: [f32; 2]) {
    if !viewport.iter().all(|side| side.is_finite() && *side > 0.0) {
        return;
    }
    let shorter = |[wide, high]: [f32; 2]| f64::from(wide.min(high));
    let (was, is) = (shorter(scene.viewport), shorter(viewport));
    scene.viewport = viewport;
    if let Some(corners) = scene.frame.or(scene.corners).filter(|_| scene.untouched) {
        scene.camera = Camera::fitted(NamedView::Anterior, &corners, viewport);
    } else if was > 0.0 {
        let scale = scene.camera.mm_per_px * was / is;
        scene.camera.mm_per_px = scale.clamp(MIN_MM_PER_PX, MAX_MM_PER_PX);
    }
}

/// Move the camera as the person asked. From now on it is theirs, and is not refitted.
fn move_camera(scene: &mut Scene, to: impl FnOnce(Camera) -> Camera) {
    scene.camera = to(scene.camera);
    scene.untouched = false;
}

/// A camera facing one side of the patient: showing the whole volume if there is one, and
/// otherwise keeping its place and scale.
fn facing(scene: &Scene, view: NamedView) -> Camera {
    match &scene.frame.or(scene.corners) {
        Some(corners) => Camera::fitted(view, corners, scene.viewport),
        None => Camera::looking(view, scene.camera.target, scene.camera.mm_per_px),
    }
}

fn change_cut(scene: &mut Scene, cut: usize, change: impl FnOnce(CutPlane) -> CutPlane) {
    if let Some(plane) = scene.cuts.get_mut(cut) {
        *plane = change(*plane);
    }
}
