//! Folding the side columns in a narrow window, and how wide each part of the window is
//! across: the series list at the left, the tab panel at the right, the pictures between.
//!
//! While the window is narrower than [`CRAMPED`], no more than one side column is open: when
//! the window first becomes that narrow both fold, and opening one, however it is opened, folds
//! the other. In a wider window they open and fold only when the person asks. Either way the
//! pictures keep [`PICTURES_LEAST`]: an open column gives way to them. A fold the window made
//! on its own is not remembered as the person's.

use super::super::{rail, shares, viewport};
use super::Screen;
use crate::ui::panel::Tab;

/// While the window is narrower than this, in points of the interface, no more than one side
/// column is open at a time.
pub const CRAMPED: f32 = 760.0;
/// The least of the window's width the pictures keep, in points of the interface.
pub const PICTURES_LEAST: f32 = 200.0;
/// What the middle of the window keeps clear around the pictures, across.
const MARGINS: f32 = 2.0 * viewport::GAP;
/// How many frames the window waits to take the size it was asked to open at before it folds
/// for the size it has.
const PATIENCE: u8 = 10;

/// How wide each side column is across, in points of the interface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Widths {
    /// The rail of scans at the left: its strip while it is folded.
    pub rail: f32,
    /// The tab panel at the right: its strip of icons while it is folded.
    pub panel: f32,
}

/// Which side columns are open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Open {
    /// Whether the series list is shown.
    rail: bool,
    /// Whether the panel is open.
    panel: bool,
}

/// How the side columns were when last settled.
#[derive(Debug, Clone, Copy)]
pub(super) struct Folds {
    /// Whether the window was narrower than [`CRAMPED`].
    cramped: bool,
    /// Which were open.
    was: Open,
    /// Whether the panel is folded because the window folded it, not the person.
    panel_by_window: bool,
    /// Whether the series list is folded because the window folded it, or did not open it for
    /// being narrow, and not the person.
    rail_by_window: bool,
    /// The width the window was asked to open at, until it has it, and how many frames it has
    /// been waited for.
    awaited: Option<(f32, u8)>,
}

impl Folds {
    /// Before the first frame, with the series list folded, by the window unless the person
    /// folded it (`rail_by_window`), and the panel `open` or not.
    pub(super) fn new(open: bool, rail_by_window: bool) -> Self {
        Self {
            cramped: false,
            was: Open {
                rail: false,
                panel: open,
            },
            panel_by_window: false,
            rail_by_window,
            awaited: None,
        }
    }
}

impl Screen {
    /// Fold what the window's being `window` points across calls for, once a frame, before
    /// the side columns are drawn.
    pub fn settle(&mut self, window: f32) {
        if self.awaiting(window) {
            return;
        }
        let cramped = window < CRAMPED;
        let now = self.open_now();
        if now.panel {
            self.folds.panel_by_window = false;
        }
        if now.rail {
            self.folds.rail_by_window = false;
        }
        if cramped {
            let was = self.folds.was;
            let (rail_opened, panel_opened) = (now.rail && !was.rail, now.panel && !was.panel);
            let (rail, panel) = (now.rail, now.panel);
            // On first becoming narrow only a column just opened stays open; after that, the one
            // open stays open unless the other was just opened. The panel wins a tie.
            let newly = !self.folds.cramped;
            let keep_panel = panel && (panel_opened || !newly && !rail_opened);
            let keep_rail = rail && !keep_panel && (rail_opened || !newly);
            if rail && !keep_rail {
                self.list_shown = false;
                self.folds.rail_by_window = true;
            }
            if panel && !keep_panel {
                self.panel.folded = true;
                self.folds.panel_by_window = true;
            }
        }
        self.folds.cramped = cramped;
        self.settled_as_they_are();
    }

    /// Whether a frame should follow to settle the side columns: the window is still being
    /// sized as it opens, or it is narrow with both open.
    pub fn unsettled(&self, window: f32) -> bool {
        let both = self.list_shown && !self.panel.folded;
        self.folds.awaited.is_some() || (window < CRAMPED && both)
    }

    /// How wide each side column is in a window `window` points across: as wide as it is set
    /// to be, or its strip while it is folded, unless that leaves the pictures less than
    /// [`PICTURES_LEAST`], when what is open gives way, each in proportion to its width.
    pub fn widths(&self, window: f32) -> Widths {
        let rail = if self.list_shown {
            shares::list_width(window, self.list_share())
        } else {
            rail::STRIP
        };
        let panel = self.panel.width(window);
        let room = (window - PICTURES_LEAST - MARGINS).max(0.0);
        let open_rail = if self.list_shown { rail } else { 0.0 };
        let open = open_rail + if self.panel.folded { 0.0 } else { panel };
        let fixed = rail + panel - open;
        if rail + panel <= room || open <= 0.0 {
            return Widths { rail, panel };
        }
        let scale = ((room - fixed).max(0.0) / open).min(1.0);
        Widths {
            rail: if self.list_shown { rail * scale } else { rail },
            panel: if self.panel.folded {
                panel
            } else {
                panel * scale
            },
        }
    }

    /// Show `tab`, unfolding the panel as the person asking for it does: in a narrow window
    /// the series list folds in its place.
    pub fn show_tab(&mut self, tab: Tab) {
        self.panel.show_tab(tab);
        self.folds.was.panel = false;
    }

    /// Fold the panel, or unfold it, as the person asks.
    pub fn toggle_panel(&mut self) {
        self.panel.toggle_fold();
    }

    /// Fold the series list, or unfold it, as the person asks.
    pub fn toggle_rail(&mut self) {
        self.list_shown = !self.list_shown;
        self.folds.rail_by_window = false;
    }

    /// Whether the panel is folded and the person folded it, which is what is remembered.
    pub(super) fn panel_folded_by_choice(&self) -> bool {
        self.panel.folded && !self.folds.panel_by_window
    }

    /// Whether the series list is folded and the person folded it, which is what is remembered.
    pub(super) fn rail_folded_by_choice(&self) -> bool {
        !self.list_shown && !self.folds.rail_by_window
    }

    /// Take the side columns as they are now as settled, so that none counts as just opened.
    fn settled_as_they_are(&mut self) {
        self.folds.was = self.open_now();
    }

    /// Take the series list as shown or not by the window as it opens, not by the person, unless
    /// the person left it folded: a tab asked for before then still counts as just opened.
    pub(super) fn rail_shown_as_it_opens(&mut self) {
        self.folds.was.rail = self.list_shown;
        self.folds.rail_by_window = !self.list_shown && !self.arrangement.rail_folded;
    }

    /// Which side columns are open now.
    fn open_now(&self) -> Open {
        Open {
            rail: self.list_shown,
            panel: !self.panel.folded,
        }
    }

    /// Wait for the window to be `window` points across, the size it was asked to open at,
    /// before folding anything for its size.
    pub(super) fn awaits(&mut self, window: f32) {
        self.folds.awaited = Some((window, 0));
    }

    /// Whether the window is still to take the size it was asked to open at, `window` points
    /// across being what it is this frame. It is waited for a few frames at most.
    fn awaiting(&mut self, window: f32) -> bool {
        let Some((asked, waited)) = self.folds.awaited else {
            return false;
        };
        let waiting = (window - asked).abs() > 1.0 && waited < PATIENCE;
        self.folds.awaited = waiting.then_some((asked, waited + 1));
        waiting
    }
}

#[cfg(test)]
#[path = "folds_tests.rs"]
mod tests;
