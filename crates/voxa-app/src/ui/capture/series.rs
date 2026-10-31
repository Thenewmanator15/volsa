//! Which series a picture of the window asks to have shown: the first listed, or the first whose
//! description holds some words.

use voxa_core::SeriesId;

use crate::action::Action;
use crate::state::AppState;

/// For a picture that asks for the first series listed, whatever Voxa would have opened: what
/// shows it. `None` until the index is finished, which is when that series' thumbnail is there.
pub fn first_listed(state: &AppState, has_thumbnail: impl Fn(&SeriesId) -> bool) -> Option<Action> {
    let series = state.index.as_ref()?.series.first()?.info.id.clone();
    has_thumbnail(&series).then_some(Action::SelectSeries {
        viewport: 0,
        series,
    })
}

/// The words a picture asks for a series by, from the variable's `value`: none when it is
/// not set, or set to nothing.
pub fn asked_for(value: Option<String>) -> Option<String> {
    value.filter(|words| !words.trim().is_empty())
}

/// For a picture that asks for a series by `words` from its description: what shows the
/// first listed whose description holds them. `None` until that series' thumbnail is there.
pub fn listed_as(
    state: &AppState,
    words: &str,
    has_thumbnail: impl Fn(&SeriesId) -> bool,
) -> Option<Action> {
    let index = state.index.as_ref()?;
    let listed = index
        .series
        .iter()
        .find(|listed| listed.info.description.contains(words))?;
    let series = listed.info.id.clone();
    has_thumbnail(&series).then_some(Action::SelectSeries {
        viewport: 0,
        series,
    })
}
