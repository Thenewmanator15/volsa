//! What Voxa remembers between runs of how the person had it set: the window's size, the size
//! of the interface, how wide the series list and the tab panel are, which tab is open and
//! whether either column is folded, whether the window is light, dark or as the system is set,
//! how many viewports, and three switches. Nothing about any scan, and nothing that says which
//! scans were opened.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use voxa_render::Overlay;

use crate::action::Action;
use crate::state::{AppState, Layout};

/// The name of the file within the settings folder.
pub const FILE: &str = "preferences.json";
/// The smallest window Voxa opens at, in points at the interface's usual size; a remembered
/// size smaller than this is not taken.
pub const SMALLEST_WINDOW: [f32; 2] = [640.0, 480.0];
/// The interface sizes a remembered one is held between: 1 is its usual size, 2 twice that.
pub const INTERFACE_SIZES: RangeInclusive<f32> = 0.5..=3.0;
/// The share of the window's width the series list starts with.
pub const LIST_SHARE: f32 = 1.0 / 6.0;
/// The share of the middle of the window the column beside the large part started with in
/// Voxa 0.6.0, whose window had a column of pictures where the tab panel is now.
pub const COLUMN_SHARE: f32 = 0.3;
/// The share of the window's width the tab panel starts with, until the person drags it: what
/// the column beside the large part took of what the series list left, at its starting widths.
pub const PANEL_SHARE: f32 = COLUMN_SHARE * (1.0 - LIST_SHARE);
/// The most of the window's width the tab panel takes, however wide it was dragged.
pub const PANEL_MOST_SHARE: f32 = 0.4;
/// The tab the panel opens on when none is remembered, by the name it is kept under.
pub const FIRST_TAB: &str = "View";
/// The theme the window starts in, by the name it is kept under: the system's.
pub const START_THEME: &str = "system";
/// The themes a remembered one is held between, by the names they are kept under.
pub const THEMES: [&str; 3] = [START_THEME, "light", "dark"];

/// What is remembered. A file that lacks a part has that part as Voxa starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a settings file of independent switches, each kept by its own name"
)]
pub struct Prefs {
    /// The window's size inside its frame, in points at the interface's usual size; `None`
    /// for the size Voxa starts at.
    pub window: Option<[f32; 2]>,
    /// How large the interface is drawn: 1 for its usual size, 1.5 for half as large again.
    pub interface_size: f32,
    /// The share of the window's width the series list takes.
    pub list_share: f32,
    /// The share of the middle of the window the column beside the large part took, in a
    /// settings file of Voxa 0.6.0: read to give the tab panel its width, and never written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_share: Option<f32>,
    /// The share of the window's width the tab panel takes; `None` until the person has dragged
    /// it to one, or a settings file of Voxa 0.6.0 gave its column's width.
    pub panel_share: Option<f32>,
    /// Whether the tab panel is folded to a strip of icons.
    pub panel_folded: bool,
    /// Whether the series list is folded to its strip, as the person left it: a fold the window
    /// made because it was narrow is not kept.
    pub rail_folded: bool,
    /// The tab last open, by the name it is kept under: `View`, `Measure`, `Scene`, `Labels` or
    /// `Tools`. A name that is not one of these opens the first.
    pub tab: String,
    /// How many viewports: 1, 2 or 4.
    pub viewports: u8,
    /// Whether the patient's left is shown on the right, as radiology has it.
    pub radiological: bool,
    /// Whether the viewports of a study are kept on the planes' position.
    pub linked: bool,
    /// How labels are drawn over the planes: `off`, `outline` or `filled`.
    pub overlay: String,
    /// The theme the window is drawn in: `system`, `light` or `dark`. A name that is not one of
    /// these is the system's.
    pub theme: String,
}

/// How the window itself is set out, which the state does not hold: its size, the size of
/// the interface, how wide the person dragged the series list and the tab panel, which tab
/// is open, and which theme it is drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrangement {
    /// The window's size inside its frame, in points at the interface's usual size; `None`
    /// for the size Voxa starts at.
    pub window: Option<[f32; 2]>,
    /// How large the interface is drawn: 1 for its usual size.
    pub interface_size: f32,
    /// The share of the window's width the series list takes.
    pub list_share: f32,
    /// The share of the window's width the tab panel takes, once the person has chosen one.
    pub panel_share: Option<f32>,
    /// Whether the tab panel is folded to a strip of icons.
    pub panel_folded: bool,
    /// Whether the series list is folded to its strip, as the person left it.
    pub rail_folded: bool,
    /// The tab last open, by the name it is kept under.
    pub tab: String,
    /// The theme the window is drawn in, by the name it is kept under.
    pub theme: String,
}

impl Default for Arrangement {
    fn default() -> Self {
        Self {
            window: None,
            interface_size: 1.0,
            list_share: LIST_SHARE,
            panel_share: None,
            panel_folded: false,
            rail_folded: false,
            tab: FIRST_TAB.to_owned(),
            theme: START_THEME.to_owned(),
        }
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Self::of(&AppState::default(), Arrangement::default())
    }
}

fn layout_of(viewports: u8) -> Option<Layout> {
    match viewports {
        1 => Some(Layout::One),
        2 => Some(Layout::TwoAcross),
        4 => Some(Layout::TwoByTwo),
        _ => None,
    }
}

fn overlay_of(word: &str) -> Option<Overlay> {
    match word {
        "off" => Some(Overlay::Off),
        "outline" => Some(Overlay::Outline),
        "filled" => Some(Overlay::Filled),
        _ => None,
    }
}

impl Prefs {
    /// The preferences as the state has them, with the window set out as `arrangement`.
    pub fn of(state: &AppState, arrangement: Arrangement) -> Self {
        let viewports = match state.layout {
            Layout::One => 1,
            Layout::TwoAcross => 2,
            Layout::TwoByTwo => 4,
        };
        let overlay = match state.labels.overlay {
            Overlay::Off => "off",
            Overlay::Outline => "outline",
            Overlay::Filled => "filled",
        };
        Self {
            window: arrangement.window,
            interface_size: arrangement.interface_size,
            list_share: arrangement.list_share,
            column_share: None,
            panel_share: arrangement.panel_share,
            panel_folded: arrangement.panel_folded,
            rail_folded: arrangement.rail_folded,
            tab: arrangement.tab,
            theme: arrangement.theme,
            viewports,
            radiological: state.radiological,
            linked: state.planes.linked,
            overlay: overlay.to_owned(),
        }
    }

    /// How these preferences have the window set out.
    pub fn arrangement(&self) -> Arrangement {
        Arrangement {
            window: self.window,
            interface_size: self.interface_size,
            list_share: self.list_share,
            panel_share: self.panel_share,
            panel_folded: self.panel_folded,
            rail_folded: self.rail_folded,
            tab: self.tab.clone(),
            theme: self.theme.clone(),
        }
    }

    /// What to ask of the state to put it as these preferences have it: an action for each
    /// thing that differs, and none for a part that is not a value Voxa knows.
    pub fn actions(&self, state: &AppState) -> Vec<Action> {
        let now = Self::of(state, Arrangement::default());
        let layout = layout_of(self.viewports).filter(|_| self.viewports != now.viewports);
        let overlay = overlay_of(&self.overlay).filter(|_| self.overlay != now.overlay);
        let flipped = self.radiological != now.radiological;
        let linked = (self.linked != now.linked).then_some(self.linked);
        let asked = [
            layout.map(Action::SetLayout),
            flipped.then_some(Action::ToggleRadiological),
            linked.map(Action::SetLinked),
            overlay.map(Action::SetOverlay),
        ];
        asked.into_iter().flatten().collect()
    }

    /// These preferences with whatever is not a value Voxa knows put back as Voxa starts, and
    /// an interface size too small or too large to work in held to one that can be.
    fn sound(mut self) -> Self {
        let start = Self::default();
        let big_enough = |[wide, high]: [f32; 2]| {
            wide.is_finite()
                && high.is_finite()
                && wide >= SMALLEST_WINDOW[0]
                && high >= SMALLEST_WINDOW[1]
        };
        self.window = self.window.filter(|size| big_enough(*size));
        let (least, most) = INTERFACE_SIZES.into_inner();
        self.interface_size = if self.interface_size.is_finite() && self.interface_size > 0.0 {
            self.interface_size.clamp(least, most)
        } else {
            start.interface_size
        };
        let a_share = |share: f32| share > 0.0 && share < 1.0;
        if !a_share(self.list_share) {
            self.list_share = start.list_share;
        }
        self.panel_share = self.panel_share.filter(|share| a_share(*share));
        // The column of pictures of 0.6.0 gives the tab panel in its place its width, unless the
        // person has dragged the panel: what the column took of what the series list left.
        let column = self.column_share.take().filter(|share| a_share(*share));
        let carried = column.map(|share| (share * (1.0 - self.list_share)).min(PANEL_MOST_SHARE));
        self.panel_share = self.panel_share.or(carried);
        if layout_of(self.viewports).is_none() {
            self.viewports = start.viewports;
        }
        if overlay_of(&self.overlay).is_none() {
            self.overlay = start.overlay;
        }
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = start.theme;
        }
        self
    }
}

/// The preferences kept in `folder`. A file that is missing, cannot be read or is not what
/// Voxa wrote gives the preferences Voxa starts with.
pub fn load(folder: &Path) -> Prefs {
    let text = std::fs::read_to_string(folder.join(FILE)).unwrap_or_default();
    serde_json::from_str::<Prefs>(&text)
        .unwrap_or_default()
        .sound()
}

/// Keep these preferences in `folder`, which is made if it is not there.
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub fn save(folder: &Path, prefs: &Prefs) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(prefs)?;
    std::fs::create_dir_all(folder)?;
    // Written beside the file and then moved over it, so a failure half way leaves the old one.
    let (file, beside) = (folder.join(FILE), folder.join(format!("{FILE}.new")));
    std::fs::write(&beside, text)?;
    std::fs::rename(beside, file)
}

/// The preferences of this run and where they are kept: read once as Voxa starts, and
/// written again whenever they have changed.
pub struct Kept {
    folder: Option<PathBuf>,
    /// What was read, or what was last written.
    last: Prefs,
    /// Whether a failure to write has been logged, so that it is logged once.
    failed: bool,
}

impl Kept {
    /// Read what is kept in `folder`. With no folder nothing is read and nothing is written:
    /// a run that takes a picture of the window is one such.
    pub fn new(folder: Option<PathBuf>) -> Self {
        let last = folder.as_deref().map(load).unwrap_or_default();
        Self {
            folder,
            last,
            failed: false,
        }
    }

    /// What was remembered from the run before.
    pub fn remembered(&self) -> &Prefs {
        &self.last
    }

    /// Keep `now` if it differs from what is kept. True when it was written.
    pub fn keep(&mut self, now: &Prefs) -> bool {
        let Some(folder) = &self.folder else {
            return false;
        };
        if *now == self.last {
            return false;
        }
        self.last = now.clone();
        let written = save(folder, now).is_ok();
        if !written && !std::mem::replace(&mut self.failed, true) {
            tracing::warn!("preferences could not be written; they will not be there next time");
        }
        written
    }
}

#[cfg(test)]
mod carried_tests;
#[cfg(test)]
mod tests;
