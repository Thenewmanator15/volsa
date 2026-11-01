//! Stepping between the volumes of a series that holds several.

use volsa_core::SeriesId;

use super::settle_window;
use super::viewport::moved;
use crate::action::Effect;
use crate::state::AppState;

/// Show, in viewport `at`, the volume `by` places on from the one it shows, stopping at the
/// first and the last. The slice and the view stay; a window set by hand stays, and one that
/// was Volsa's own choice is chosen again for the new volume.
pub(super) fn step(state: &mut AppState, at: usize, by: i32) -> Vec<Effect> {
    let Some(series) = other_volume(state, at, by) else {
        return Vec::new();
    };
    let count = state
        .series(&series)
        .map_or(0, |listed| listed.slices.len());
    let Some(viewport) = state.viewports.get_mut(at) else {
        return Vec::new();
    };
    viewport.series = Some(series.clone());
    viewport.slice = viewport.slice.min(count.saturating_sub(1));
    viewport.wl_pending = viewport.wl_auto;
    settle_window(viewport, &state.windows);
    vec![Effect::RequestFrames {
        series,
        around: viewport.slice,
    }]
}

/// The volume `by` places on from the one viewport `at` shows. `None` when the viewport shows
/// no volume of a series that holds several, or is at that end already.
fn other_volume(state: &AppState, at: usize, by: i32) -> Option<SeriesId> {
    let shown = state.viewports.get(at)?.series.as_ref()?;
    let set = state.series(shown)?.info.set.as_ref()?;
    let to = moved(set.at, by).min(set.count.saturating_sub(1));
    if to == set.at {
        return None;
    }
    let listed = &state.index.as_ref()?.series;
    let is_it = |other: &&volsa_dicom::IndexedSeries| {
        let found = other.info.set.as_ref();
        found.is_some_and(|found| found.of == set.of && found.at == to)
    };
    listed.iter().find(is_it).map(|found| found.info.id.clone())
}
