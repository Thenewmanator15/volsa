//! The window keeping what the person set from one run to the next.

use std::time::{Duration, Instant};

use super::theme::ThemeChoice;
use super::{VolsaApp, capture};
use crate::action::Action;
use crate::places::PlacesAction;
use crate::prefs::{Arrangement, Prefs};

/// How long is left between one keeping of where the series were left and the next, while
/// the window is in use. They are kept too whenever a series leaves its viewport, and when the
/// window closes; this is for a Volsa that is stopped without closing.
const PACE: Duration = Duration::from_secs(30);

/// How long the preferences have to stay the same before they are written. Dragging the
/// window's edge changes its size every frame, and each of those is not a file written.
const SETTLE: Duration = Duration::from_secs(1);

/// What the window keeps from one run to the next: the preferences, those changed and not yet
/// written, and the pace at which where the series were left is kept.
pub(super) struct Kept {
    prefs: crate::prefs::Kept,
    settling: Settling,
    pace: Pace,
}

impl Kept {
    /// The preferences as they were read, with nothing kept of the places yet.
    pub(super) fn of(prefs: crate::prefs::Kept) -> Self {
        let (settling, pace) = (Settling::default(), Pace::default());
        Self {
            prefs,
            settling,
            pace,
        }
    }

    /// Write the preferences changed and not yet written: for when the window closes.
    pub(super) fn keep_now(&mut self) {
        if let Some(changed) = self.settling.take() {
            self.prefs.keep(&changed);
        }
    }
}

/// Preferences that have changed, and when they last changed.
#[derive(Debug, Default)]
struct Settling(Option<(Prefs, Instant)>);

impl Settling {
    /// Note the preferences as they are at `now`, and whether they are those written. Gives
    /// them once they have stayed the same for [`SETTLE`], and how long is left otherwise:
    /// `Duration::MAX` when there is nothing to write.
    fn look(&mut self, prefs: Prefs, written: bool, now: Instant) -> Result<Prefs, Duration> {
        match &self.0 {
            Some((waiting, _)) if *waiting == prefs => {}
            _ if written => {
                self.0 = None;
                return Err(Duration::MAX);
            }
            _ => self.0 = Some((prefs, now)),
        }
        let since = self.0.as_ref().map_or(now, |(_, since)| *since);
        let left = SETTLE.saturating_sub(now.saturating_duration_since(since));
        if left.is_zero() {
            self.take().ok_or(Duration::MAX)
        } else {
            Err(left)
        }
    }

    fn take(&mut self) -> Option<Prefs> {
        self.0.take().map(|(prefs, _)| prefs)
    }
}

/// When where the series were left was last asked to be kept.
#[derive(Debug, Default)]
struct Pace(Option<Instant>);

impl Pace {
    /// Whether it is time to keep them again at `now`; if it is, `now` is noted as the last
    /// time. The first time asked is never the time: the count starts there.
    fn due(&mut self, now: Instant) -> bool {
        let last = *self.0.get_or_insert(now);
        let due = now.saturating_duration_since(last) >= PACE;
        if due {
            self.0 = Some(now);
        }
        due
    }
}

impl VolsaApp {
    /// Put the state as the run before left it.
    pub(super) fn recall(&mut self) {
        let remembered = self.kept.prefs.remembered().clone();
        for action in remembered.actions(self.session.state()) {
            self.session.dispatch(action);
        }
    }

    /// Keep the preferences if they have changed, once the window has opened. While a button
    /// is held the window, the series list or the column may be being dragged to a new size,
    /// and nothing is written until it is let go.
    pub(super) fn remember(&mut self, ctx: &egui::Context) {
        let held = ctx.input(|input| input.pointer.any_down());
        if held || !self.screen.has_opened() {
            return;
        }
        self.screen.look(ctx);
        let arrangement = Arrangement {
            theme: self.bars.theme.name().to_owned(),
            ..self.screen.arrangement(ctx.zoom_factor())
        };
        let prefs = Prefs::of(self.session.state(), arrangement);
        let written = self.kept.prefs.holds(&prefs);
        match self.kept.settling.look(prefs, written, Instant::now()) {
            Ok(settled) => {
                self.kept.prefs.keep(&settled);
            }
            Err(left) if left < Duration::MAX => ctx.request_repaint_after(left),
            Err(_) => {}
        }
        // Where each series is left is kept at a slow pace, not at every slice scrolled to.
        if self.kept.pace.due(Instant::now()) {
            self.session.dispatch(Action::Places(PlacesAction::Keep));
        }
    }
}

/// The theme the window opens in: the one `kept` from the run before, or for a picture of the
/// window, the one it asks for.
pub(super) fn opening_theme(kept: &Arrangement, capturing: bool) -> ThemeChoice {
    let remembered = ThemeChoice::named(&kept.theme).unwrap_or_default();
    capture::theme(capturing, remembered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_kept_at_a_slow_pace_and_not_at_once() {
        let (mut pace, start) = (Pace::default(), Instant::now());
        assert!(!pace.due(start), "the count starts at the first frame");
        assert!(!pace.due(start + PACE / 2));
        assert!(pace.due(start + PACE));
        assert!(!pace.due(start + PACE + PACE / 2), "and starts again");
        assert!(pace.due(start + PACE * 2));
    }

    fn sized(width: f32) -> Prefs {
        Prefs {
            window: Some([width, 600.0]),
            ..Prefs::default()
        }
    }

    #[test]
    fn preferences_are_written_once_they_stay_the_same_for_a_second() {
        let (mut settling, start) = (Settling::default(), Instant::now());
        assert_eq!(settling.look(sized(800.0), true, start), Err(Duration::MAX));
        // The edge of the window is dragged: each frame a new size, none written.
        for frame in 1..30_u16 {
            let at = start + Duration::from_millis(u64::from(frame) * 16);
            let looked = settling.look(sized(800.0 + f32::from(frame)), false, at);
            assert_eq!(looked, Err(SETTLE), "frame {frame}");
        }
        let last = start + Duration::from_millis(29 * 16);
        let kept = settling.look(sized(829.0), false, last + SETTLE);
        assert_eq!(kept, Ok(sized(829.0)), "once it has stayed");
        let again = settling.look(sized(829.0), true, last + SETTLE * 2);
        assert_eq!(again, Err(Duration::MAX), "and not again");
    }
}
