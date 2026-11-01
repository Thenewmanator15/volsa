//! What a screen reader is told of a viewport, since a picture has no words of its own.

use crate::naming::plain_name;
use crate::state::{AppState, Layout};

/// What a screen reader calls viewport `at`: the plane its slices lie in, which slice it shows
/// and of how many, then the scan's plain name, as `Axial slice 88 of 176, T2, axial`. A scan
/// whose files do not say its plane has `Slice 88 of 176`. A viewport that shows nothing says
/// so. With several viewports in the window the one the keys act on ends with `, selected`.
pub fn said(state: &AppState, at: usize) -> String {
    let mut said = shown(state, at).unwrap_or_else(|| nothing(state, at));
    if at == state.active && state.layout != Layout::One {
        said.push_str(", selected");
    }
    said
}

/// What viewport `at` shows, if it shows a series that is listed.
fn shown(state: &AppState, at: usize) -> Option<String> {
    let viewport = state.viewports.get(at)?;
    let listed = state.series(viewport.series.as_ref()?)?;
    let which = format!("slice {} of {}", viewport.slice + 1, listed.slices.len());
    let which = match listed.info.scan.plane {
        Some(facing) => format!("{} {which}", facing.words()),
        None => capitalised(&which),
    };
    Some(format!("{which}, {}", plain_name(&listed.info)))
}

/// What is said of a viewport that shows no slice: what it writes over itself.
fn nothing(state: &AppState, at: usize) -> String {
    super::message(state, at, false).unwrap_or_else(|| "No series is shown here".to_owned())
}

/// The words with a capital first letter.
fn capitalised(words: &str) -> String {
    let mut letters = words.chars();
    let first = letters.next().map(|letter| letter.to_ascii_uppercase());
    first.into_iter().chain(letters).collect()
}
