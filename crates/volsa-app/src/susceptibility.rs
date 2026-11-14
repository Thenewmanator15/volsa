//! A susceptibility map shown in place of the phase of an SWI scan: when one can be made, what
//! is written over the pictures while it is shown, its window, and the unit its values are read
//! in. The map is worked out by `volsa_segment::susceptibility`; here it is only offered and
//! said.

use volsa_core::{SeriesId, SeriesInfo, WindowLevel};
use volsa_segment::{PhaseFacts, SUSCEPTIBILITY_WINDOW_PPB};

use crate::profile::{Unit, unit_of};
use crate::scene::MapState;
use crate::state::AppState;

/// What is written over every picture that shows the map.
pub const NOTE: &str = "Susceptibility map (approximate), worked out from this phase";
/// The unit the map's values are read in: parts per billion.
pub const UNIT: &str = "ppb";

/// Why not: the scene has no volume built yet.
const NO_VOLUME: &str = "A susceptibility map needs this scan's 3D view. Open the phase \
                         series of an SWI scan, or wait for its 3D view.";
/// Why not: the series is not named as phase.
const NOT_PHASE: &str = "This series is not named as phase. A susceptibility map is worked \
                         out from the phase series of an SWI scan.";
/// Why not: its files do not say what the map needs.
const NO_FACTS: &str = "This scan does not say its echo time and field strength, which the map \
                        needs.";
/// Why not: one is being worked out.
const BUSY: &str = "A susceptibility map is being worked out. Wait for it to finish.";
/// Why not: one is shown already.
const SHOWN: &str = "A susceptibility map is shown already.";
/// Why there is no map to go back from.
const NO_MAP: &str = "No susceptibility map is shown.";

/// Whether a series' description names it as phase: it holds `PHA` or `phase`, in any case,
/// as GE's `FILT_PHA: 3D Ax eSWAN` does.
pub fn is_phase(info: &SeriesInfo) -> bool {
    // "phase" holds "pha" too.
    info.description.to_lowercase().contains("pha")
}

/// What the map needs to know of a scan, from its files.
pub fn facts(info: &SeriesInfo) -> PhaseFacts {
    PhaseFacts {
        echo_ms: info.scan.echo_ms,
        field_t: info.scan.field_t,
    }
}

/// Whether a susceptibility map can be worked out now from the series in the scene; if not,
/// why. It can when the scene shows a series named as phase whose volume is built, whose files
/// give its echo time and field strength, and no map is being worked out or shown.
///
/// # Errors
///
/// The sentence that says why not.
pub fn can_make_map(state: &AppState) -> Result<(), &'static str> {
    match state.scene.map {
        MapState::Making => return Err(BUSY),
        MapState::Shown => return Err(SHOWN),
        MapState::Off => {}
    }
    let info = state.scene.series.as_ref().and_then(|id| state.series(id));
    let info = info.map(|listed| &listed.info);
    if info.is_some_and(|info| !is_phase(info)) {
        return Err(NOT_PHASE);
    }
    let Some(info) = info.filter(|_| crate::planes::grid(state).is_some()) else {
        return Err(NO_VOLUME);
    };
    let facts = facts(info);
    let given = |value: Option<f64>| value.is_some_and(|value| value > 0.0);
    if !(given(facts.echo_ms) && given(facts.field_t)) {
        return Err(NO_FACTS);
    }
    Ok(())
}

/// Whether the scene can go back from a map to the phase; if not, why.
///
/// # Errors
///
/// The sentence that says why not.
pub fn can_go_back(state: &AppState) -> Result<(), &'static str> {
    if state.scene.map == MapState::Shown {
        Ok(())
    } else {
        Err(NO_MAP)
    }
}

/// Whether the pictures of `series` show a susceptibility map in place of its own values.
pub fn shown(state: &AppState, series: &SeriesId) -> bool {
    state.scene.map == MapState::Shown && state.scene.series.as_ref() == Some(series)
}

/// Whether the scene shows a susceptibility map.
pub fn in_the_scene(state: &AppState) -> bool {
    state.scene.map == MapState::Shown
}

/// The window the map is shown through: centred on 0, as far either way as
/// [`SUSCEPTIBILITY_WINDOW_PPB`].
pub fn window() -> WindowLevel {
    WindowLevel::new(0.0, 2.0 * SUSCEPTIBILITY_WINDOW_PPB)
}

/// The unit the values of `series` are read in: parts per billion while it shows a map, and
/// otherwise the one its files give.
pub fn unit<'a>(state: &AppState, series: &SeriesId, info: &'a SeriesInfo) -> Unit<'a> {
    if shown(state, series) {
        Unit::Named(UNIT)
    } else {
        unit_of(info)
    }
}
