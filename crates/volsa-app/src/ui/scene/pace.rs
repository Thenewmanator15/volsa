//! How finely the scene is drawn while it moves: as finely as the graphics device has shown
//! it can keep up with. A fast device draws the moving scene in full, so that nothing
//! changes when it comes to rest; a slow one draws it smaller and coarser, and in full once
//! it is still.

use std::time::Duration;

/// The longest the device may take over a picture drawn while the scene moves: half of a
/// sixtieth of a second, which leaves the other half for everything else in the window.
pub const MOVING_BUDGET: Duration = Duration::from_millis(8);
/// How long after a full picture that could not be timed, the device being busy, another
/// is drawn to be timed; and how many times that is tried before it is given up.
pub const TIME_AGAIN: Duration = Duration::from_millis(250);
pub const TIMING_TRIES: u32 = 8;
/// While the moving scene is drawn in full, one picture in this many is timed, so that
/// a view that has become slower to draw is noticed.
pub const TIME_EVERY: u32 = 30;
/// The ways a moving picture can be drawn, finest first: in full, at three quarters of the
/// size and fineness, and at a half.
const STEPS: [f32; 3] = [1.0, 0.75, 0.5];
/// How the time a picture takes falls as it is drawn smaller and coarser: by about this
/// power of the step. Measured on two devices: at a half it takes between a fifth and a
/// sixth of the time, at three quarters a little over half.
const COST_POWER: f32 = 2.3;

/// How finely to draw the scene while it moves, given how long the device took over the
/// last picture it drew in full, if it has drawn one: the finest way expected to take no
/// longer than [`MOVING_BUDGET`], and a half when nothing has been timed yet, or nothing
/// finer would keep up.
pub fn moving_quality(full_took: Option<Duration>) -> f32 {
    let Some(full) = full_took else {
        return 0.5;
    };
    let keeps_up =
        |step: &f32| full.as_secs_f32() * step.powf(COST_POWER) <= MOVING_BUDGET.as_secs_f32();
    STEPS.into_iter().find(keeps_up).unwrap_or(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_that_has_not_been_timed_draws_the_moving_scene_at_a_half() {
        assert_eq!(moving_quality(None), 0.5);
    }

    #[test]
    fn the_moving_scene_is_drawn_as_finely_as_the_device_keeps_up_with() {
        let after = |micros| moving_quality(Some(Duration::from_micros(micros)));
        assert_eq!(after(3_000), 1.0, "a fast device draws it in full");
        assert_eq!(after(8_000), 1.0, "up to the whole of the budget");
        assert_eq!(after(8_100), 0.75);
        assert_eq!(
            after(15_000),
            0.75,
            "which takes a little over half as long"
        );
        assert_eq!(after(16_000), 0.5);
        assert_eq!(after(60_000), 0.5, "and never coarser than a half");
        assert_eq!(after(0), 1.0);
    }
}
