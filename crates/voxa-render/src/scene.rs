//! What a 3D picture of a volume is drawn from, apart from the volume itself: where it is seen
//! from, through which window, in which look, and the planes that cut it open.

use voxa_core::{Patient, Point, WindowLevel};

use crate::camera::Camera;

/// The most cut planes a scene can have.
pub const MAX_CUTS: usize = 3;

/// How a volume is turned into a picture. The names are the ones radiology uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A lit, shaded surface, where the values first reach the window.
    Surface,
    /// Volume rendering: everything within the window is drawn see-through, brighter values
    /// more solid than darker ones.
    Vr,
    /// Maximum-intensity projection: each pixel shows the brightest value on its line of
    /// sight, wherever along the line it lies.
    Mip,
}

/// A plane that cuts the volume open. Everything on the side the normal points to is removed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutPlane {
    /// A point on the plane.
    pub point: Point<Patient>,
    /// The direction, of length 1, towards the part that is removed.
    pub normal: [f64; 3],
}

/// The plane of the slice shown in the 2D viewport, drawn inside the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlicePlane {
    /// A point on the plane.
    pub point: Point<Patient>,
    /// The direction, of length 1, straight out of the slice.
    pub normal: [f64; 3],
}

/// Everything a picture of a volume is drawn from, apart from the volume.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneParams {
    /// Where the volume is seen from.
    pub camera: Camera,
    /// The values shown from black to white. Below the window nothing is drawn.
    pub window: WindowLevel,
    /// How the volume is turned into a picture.
    pub look: Look,
    /// The planes that cut the volume open. No more than [`MAX_CUTS`] are used.
    pub cuts: Vec<CutPlane>,
    /// The slice shown in the 2D viewport, if it is to be drawn in the scene.
    pub slice: Option<SlicePlane>,
    /// How finely each line of sight is followed: 1.0 is full quality, and less is quicker
    /// and coarser, for while the camera is moving.
    pub quality: f32,
    /// Whether parts of the volume with nothing in them to draw are stepped over.
    pub skip_empty: bool,
}
