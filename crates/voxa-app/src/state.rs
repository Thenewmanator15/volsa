//! Everything the application remembers, in one place.

use std::collections::HashMap;

use voxa_core::{SeriesId, SeriesInfo, WindowLevel};
use voxa_dicom::{Index, IndexedSeries, Report};
use voxa_render::ViewTransform;
use voxa_volume::PRESETS;

use crate::annotations::{Annotations, Left, Tool};
use crate::labelling::Labelling;
use crate::labels::Labels;
use crate::pictures::Pictures;
use crate::planes::Planes;
use crate::scene::Scene;
use crate::views::SavedView;

/// How many viewports the window can show at once.
pub const VIEWPORTS: usize = 4;

/// The window each preset gives for one slice, in the order of [`PRESETS`].
pub type PresetWindows = [WindowLevel; PRESETS.len()];

/// How the viewports are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// One viewport.
    #[default]
    One,
    /// Two viewports side by side.
    TwoAcross,
    /// Four viewports, two by two.
    TwoByTwo,
}

impl Layout {
    /// How many viewports this layout shows.
    pub fn count(self) -> usize {
        match self {
            Self::One => 1,
            Self::TwoAcross => 2,
            Self::TwoByTwo => 4,
        }
    }
}

/// Playing through the slices of a series like a film.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cine {
    /// Slices shown each second, from 1 to 60.
    pub fps: f32,
    /// Whether to start again from the first slice after the last.
    pub looping: bool,
    /// Whether it is running now.
    pub playing: bool,
}

impl Cine {
    /// The slowest playback offered, in slices each second.
    pub const MIN_FPS: f32 = 1.0;
    /// The fastest playback offered, in slices each second.
    pub const MAX_FPS: f32 = 60.0;
    /// The speed playback starts at.
    pub const DEFAULT_FPS: f32 = 10.0;

    /// Playback at `fps`, held between [`Cine::MIN_FPS`] and [`Cine::MAX_FPS`]; a speed that
    /// is not a number becomes [`Cine::DEFAULT_FPS`].
    pub fn new(fps: f32, looping: bool, playing: bool) -> Self {
        let fps = if fps.is_nan() {
            Self::DEFAULT_FPS
        } else {
            fps.clamp(Self::MIN_FPS, Self::MAX_FPS)
        };
        Self {
            fps,
            looping,
            playing,
        }
    }
}

/// What one viewport shows and how.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportState {
    /// The series shown, if any.
    pub series: Option<SeriesId>,
    /// Which slice of it, counting from 0.
    pub slice: usize,
    /// The window and level in use.
    pub wl: WindowLevel,
    /// True while the window is Voxa's own choice; false once the person has set one.
    pub wl_auto: bool,
    /// True while that choice is still to be made, because the slice shown is not decoded yet.
    pub wl_pending: bool,
    /// Zoom, pan, flips and turns.
    pub view: ViewTransform,
    /// Film-like playback, once it has been started.
    pub cine: Option<Cine>,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            series: None,
            slice: 0,
            wl: WindowLevel::new(0.0, 1.0),
            wl_auto: true,
            wl_pending: true,
            view: ViewTransform::fit(),
            cine: None,
        }
    }
}

/// How serious a notice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// Worth knowing.
    Info,
    /// Something is not as it should be, but work can go on.
    Warning,
    /// What was asked for could not be done.
    Error,
}

/// A message for the person using Voxa, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// How serious it is.
    pub kind: NoticeKind,
    /// The message.
    pub text: String,
    /// More to show on request, such as the names of the files concerned.
    pub details: Vec<String>,
    /// A notice with a key replaces any earlier notice with the same key, so a count that
    /// keeps changing stays one notice.
    pub key: Option<&'static str>,
}

/// A document of the export shown in the middle of the window in place of the pictures.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenDocument {
    /// The name of the entry in the source that holds the document's file.
    pub entry: String,
    /// What a worker made of the file, once it has: a structured report as text, or the
    /// sentence that says why it could not be read. `None` while it is being read, and for a
    /// document that is not read as text.
    pub report: Option<Result<Report, String>>,
}

/// Everything the application remembers. It changes only through
/// [`reduce`](crate::reduce).
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    /// What was found in the source that is open.
    pub index: Option<Index>,
    /// How many times in this run the index has been replaced or cleared: what is worked out
    /// from the index is worked out again when this changes.
    pub index_changes: u64,
    /// How the viewports are arranged.
    pub layout: Layout,
    /// The viewports; a layout shows the first one, two or four.
    pub viewports: [ViewportState; VIEWPORTS],
    /// The viewport that keyboard commands act on.
    pub active: usize,
    /// Radiological convention: the patient's left on the right of the screen.
    pub radiological: bool,
    /// Whether the patient's name and identifier are shown.
    pub show_patient: bool,
    /// What the tag browser is filtered by.
    pub tag_filter: String,
    /// Messages waiting to be read.
    pub notices: Vec<Notice>,
    /// For each decoded slice, the window each preset gives.
    pub windows: HashMap<(SeriesId, usize), PresetWindows>,
    /// Slices that could not be decoded, each with the reason.
    pub failed: HashMap<(SeriesId, usize), String>,
    /// The 3D scene, which shows the series of the active viewport.
    pub scene: Scene,
    /// How the pictures in the middle of the window are arranged.
    pub pictures: Pictures,
    /// The views saved of the scene's series, oldest first.
    pub views: Vec<SavedView>,
    /// The axial, coronal and sagittal planes through one point of the scene's volume.
    pub planes: Planes,
    /// The measurements and markers made on the scene's series.
    pub annotations: Annotations,
    /// The measurements of the series the scene has left in this run, with their history.
    pub annotations_left: Left,
    /// What the main button does in the plane views and the 3D scene.
    pub tool: Tool,
    /// The labelled parts open on a study, and what is shown of them.
    pub labels: Labels,
    /// Labelling a scan with Voxa's own networks: what can be done, what is chosen, what runs.
    pub labelling: Labelling,
    /// The document of the export that is open in the middle of the window, if one is.
    pub document: Option<OpenDocument>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            index: None,
            index_changes: 0,
            layout: Layout::One,
            viewports: Default::default(),
            active: 0,
            radiological: true,
            show_patient: false,
            tag_filter: String::new(),
            notices: Vec::new(),
            windows: HashMap::new(),
            failed: HashMap::new(),
            scene: Scene::default(),
            pictures: Pictures::default(),
            views: Vec::new(),
            planes: Planes::default(),
            annotations: Annotations::default(),
            annotations_left: Left::default(),
            tool: Tool::default(),
            labels: Labels::default(),
            labelling: Labelling::default(),
            document: None,
        }
    }
}

impl AppState {
    /// The series with this identifier in the open source.
    pub fn series(&self, id: &SeriesId) -> Option<&IndexedSeries> {
        self.index
            .as_ref()?
            .series
            .iter()
            .find(|series| series.info.id == *id)
    }

    /// What is known of the series a viewport shows; `None` when it shows nothing.
    pub fn shown_info(&self, viewport: usize) -> Option<&SeriesInfo> {
        let shown = self.viewports.get(viewport)?.series.as_ref()?;
        Some(&self.series(shown)?.info)
    }

    /// How many slices the series shown in a viewport has; 0 when it shows nothing.
    pub fn slices_in(&self, viewport: usize) -> usize {
        let shown = self
            .viewports
            .get(viewport)
            .and_then(|viewport| viewport.series.as_ref());
        shown
            .and_then(|id| self.series(id))
            .map_or(0, |series| series.slices.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_state_is_radiological_and_hides_the_patient() {
        let state = AppState::default();
        assert!(state.radiological);
        assert!(!state.show_patient);
        assert_eq!((state.layout, state.active), (Layout::One, 0));
        assert!(
            state
                .viewports
                .iter()
                .all(|viewport| viewport.series.is_none())
        );
    }

    #[test]
    fn layouts_show_one_two_or_four_viewports() {
        let counts = [Layout::One, Layout::TwoAcross, Layout::TwoByTwo].map(Layout::count);
        assert_eq!(counts, [1, 2, 4]);
    }

    #[test]
    fn cine_speed_is_held_between_one_and_sixty() {
        assert_eq!(Cine::new(0.2, true, true).fps, 1.0);
        assert_eq!(Cine::new(500.0, true, true).fps, 60.0);
        assert_eq!(Cine::new(24.0, true, true).fps, 24.0);
        assert_eq!(Cine::new(f32::NAN, true, true).fps, Cine::DEFAULT_FPS);
    }

    #[test]
    fn a_viewport_showing_nothing_has_no_slices() {
        let state = AppState::default();
        assert_eq!(state.slices_in(0), 0);
        assert_eq!(state.slices_in(99), 0);
    }
}
