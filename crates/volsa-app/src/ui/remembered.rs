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

/// What the window keeps from one run to the next: the preferences, and the pace at which
/// where the series were left is kept.
pub(super) struct Kept {
    prefs: crate::prefs::Kept,
    pace: Pace,
}

impl Kept {
    /// The preferences as they were read, with nothing kept of the places yet.
    pub(super) fn of(prefs: crate::prefs::Kept) -> Self {
        let pace = Pace::default();
        Self { prefs, pace }
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
        let now = Prefs::of(self.session.state(), arrangement);
        self.kept.prefs.keep(&now);
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
}
