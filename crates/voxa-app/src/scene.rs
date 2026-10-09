//! The 3D scene as the application remembers it: which series it shows, how far its volume
//! has got, where it is seen from, in which look, and how it is cut open.

use voxa_core::{Patient, Point, SeriesId};
use voxa_render::{Camera, CutPlane, Look, NamedView};

/// Which kind of view has the large part of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainPanel {
    /// The 3D scene, with the 2D viewports in a column beside it.
    #[default]
    Scene,
    /// The 2D viewports, with the 3D scene small beside them.
    Stack,
}

/// How far the volume the scene shows has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneStatus {
    /// No series is shown.
    Empty,
    /// The slices are being decoded and stacked.
    Building {
        /// Slices decoded so far.
        done: usize,
        /// Slices in the series.
        of: usize,
    },
    /// The volume is built and can be drawn.
    Ready,
    /// The series has no 3D view. The sentences say why, the first being the summary.
    Unavailable(Vec<String>),
}

/// The 3D scene. It shows the series of the active viewport, and starts afresh whenever that
/// changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// The series shown, if any.
    pub series: Option<SeriesId>,
    /// How far its volume has got.
    pub status: SceneStatus,
    /// Where the volume is seen from.
    pub camera: Camera,
    /// True while the camera is Voxa's own choice; false once the person has moved it.
    pub untouched: bool,
    /// The width and height of the scene's picture, in pixels.
    pub viewport: [f32; 2],
    /// The eight outer corners of the volume in the patient, once it is built.
    pub corners: Option<[Point<Patient>; 8]>,
    /// How many times the volume was halved, each way, to fit the graphics device.
    pub halved: u32,
    /// How the volume is turned into a picture.
    pub look: Look,
    /// The planes that cut the volume open.
    pub cuts: Vec<CutPlane>,
    /// Whether the slice shown in the 2D viewport is drawn in the scene.
    pub slice_shown: bool,
}

impl Scene {
    /// The size the scene's picture is taken to have until the window says what it is.
    pub const FIRST_VIEWPORT: [f32; 2] = [800.0, 600.0];
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            series: None,
            status: SceneStatus::Empty,
            camera: Camera::looking(NamedView::Anterior, Point::new(0.0, 0.0, 0.0), 1.0),
            untouched: true,
            viewport: Self::FIRST_VIEWPORT,
            corners: None,
            halved: 0,
            look: Look::Surface,
            cuts: Vec::new(),
            slice_shown: false,
        }
    }
}
