//! Whether a picture of the window is of the 3D scene at rest or while it moves, and how long it
//! waits and how it keeps the scene moving.

use super::SETTLE_FRAMES;
use crate::action::Action;

/// Whether a picture is of the 3D scene at rest or while it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// At rest, drawn in full.
    Still,
    /// Kept moving, drawn as it is while it moves.
    Moving,
}

/// How many frames the window shows what is wanted before its picture is taken: twice as
/// many for a picture of the scene moving, which is first left to come to rest.
pub fn frames_before(scene: Motion) -> u32 {
    if scene == Motion::Moving {
        2 * SETTLE_FRAMES
    } else {
        SETTLE_FRAMES
    }
}

/// For a picture of the scene while it moves, asked for by the word `moving`: after
/// `settled` frames at rest, a turn of half a point one way on one frame and back on the
/// next, which keeps the scene moving and shows the same view. Nothing for the first
/// frames, in which the scene comes to rest and is drawn in full.
pub fn turn_at(scene: Motion, settled: u32) -> Option<Action> {
    let way = if settled.is_multiple_of(2) { 0.5 } else { -0.5 };
    (scene == Motion::Moving && settled >= SETTLE_FRAMES + SETTLE_FRAMES / 2)
        .then_some(Action::SceneOrbit { by: [way, 0.0] })
}
