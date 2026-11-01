//! The 3D scene as the application remembers it: which series it shows, how far its volume
//! has got, where it is seen from, in which look, and how it is cut open.

use voxa_core::{Patient, Point, SeriesId};
use voxa_render::{Camera, CutPlane, Look, NamedView};

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each says one thing of the scene on its own; no two make a state together"
)]
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
    /// The box the camera is fitted to when it is fitted: the volume's own, or, in the
    /// Parts look, that of labels which reach beyond the volume.
    pub frame: Option<[Point<Patient>; 8]>,
    /// How many times the volume was halved, each way, to fit the graphics device.
    pub halved: u32,
    /// How the volume is turned into a picture.
    pub look: Look,
    /// The planes that cut the volume open.
    pub cuts: Vec<CutPlane>,
    /// Whether the slice shown in the 2D viewport is drawn in the scene.
    pub slice_shown: bool,
    /// True while the scene waits for another volume of the series it was showing: the
    /// camera is then left where it is when that volume arrives.
    pub keeps_view: bool,
    /// True while a preview of the volume being built is there to draw.
    pub preview: bool,
    /// Whether the 3D scene is wanted at all: while it is not, no volume is built for it.
    pub wanted: bool,
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
            frame: None,
            halved: 0,
            look: Look::Surface,
            cuts: Vec::new(),
            slice_shown: false,
            keeps_view: false,
            preview: false,
            wanted: true,
        }
    }
}
