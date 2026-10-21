//! What Voxa remembers between runs of how the person had it set: the window's size, how many
//! viewports, and three switches. Nothing about any scan, and nothing that says which scans
//! were opened.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use voxa_render::Overlay;

use crate::action::Action;
use crate::state::{AppState, Layout};

/// The name of the file within the settings folder.
pub const FILE: &str = "preferences.json";
/// The smallest window Voxa opens at; a remembered size smaller than this is not taken.
pub const SMALLEST_WINDOW: [f32; 2] = [900.0, 600.0];

/// What is remembered. A file that lacks a part has that part as Voxa starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The window's size inside its frame, in points; `None` for the size Voxa starts at.
    pub window: Option<[f32; 2]>,
    /// How many viewports: 1, 2 or 4.
    pub viewports: u8,
    /// Whether the patient's left is shown on the right, as radiology has it.
    pub radiological: bool,
    /// Whether the viewports of a study are kept on the planes' position.
    pub linked: bool,
    /// How labels are drawn over the planes: `off`, `outline` or `filled`.
    pub overlay: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self::of(&AppState::default(), None)
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
    /// The preferences as the state has them, with the window at `window`.
    pub fn of(state: &AppState, window: Option<[f32; 2]>) -> Self {
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
            window,
            viewports,
            radiological: state.radiological,
            linked: state.planes.linked,
            overlay: overlay.to_owned(),
        }
    }

    /// What to ask of the state to put it as these preferences have it: an action for each
    /// thing that differs, and none for a part that is not a value Voxa knows.
    pub fn actions(&self, state: &AppState) -> Vec<Action> {
        let now = Self::of(state, None);
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

    /// These preferences with whatever is not a value Voxa knows put back as Voxa starts.
    fn sound(mut self) -> Self {
        let start = Self::default();
        let big_enough = |[wide, high]: [f32; 2]| {
            wide.is_finite()
                && high.is_finite()
                && wide >= SMALLEST_WINDOW[0]
                && high >= SMALLEST_WINDOW[1]
        };
        self.window = self.window.filter(|size| big_enough(*size));
        if layout_of(self.viewports).is_none() {
            self.viewports = start.viewports;
        }
        if overlay_of(&self.overlay).is_none() {
            self.overlay = start.overlay;
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
mod tests;
