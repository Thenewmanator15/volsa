//! The one place the application's state changes.

use std::collections::HashMap;

use voxa_core::SeriesId;
use voxa_dicom::best_series;

use crate::action::{Action, Effect};
use crate::planes::Planes;
use crate::scene::{MainPanel, Scene};
use crate::state::{AppState, Notice, NoticeKind, PresetWindows, ViewportState};

mod annotations;
mod frame;
mod labelling;
mod labels;
mod link;
mod planes;
mod scene;
mod viewport;
mod views;

/// The key of the notice that counts skipped files.
const SKIPPED: &str = "skipped";
/// The key of the notice that says a source could not be opened.
const OPENING: &str = "opening";
/// The key of the notice that says saved views could not be written.
const VIEWS: &str = "views";

/// Apply one action to the state and say what work it calls for.
///
/// This is the only code that changes [`AppState`], and it does nothing else: no files, no
/// threads, no clock. Whatever an action refers to that is not there (a viewport, a series, a
/// preset, a notice, a cut) is ignored. After every action the 3D scene is kept on the series
/// the active viewport shows, the planes' position inside that scene's volume, the
/// annotations with the series they were made on, and the linked viewports on the position.
pub fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    // Indexing may have added slices to a series chosen before it ended.
    let rebuild = action == Action::IndexDone;
    let fit = matches!(action, Action::Fit { .. });
    let was = state.scene.series.clone();
    let before = link::Before::of(state, &action);
    let smoothed = planes::smoothing(state);
    let mut effects = act(state, action);
    if fit && state.main == MainPanel::Scene {
        // The key that shows the whole slice again shows the whole volume again too.
        scene::refit(state);
    }
    effects.extend(if rebuild {
        scene::rebuild(state)
    } else {
        scene::follow(state)
    });
    planes::settle(state);
    annotations::settle(state, was.as_ref());
    effects.extend(link::keep(state, &before));
    effects.extend(planes::keep_curve(state, &smoothed));
    frame::keep(state);
    effects
}

fn act(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::OpenPath(_)
        | Action::IndexProgress(_)
        | Action::IndexFailed(_)
        | Action::IndexDone
        | Action::FrameReady { .. }
        | Action::FrameFailed { .. } => source(state, action),
        Action::SetLayout(_)
        | Action::SetActive(_)
        | Action::ToggleRadiological
        | Action::TogglePatient
        | Action::SetTagFilter(_)
        | Action::DismissNotice(_)
        | Action::Post(_)
        | Action::SetMain(_) => {
            window(state, action);
            Vec::new()
        }
        // The Parts look has nothing to draw without labels on this series.
        Action::SceneLook(voxa_render::Look::Parts) if !labels::parts_look_allowed(state) => {
            Vec::new()
        }
        Action::SceneResized { .. }
        | Action::SceneOrbit { .. }
        | Action::ScenePan { .. }
        | Action::SceneZoom { .. }
        | Action::SceneView(_)
        | Action::SceneCentre(_)
        | Action::SceneLook(_)
        | Action::SceneUploaded { .. }
        | Action::CutAdd
        | Action::CutMove { .. }
        | Action::CutTurn { .. }
        | Action::CutRemove { .. }
        | Action::ShowSliceInScene(_)
        | Action::VolumeProgress { .. }
        | Action::VolumeReady { .. }
        | Action::VolumeFailed { .. } => {
            scene::reduce(state, action);
            Vec::new()
        }
        Action::ViewSave { .. }
        | Action::ViewApply(_)
        | Action::ViewDelete(_)
        | Action::ViewsLoaded { .. }
        | Action::ViewsNotStored { .. } => views::reduce(state, action),
        about_the_planes if planes::concerns(&about_the_planes) => {
            planes::reduce(state, &about_the_planes);
            Vec::new()
        }
        about_annotations if annotations::concerns(&about_annotations) => {
            annotations::reduce(state, about_annotations)
        }
        Action::Labelling(about_labelling) => labelling::reduce(state, about_labelling),
        about_labels if labels::concerns(&about_labels) => labels::reduce(state, about_labels),
        about_a_viewport => viewport::reduce(state, about_a_viewport),
    }
}

/// What opening, indexing and decoding report.
fn source(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::OpenPath(path) => {
            // Labels, and a run of labelling, belong to a study of the export that is being left.
            let labelling = state.labelling.running.is_some();
            let labelled = state.labels.status != crate::labels::LabelsStatus::None;
            *state = emptied(state);
            let stopped = labelling.then_some(Effect::StopLabelling);
            let dropped = labelled.then_some(Effect::DropLabels);
            return stopped
                .into_iter()
                .chain(dropped)
                .chain([Effect::StartIndex(path)])
                .collect();
        }
        Action::IndexProgress(index) => {
            post(state, SKIPPED, skipped_notice(&index.skipped));
            state.index = Some(index);
        }
        Action::IndexFailed(why) => {
            let notice = Notice {
                kind: NoticeKind::Error,
                text: why,
                details: Vec::new(),
                key: Some(OPENING),
            };
            post(state, OPENING, Some(notice));
        }
        Action::IndexDone => {
            // Thumbnails first: the request for the whole of the series opened is then the
            // newer one for that series, and is not set aside in favour of its thumbnail.
            let mut effects = vec![Effect::RequestThumbnails];
            effects.extend(open_best(state));
            return effects;
        }
        Action::FrameReady {
            series,
            slice,
            windows,
        } => {
            let key = (series, slice);
            state.failed.remove(&key);
            state.windows.insert(key, windows);
            for viewport in &mut state.viewports {
                settle_window(viewport, &state.windows);
            }
        }
        Action::FrameFailed {
            series,
            slice,
            reason,
        } => {
            state.failed.insert((series, slice), reason);
        }
        _ => {}
    }
    Vec::new()
}

/// The state with nothing open: what another path starts from. How the window is arranged
/// and what the person chose to see is kept; everything about the scan is gone.
fn emptied(state: &AppState) -> AppState {
    AppState {
        layout: state.layout,
        active: state.active,
        radiological: state.radiological,
        show_patient: state.show_patient,
        main: state.main,
        scene: Scene {
            viewport: state.scene.viewport,
            ..Scene::default()
        },
        planes: Planes {
            column: state.planes.column,
            linked: state.planes.linked,
            smooth: state.planes.smooth,
            ..Planes::default()
        },
        labels: labels::emptied(&state.labels),
        labelling: labelling::emptied(&state.labelling),
        ..AppState::default()
    }
}

/// What concerns the window as a whole, not one viewport.
fn window(state: &mut AppState, action: Action) {
    match action {
        Action::SetLayout(layout) => {
            state.layout = layout;
            if state.active >= layout.count() {
                state.active = 0;
            }
        }
        Action::SetActive(viewport) if viewport < state.layout.count() => state.active = viewport,
        Action::ToggleRadiological => state.radiological = !state.radiological,
        Action::TogglePatient => state.show_patient = !state.show_patient,
        Action::SetTagFilter(filter) => state.tag_filter = filter,
        Action::SetMain(panel) => state.main = panel,
        Action::Post(notice) => match notice.key {
            Some(key) => post(state, key, Some(notice)),
            None => state.notices.push(notice),
        },
        Action::DismissNotice(at) if at < state.notices.len() => {
            state.notices.remove(at);
        }
        _ => {}
    }
}

/// Show the most detailed series in the first viewport, unless that viewport is in use.
fn open_best(state: &mut AppState) -> Vec<Effect> {
    let free = state
        .viewports
        .first()
        .is_some_and(|viewport| viewport.series.is_none());
    let best = state
        .index
        .as_ref()
        .and_then(best_series)
        .map(|series| series.info.id.clone());
    match best {
        Some(series) if free => viewport::reduce(
            state,
            Action::SelectSeries {
                viewport: 0,
                series,
            },
        ),
        _ => Vec::new(),
    }
}

/// Replace the notice with this key, or remove it when there is nothing to say.
fn post(state: &mut AppState, key: &'static str, notice: Option<Notice>) {
    state.notices.retain(|notice| notice.key != Some(key));
    state.notices.extend(notice);
}

fn skipped_notice(skipped: &[String]) -> Option<Notice> {
    let text = match skipped.len() {
        0 => return None,
        1 => "1 file skipped".to_owned(),
        several => format!("{several} files skipped"),
    };
    Some(Notice {
        kind: NoticeKind::Info,
        text,
        details: skipped.to_vec(),
        key: Some(SKIPPED),
    })
}

/// Give a viewport still waiting for its automatic window the one worked out for the slice it
/// shows, once that slice has been decoded. The window then stays as the person scrolls: one
/// window for the series, not a different one for every slice.
fn settle_window(
    viewport: &mut ViewportState,
    windows: &HashMap<(SeriesId, usize), PresetWindows>,
) {
    if !viewport.wl_pending {
        return;
    }
    let shown = viewport
        .series
        .clone()
        .map(|series| (series, viewport.slice));
    if let Some([automatic, ..]) = shown.and_then(|key| windows.get(&key)) {
        viewport.wl = *automatic;
        viewport.wl_pending = false;
    }
}

#[cfg(test)]
mod annotations_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod labelling_result_tests;
#[cfg(test)]
mod labelling_tests;
#[cfg(test)]
mod labelling_words_tests;
#[cfg(test)]
mod labels_open_tests;
#[cfg(test)]
mod labels_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod planes_tests;
#[cfg(test)]
mod scene_camera_tests;
#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod viewport_tests;
#[cfg(test)]
mod views_tests;
#[cfg(test)]
mod window_tests;
