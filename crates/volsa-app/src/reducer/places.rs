//! Keeping each series where the person left it: asking for what is kept when an export has
//! been read, putting a series back as it was left when it is shown, and asking for a place to
//! be written when its series leaves a viewport.
//!
//! Two rules hold throughout. A kept place never takes the place of what the person has done:
//! it is put back only on a viewport still as choosing its series left it. And nothing is
//! written until what is kept for the export has been read: before that, what would be
//! written is Volsa's own starting place, over the one the person left.

use volsa_core::{SeriesId, StudyId};

use super::{link, viewport};
use crate::action::{Action, Effect};
use crate::labels::study_of;
use crate::places::{Place, PlacesAction};
use crate::state::{AppState, VIEWPORTS, ViewportState};

/// What kind of action is being applied, as far as the places are concerned.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// One that cannot change which series a viewport shows, nor asks for places to be kept.
    Other,
    /// The export has been read through, and Volsa may have chosen the series to open first.
    Indexed,
    /// The person chose a series for this viewport.
    Chosen(usize),
    /// Places are to be kept now, whether or not a series leaves its viewport.
    Keep,
    /// Any other that can change which series a viewport shows.
    Changing,
}

/// What was so before an action, to tell afterwards which series it took out of a viewport.
pub(super) struct Before {
    kind: Kind,
    /// How many exports had been opened.
    opened: u64,
    /// The series each viewport showed.
    shown: [Option<SeriesId>; VIEWPORTS],
    /// The place to keep for each of them if it leaves now: `None` where there is nothing to
    /// write, or nothing may be written yet.
    due: [Option<Place>; VIEWPORTS],
}

impl Before {
    pub(super) fn of(state: &AppState, action: &Action) -> Self {
        let kind = match action {
            Action::IndexDone => Kind::Indexed,
            Action::SelectSeries { viewport, .. } => Kind::Chosen(*viewport),
            Action::Places(PlacesAction::Keep) => Kind::Keep,
            Action::OpenPath(_) | Action::StepVolume { .. } | Action::Places(_) => Kind::Changing,
            _ => Kind::Other,
        };
        // An action of no concern here is looked at no further: most are of that kind.
        let seen = kind != Kind::Other;
        let viewports = state.viewports.each_ref();
        Self {
            kind,
            opened: state.opened,
            shown: viewports.map(|shown| shown.series.clone().filter(|_| seen)),
            due: viewports.map(|shown| seen.then(|| due(state, shown)).flatten()),
        }
    }
}

/// What the workers report of the places kept. What was read for an export that is no longer
/// the one open is ignored.
pub(super) fn reduce(state: &mut AppState, action: PlacesAction) -> Vec<Effect> {
    match action {
        PlacesAction::Loaded {
            opened,
            places,
            first,
        } if opened == state.opened => loaded(state, places, first),
        // Keeping is done after the action, with every other leaving: see `keep`.
        PlacesAction::Loaded { .. } | PlacesAction::Keep => Vec::new(),
    }
}

/// Keep the places after an action: ask for what is kept once an export has been read
/// through; ask for the place of a series that has left its viewport to be written, and of
/// every series shown when the action asks for that; put a series the person has just chosen
/// back as it was left; and keep the file's first series of the study the one the first
/// viewport shows.
pub(super) fn keep(state: &mut AppState, before: &Before) -> Vec<Effect> {
    if before.kind == Kind::Other {
        return Vec::new();
    }
    let mut effects = Vec::new();
    let now = state.viewports.each_ref().map(|shown| shown.series.clone());
    let changes = before.shown.iter().zip(&now).zip(&before.due);
    let left = changes.filter(|((was, now), _)| before.kind == Kind::Keep || was != now);
    let places: Vec<(SeriesId, Place)> = left
        .filter_map(|((was, _), due)| was.clone().zip(*due))
        .collect();
    if before.opened == state.opened {
        state.places.kept.extend(places.iter().cloned());
    }
    match before.kind {
        Kind::Indexed => effects.extend(indexed(state, before)),
        Kind::Chosen(at) => {
            if at == 0 {
                state.places.automatic = None;
            }
            // The series shown chosen again starts afresh, as it always has.
            if before.shown.get(at) != now.get(at) {
                effects.extend(put_back(state, at));
            }
        }
        _ => {}
    }
    let first = first_due(state);
    if !places.is_empty() || first.is_some() {
        effects.push(Effect::StorePlaces { places, first });
    }
    effects
}

/// The export has been read through: note the series Volsa chose itself to open first, if it
/// chose one, and ask for what is kept for the export's series and studies.
fn indexed(state: &mut AppState, before: &Before) -> Option<Effect> {
    let was_free = before.shown.first().is_some_and(Option::is_none);
    let chosen = state
        .viewports
        .first()
        .and_then(|shown| shown.series.clone());
    if was_free {
        state.places.automatic = chosen;
    }
    let index = state.index.as_ref()?;
    let series: Vec<SeriesId> = index.series.iter().map(|s| s.info.id.clone()).collect();
    let studies = index.studies.iter().map(|study| study.id.clone()).collect();
    (!series.is_empty()).then_some(Effect::LoadPlaces {
        opened: state.opened,
        series,
        studies,
    })
}

/// Take in what was kept for the open export. Each place is held to what its series can
/// show, and one that cannot be a place of it is left out. Then the series kept as the
/// study's first takes the place of the one Volsa chose, and every viewport still as choosing
/// its series left it is put back as that series was left.
fn loaded(
    state: &mut AppState,
    places: Vec<(SeriesId, Place)>,
    first: Option<SeriesId>,
) -> Vec<Effect> {
    let fitted = |(series, place): (SeriesId, Place)| {
        let slices = state.series(&series)?.slices.len();
        Some((series, place.fitted(slices)?))
    };
    state.places.kept = places.into_iter().filter_map(fitted).collect();
    state.places.first = first.filter(|series| state.series(series).is_some());
    state.places.read = true;
    let mut effects = open_first(state);
    for at in 0..VIEWPORTS {
        effects.extend(put_back(state, at));
    }
    effects
}

/// Show the series kept as the study's first in the first viewport, if what that viewport
/// shows is still Volsa's own choice, untouched. A series the person chose stays.
fn open_first(state: &mut AppState) -> Vec<Effect> {
    let Some(first) = state.places.first.clone() else {
        return Vec::new();
    };
    let shown = state
        .viewports
        .first()
        .and_then(|shown| shown.series.clone());
    let own = shown.is_some() && shown == state.places.automatic && untouched(state, 0);
    if !own || shown.as_ref() == Some(&first) {
        return Vec::new();
    }
    state.places.automatic = None;
    // The viewport the person made active meanwhile stays the active one.
    let active = state.active;
    let (viewport, series) = (0, first);
    let effects = viewport::reduce(state, Action::SelectSeries { viewport, series });
    state.active = active;
    effects
}

/// Put the series a viewport shows back as it was left, if a place is kept for it and the
/// viewport is still as choosing the series left it. A picture in colour has no window to
/// put back. A linked viewport takes the planes' position with it, as when it is scrolled.
fn put_back(state: &mut AppState, at: usize) -> Vec<Effect> {
    let kept = || {
        let series = state.viewports.get(at)?.series.as_ref()?;
        let place = state.places.kept.get(series).copied()?;
        let colour = state.series(series)?.info.colour;
        (state.places.read && untouched(state, at)).then_some((place, colour))
    };
    let Some((place, colour)) = kept() else {
        return Vec::new();
    };
    let Some(shown) = state.viewports.get_mut(at) else {
        return Vec::new();
    };
    shown.view = place.view();
    if let Some(window) = place.window().filter(|_| !colour) {
        (shown.wl, shown.wl_auto, shown.wl_pending) = (window, false, false);
    }
    let effects = viewport::go_to(state, at, place.slice);
    link::carry(state, at);
    effects
}

/// Whether a viewport is as choosing its series left it: on the middle slice, fitted, in
/// Volsa's own window. One showing nothing is not.
fn untouched(state: &AppState, at: usize) -> bool {
    let shown = state.viewports.get(at);
    let slices = shown
        .and_then(|shown| shown.series.as_ref())
        .and_then(|series| state.series(series))
        .map(|listed| listed.slices.len());
    shown.zip(slices).is_some_and(|(shown, slices)| {
        shown.cine.is_none() && Place::of(shown) == Place::fresh(slices)
    })
}

/// The place to write for the series a viewport shows, were it to leave now: where it is, if
/// that is not what the file holds for it already. `None` until what is kept has been read.
fn due(state: &AppState, shown: &ViewportState) -> Option<Place> {
    if !state.places.read {
        return None;
    }
    let series = shown.series.as_ref()?;
    let slices = state.series(series)?.slices.len();
    let now = Place::of(shown);
    let kept = state.places.kept.get(series).copied();
    (now != kept.unwrap_or_else(|| Place::fresh(slices))).then_some(now)
}

/// The study and series to write as the study's first: the series the first viewport shows,
/// when the file does not hold it already. `None` until what is kept has been read, so that
/// the series Volsa chose itself is never written over the one the person left; and that
/// series is not written at all while the file holds none and the person has chosen none.
fn first_due(state: &mut AppState) -> Option<(StudyId, SeriesId)> {
    let shown = state.viewports.first()?.series.clone()?;
    let places = &state.places;
    let own = places.first.is_none() && places.automatic.as_ref() == Some(&shown);
    if !places.read || own || places.first.as_ref() == Some(&shown) {
        return None;
    }
    let study = study_of(state, &shown)?;
    state.places.first = Some(shown.clone());
    Some((study, shown))
}
