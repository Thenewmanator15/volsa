//! Keeping the stack viewports and the planes' position together: a viewport showing a
//! series of the scene's study goes to the slice nearest the position, and takes the
//! position with it when it is moved through its slices.

use volsa_core::{Patient, Point};
use volsa_volume::{nearest_slice, position_on_slice};

use super::{planes, viewport};
use crate::action::{Action, Effect};
use crate::planes::linked_placement;
use crate::state::AppState;

/// What was so before an action, to tell afterwards what it changed.
pub(super) struct Before {
    position: Option<Point<Patient>>,
    /// The viewport the action moves through its slices, if it is one that does.
    scrolled: Option<usize>,
    /// Whether the action is one after which every linked viewport is brought to the
    /// position whether that moved or not: the link turned on, or viewports brought into
    /// sight.
    bring: bool,
}

impl Before {
    pub(super) fn of(state: &AppState, action: &Action) -> Self {
        let scrolled = match *action {
            Action::Scroll { viewport, .. }
            | Action::SetSlice { viewport, .. }
            | Action::CineTick { viewport } => Some(viewport),
            _ => None,
        };
        Self {
            position: state.planes.position,
            scrolled,
            bring: matches!(action, Action::SetLinked(true) | Action::SetLayout(_)),
        }
    }
}

/// Keep the link after an action. A viewport that was moved through its slices takes the
/// position onto the slice it now shows; then, if the position moved, every other linked
/// viewport in sight goes to the slice nearest it. The viewport that was moved is left where
/// the person put it, even if the position could not go all the way there.
pub(super) fn keep(state: &mut AppState, before: &Before) -> Vec<Effect> {
    if let Some(at) = before.scrolled {
        carry(state, at);
    }
    let Some(position) = state.planes.position else {
        return Vec::new();
    };
    if !before.bring && before.position == Some(position) {
        return Vec::new();
    }
    let mut effects = Vec::new();
    for at in (0..state.layout.count()).filter(|at| Some(*at) != before.scrolled) {
        if let Some(placement) = linked_placement(state, at) {
            let slice = nearest_slice(&placement, state.slices_in(at), position);
            effects.extend(viewport::go_to(state, at, slice));
        }
    }
    effects
}

/// Move the position onto the slice a linked viewport shows, keeping its place across the
/// slice.
fn carry(state: &mut AppState, at: usize) {
    let onto = || {
        let placement = linked_placement(state, at)?;
        let slice = state.viewports.get(at)?.slice;
        let ([column, row], _) = position_on_slice(&placement, slice, state.planes.position?)?;
        let slice = f64::from(u32::try_from(slice).ok()?);
        Some(placement.apply(Point::new(column, row, slice)))
    };
    if let Some(point) = onto() {
        planes::set_position(state, point);
    }
}
