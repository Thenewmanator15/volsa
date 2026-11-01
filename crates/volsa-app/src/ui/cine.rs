//! Keeping time for film-like playback.

use std::time::{Duration, Instant};

use crate::state::Cine;

/// The most slices playback moves on in one frame. After a long pause (the window hidden, the
/// machine asleep) the rest of what is owed is dropped, not played in a burst.
pub const MAX_TICKS_AT_ONCE: u32 = 4;

/// How many slices playback at `fps` should have moved on between `last_tick` and `now`. The
/// speed is held between 1 and 60 a second.
pub fn cine_due(last_tick: Instant, now: Instant, fps: f32) -> u32 {
    let elapsed = now.saturating_duration_since(last_tick).as_secs_f64();
    whole(elapsed * f64::from(speed(fps)))
}

/// The speed playback really runs at for a speed asked for.
fn speed(fps: f32) -> f32 {
    Cine::new(fps, false, false).fps
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a count of ticks, never negative"
)]
fn whole(ticks: f64) -> u32 {
    ticks.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// When playback in one viewport last moved on.
#[derive(Debug, Clone, Copy, Default)]
pub struct Clock {
    last: Option<Instant>,
}

impl Clock {
    /// How many slices playback should move on now, and how long until it will next be due,
    /// so the window can sleep until then instead of redrawing all the time.
    pub fn advance(&mut self, now: Instant, fps: f32) -> (u32, Duration) {
        let period = Duration::from_secs_f64(1.0 / f64::from(speed(fps)));
        let last = self.last.unwrap_or(now);
        let owed = cine_due(last, now, fps);
        let ticks = owed.min(MAX_TICKS_AT_ONCE);
        let last = if owed > MAX_TICKS_AT_ONCE {
            now
        } else {
            last + period * ticks
        };
        self.last = Some(last);
        (
            ticks,
            period.saturating_sub(now.saturating_duration_since(last)),
        )
    }

    /// Forget the time, so that starting again owes nothing for the time stopped.
    pub fn stop(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn after(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn ten_a_second_owes_two_ticks_after_a_quarter_of_a_second() {
        let start = Instant::now();
        assert_eq!(cine_due(start, after(start, 250), 10.0), 2);
        assert_eq!(cine_due(start, start, 10.0), 0);
        assert_eq!(cine_due(start, after(start, 99), 10.0), 0);
        assert_eq!(cine_due(start, after(start, 100), 10.0), 1);
    }

    #[test]
    fn a_speed_outside_one_to_sixty_is_held_to_that_range() {
        let start = Instant::now();
        assert_eq!(
            cine_due(start, after(start, 2500), 0.1),
            2,
            "as at one a second"
        );
        assert_eq!(
            cine_due(start, after(start, 100), 1000.0),
            6,
            "as at sixty a second"
        );
        assert_eq!(
            cine_due(start, after(start, 1000), f32::NAN),
            10,
            "as at the usual speed"
        );
    }

    #[test]
    fn a_clock_that_has_gone_backwards_owes_nothing() {
        let start = Instant::now();
        assert_eq!(cine_due(after(start, 500), start, 10.0), 0);
    }

    #[test]
    fn playback_ticks_on_time_and_says_when_to_look_again() {
        let start = Instant::now();
        let mut clock = Clock::default();
        assert_eq!(
            clock.advance(start, 10.0),
            (0, Duration::from_millis(100)),
            "just started"
        );
        assert_eq!(
            clock.advance(after(start, 40), 10.0),
            (0, Duration::from_millis(60))
        );
        assert_eq!(
            clock.advance(after(start, 250), 10.0),
            (2, Duration::from_millis(50))
        );
        assert_eq!(
            clock.advance(after(start, 300), 10.0),
            (1, Duration::from_millis(100))
        );
        clock.stop();
        assert_eq!(
            clock.advance(after(start, 900), 10.0).0,
            0,
            "starting again owes nothing"
        );
    }

    #[test]
    fn a_long_pause_does_not_become_a_burst_of_ticks() {
        let start = Instant::now();
        let mut clock = Clock::default();
        let _ = clock.advance(start, 60.0);
        let (ticks, _) = clock.advance(after(start, 60_000), 60.0);
        assert_eq!(ticks, MAX_TICKS_AT_ONCE);
        assert_eq!(
            clock.advance(after(start, 60_001), 60.0).0,
            0,
            "the backlog is dropped"
        );
    }
}
