//! Actions about one viewport: what it shows, which slice, the view and the window.

use volsa_core::{SeriesId, SeriesInfo, SeriesKind, SeriesProblem, WindowLevel};
use volsa_render::ViewTransform;

use super::settle_window;
use crate::action::{Action, Effect};
use crate::naming::plain_name;
use crate::state::{AppState, Cine, Notice, NoticeKind, PresetWindows, ViewportState};

/// The key of the notice that says what stops the series opened being shown in full.
pub(super) const SERIES: &str = "series";

pub(super) fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::SelectSeries { viewport, series } => select(state, viewport, series),
        Action::Scroll { viewport, delta } => {
            let from = state.viewports.get(viewport).map(|viewport| viewport.slice);
            from.map_or_else(Vec::new, |from| go_to(state, viewport, moved(from, delta)))
        }
        Action::SetSlice { viewport, slice } => go_to(state, viewport, slice),
        Action::StepVolume { viewport, by } => super::volumes::step(state, viewport, by),
        Action::CineTick { viewport } => tick(state, viewport),
        other => {
            if !(about_the_window(&other) && in_colour(state, &other)) {
                adjust(state, other);
            }
            Vec::new()
        }
    }
}

/// Whether an action sets the window or level of a viewport.
fn about_the_window(action: &Action) -> bool {
    matches!(
        action,
        Action::AdjustWindow { .. } | Action::SetWindow { .. } | Action::ApplyPreset { .. }
    )
}

/// Whether the viewport a window action names shows pictures in colour, which are drawn in
/// their own colours and have no window to set.
fn in_colour(state: &AppState, action: &Action) -> bool {
    let (Action::AdjustWindow { viewport, .. }
    | Action::SetWindow { viewport, .. }
    | Action::ApplyPreset { viewport, .. }) = action
    else {
        return false;
    };
    let shown = state.viewports.get(*viewport);
    let series = shown.and_then(|shown| shown.series.as_ref());
    let listed = series.and_then(|series| state.series(series));
    listed.is_some_and(|listed| listed.info.colour)
}

/// Show a series: its middle slice, fitted, with the window still to be chosen.
fn select(state: &mut AppState, at: usize, series: SeriesId) -> Vec<Effect> {
    let Some(count) = state.series(&series).map(|listed| listed.slices.len()) else {
        return Vec::new();
    };
    let Some(viewport) = state.viewports.get_mut(at) else {
        return Vec::new();
    };
    let slice = count / 2;
    *viewport = ViewportState {
        series: Some(series.clone()),
        slice,
        ..ViewportState::default()
    };
    settle_window(viewport, &state.windows);
    if at < state.layout.count() {
        state.active = at;
    }
    // A scan shown takes the place of a document that was open in the middle of the window.
    state.document = None;
    say_what_is_wrong(state, &series);
    vec![Effect::RequestFrames {
        series,
        around: slice,
    }]
}

/// Say in a notice what stops `series`, just opened, being shown in full, in place of what was
/// said of the series opened before. A series with nothing wrong takes that away, once no
/// viewport shows a series with something wrong.
fn say_what_is_wrong(state: &mut AppState, series: &SeriesId) {
    let notice = state
        .series(series)
        .and_then(|listed| fault_notice(&listed.info));
    let shown = state.viewports.iter().take(state.layout.count());
    let at_fault = |id: &SeriesId| state.series(id).is_some_and(|l| l.info.kind().is_fault());
    let still = shown.filter_map(|v| v.series.as_ref()).any(at_fault);
    if notice.is_some() || !still {
        super::post(state, SERIES, notice);
    }
}

/// The notice of a series that cannot be shown in full: the sentence of what stops it, then,
/// beneath, the series' plain name and any other reasons. `None` for any other series.
fn fault_notice(info: &SeriesInfo) -> Option<Notice> {
    let first = match info.kind() {
        SeriesKind::Stack(problem) => problem,
        SeriesKind::Unreadable(name) => SeriesProblem::UnsupportedTransferSyntax(name),
        _ => return None,
    };
    let others = info.problems.iter().filter(|other| **other != first);
    let details = std::iter::once(plain_name(info)).chain(others.map(ToString::to_string));
    Some(Notice {
        kind: NoticeKind::Warning,
        text: first.to_string(),
        details: details.collect(),
        key: Some(SERIES),
    })
}

/// A slice number moved by `delta`, never below zero.
pub(super) fn moved(from: usize, delta: i32) -> usize {
    let by = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
    if delta < 0 {
        from.saturating_sub(by)
    } else {
        from.saturating_add(by)
    }
}

/// Show another slice of the same series, stopping at the last, and ask for the frames
/// around it. Nothing happens when the viewport is already there.
pub(super) fn go_to(state: &mut AppState, at: usize, slice: usize) -> Vec<Effect> {
    let count = state.slices_in(at);
    let Some(viewport) = state.viewports.get_mut(at) else {
        return Vec::new();
    };
    let Some(series) = viewport.series.clone() else {
        return Vec::new();
    };
    let slice = slice.min(count.saturating_sub(1));
    if slice == viewport.slice {
        return Vec::new();
    }
    viewport.slice = slice;
    settle_window(viewport, &state.windows);
    vec![Effect::RequestFrames {
        series,
        around: slice,
    }]
}

/// Playback moves on one slice. After the last it starts again when looping, and otherwise
/// stops.
fn tick(state: &mut AppState, at: usize) -> Vec<Effect> {
    let count = state.slices_in(at);
    let Some(viewport) = state.viewports.get_mut(at) else {
        return Vec::new();
    };
    let Some(cine) = viewport.cine.as_mut().filter(|cine| cine.playing) else {
        return Vec::new();
    };
    let next = viewport.slice + 1;
    if next < count {
        go_to(state, at, next)
    } else if cine.looping {
        go_to(state, at, 0)
    } else {
        cine.playing = false;
        Vec::new()
    }
}

/// Changes to the view, the window or playback that ask for no work.
fn adjust(state: &mut AppState, action: Action) {
    let (Action::Zoom { viewport: at, .. }
    | Action::Pan { viewport: at, .. }
    | Action::Flip { viewport: at, .. }
    | Action::Rotate { viewport: at }
    | Action::Fit { viewport: at }
    | Action::AdjustWindow { viewport: at, .. }
    | Action::SetWindow { viewport: at, .. }
    | Action::ApplyPreset { viewport: at, .. }
    | Action::CineToggle { viewport: at }
    | Action::CineSet { viewport: at, .. }) = action
    else {
        return;
    };
    let Some(viewport) = state.viewports.get_mut(at) else {
        return;
    };
    let shown = viewport
        .series
        .clone()
        .map(|series| (series, viewport.slice));
    let known = shown.and_then(|key| state.windows.get(&key)).copied();
    let view = viewport.view;
    match action {
        Action::Zoom { factor, about, .. } => viewport.view = view.zoomed_about(factor, about),
        Action::Pan { by: [x, y], .. } => viewport.view.pan = [view.pan[0] + x, view.pan[1] + y],
        Action::Flip { horizontal, .. } => viewport.view = view.flipped(horizontal),
        Action::Rotate { .. } => viewport.view = view.turned_clockwise(),
        Action::Fit { .. } => {
            viewport.view = ViewTransform {
                zoom: 1.0,
                pan: [0.0, 0.0],
                ..view
            }
        }
        Action::CineToggle { .. } => {
            let started = Cine::new(Cine::DEFAULT_FPS, true, true);
            let toggled = viewport.cine.map(|cine| Cine {
                playing: !cine.playing,
                ..cine
            });
            viewport.cine = Some(toggled.unwrap_or(started));
        }
        Action::CineSet { fps, looping, .. } => {
            let playing = viewport.cine.is_some_and(|cine| cine.playing);
            viewport.cine = Some(Cine::new(fps, looping, playing));
        }
        about_the_window => set_window(viewport, known, &about_the_window),
    }
}

/// Changes to the window. One made by hand ends automatic windowing; a preset uses the
/// window worked out for the slice shown.
fn set_window(viewport: &mut ViewportState, known: Option<PresetWindows>, action: &Action) {
    let by_hand = match *action {
        Action::AdjustWindow {
            d_center, d_width, ..
        } => WindowLevel::new(viewport.wl.center + d_center, viewport.wl.width + d_width),
        Action::SetWindow { wl, .. } => WindowLevel::new(wl.center, wl.width),
        Action::ApplyPreset { preset, .. } => {
            let automatic = preset == 0;
            if let Some(wl) = known.and_then(|windows| windows.get(preset).copied()) {
                (viewport.wl, viewport.wl_auto, viewport.wl_pending) = (wl, automatic, false);
            } else if automatic && known.is_none() {
                // Not decoded yet: take the automatic window when it is.
                (viewport.wl_auto, viewport.wl_pending) = (true, true);
            }
            return;
        }
        _ => return,
    };
    (viewport.wl, viewport.wl_auto, viewport.wl_pending) = (by_hand, false, false);
}
