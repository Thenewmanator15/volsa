//! Where the 3D scene is seen from: a camera in patient space with no perspective, so that
//! a millimetre is the same size on the screen wherever it is.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use voxa_core::{Affine, Patient, Point, Screen};
use voxa_volume::Ray;

use crate::vector::{dot, moved};
use crate::view::{finite_or, positive_or};

/// How far the view turns for each screen pixel the pointer is dragged, in degrees.
pub const ORBIT_DEGREES_PER_PX: f64 = 0.5;
/// The most the scene can be magnified: millimetres of the patient to one screen pixel.
pub const MIN_MM_PER_PX: f64 = 0.01;
/// The most the scene can be shrunk: millimetres of the patient to one screen pixel.
pub const MAX_MM_PER_PX: f64 = 10.0;
/// How far towards the viewer from the target a line of sight starts, in millimetres:
/// further than any scan reaches.
const SIGHT_STARTS_MM: f64 = 10_000.0;
/// How much of the viewport a fitted volume fills along its tighter side.
const FITTED_SHARE: f64 = 0.9;

/// The six sides a patient can be looked at from, each named for the side facing the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedView {
    /// From the front: the face to the viewer, the patient's left on the right of the screen.
    Anterior,
    /// From behind: the patient's left on the left of the screen.
    Posterior,
    /// From the patient's left side: the face to the left of the screen.
    Left,
    /// From the patient's right side: the face to the right of the screen.
    Right,
    /// From above the head, the face to the top of the screen.
    Superior,
    /// From below the feet, the face to the top of the screen and the patient's left on the
    /// right: the way slices across the body are shown.
    Inferior,
}

impl NamedView {
    /// The yaw and pitch of this view, in radians.
    fn angles(self) -> (f64, f64) {
        match self {
            Self::Anterior => (0.0, 0.0),
            Self::Left => (FRAC_PI_2, 0.0),
            Self::Posterior => (PI, 0.0),
            Self::Right => (3.0 * FRAC_PI_2, 0.0),
            Self::Superior => (PI, FRAC_PI_2),
            Self::Inferior => (0.0, -FRAC_PI_2),
        }
    }
}

/// Where the scene is seen from. The head stays up: the view turns about the patient's
/// head-to-foot axis and tips up and down, and never rolls.
///
/// A number here that cannot be used (not finite, or out of range) is read as its nearest
/// sound value, so a camera always gives a view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// The point of the patient in the middle of the viewport.
    pub target: Point<Patient>,
    /// The turn about the head-to-foot axis, in radians: 0 faces the patient's face, a quarter
    /// turn faces their left side.
    pub yaw: f64,
    /// How far the view looks down on the head, in radians: a quarter turn looks straight
    /// down from above, minus a quarter turn straight up from below.
    pub pitch: f64,
    /// Millimetres of the patient to one screen pixel.
    pub mm_per_px: f64,
}

impl Camera {
    /// A camera facing one side of the patient, with `target` in the middle of the viewport.
    pub fn looking(view: NamedView, target: Point<Patient>, mm_per_px: f64) -> Self {
        let (yaw, pitch) = view.angles();
        Self {
            target,
            yaw,
            pitch,
            mm_per_px,
        }
        .sound()
    }

    /// A camera facing one side of the patient that shows the whole of a box, given by its
    /// eight corners, in a viewport of this width and height: the box is in the middle and
    /// fills nine tenths of the viewport along the side it fits more tightly.
    pub fn fitted(view: NamedView, corners: &[Point<Patient>; 8], viewport: [f32; 2]) -> Self {
        Self::looking(view, Point::new(0.0, 0.0, 0.0), 1.0).refitted(corners, viewport)
    }

    /// The camera, facing the way it does, moved and scaled to show the whole of a box as
    /// [`Camera::fitted`] would.
    #[must_use]
    pub fn refitted(self, corners: &[Point<Patient>; 8], viewport: [f32; 2]) -> Self {
        let sum = corners
            .iter()
            .fold([0.0; 3], |sum, corner| moved(sum, 1.0, xyz(*corner)));
        let [x, y, z] = sum.map(|total| total / 8.0);
        let facing = Self {
            target: Point::new(x, y, z),
            ..self
        }
        .sound();
        let [right, down, _] = facing.axes();
        let span = |axis: [f64; 3]| {
            let along = corners.iter().map(|corner| dot(axis, xyz(*corner)));
            let ends = (f64::INFINITY, f64::NEG_INFINITY);
            let (low, high) = along.fold(ends, |(low, high), at| (low.min(at), high.max(at)));
            high - low
        };
        let [wide, high] = viewport.map(|size| positive_or(f64::from(size), 1.0));
        let mm_per_px = (span(right) / wide).max(span(down) / high) / FITTED_SHARE;
        Self {
            mm_per_px,
            ..facing
        }
        .sound()
    }

    /// The directions, in the patient, of the right of the screen, down the screen, and
    /// straight ahead away from the viewer. Each has length 1.
    pub fn axes(&self) -> [[f64; 3]; 3] {
        let sound = self.sound();
        let (sin_yaw, cos_yaw) = sound.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = sound.pitch.sin_cos();
        let right = [cos_yaw, sin_yaw, 0.0];
        let forward = [-cos_pitch * sin_yaw, cos_pitch * cos_yaw, -sin_pitch];
        // Forward crossed with right, so that right, down and forward turn the same way as
        // the patient's left, back and head: nothing is ever shown mirrored.
        let down = [sin_pitch * sin_yaw, -sin_pitch * cos_yaw, -cos_pitch];
        [right, down, forward]
    }

    /// The transform from a viewport of this width and height to the patient. Screen x and y
    /// are pixels from the viewport's top left corner; screen z is millimetres beyond the
    /// target, away from the viewer. It can always be undone.
    pub fn screen_to_patient(&self, viewport: [f32; 2]) -> Affine<Screen, Patient> {
        let sound = self.sound();
        let [right, down, forward] = sound.axes();
        let [across, along] = [right, down].map(|axis| axis.map(|part| part * sound.mm_per_px));
        let [half_wide, half_high] = viewport.map(|size| finite_or(f64::from(size), 0.0) / 2.0);
        let top_left = moved(
            moved(xyz(sound.target), -half_wide, across),
            -half_high,
            along,
        );
        Affine::from_columns(across, along, forward, top_left)
    }

    /// The line of sight through a point of the viewport, given in pixels from its top left
    /// corner. It starts well outside any volume, on the viewer's side.
    pub fn ray_through(&self, viewport: [f32; 2], at: [f32; 2]) -> Ray {
        let [_, _, forward] = self.axes();
        let [x, y] = at.map(|px| finite_or(f64::from(px), 0.0));
        let origin = self
            .screen_to_patient(viewport)
            .apply(Point::new(x, y, -SIGHT_STARTS_MM));
        Ray {
            origin,
            direction: forward,
        }
    }

    /// The camera after the pointer is dragged this many pixels across and down: the patient
    /// turns with the pointer, as if held by the surface nearest the viewer. The view tips no
    /// further than straight down or straight up.
    #[must_use]
    pub fn orbited(self, by_px: [f32; 2]) -> Self {
        let [across, down] = by_px.map(f64::from);
        if !(across.is_finite() && down.is_finite()) {
            return self;
        }
        let from = self.sound();
        let turn = ORBIT_DEGREES_PER_PX.to_radians();
        Self {
            yaw: from.yaw - across * turn,
            pitch: from.pitch + down * turn,
            ..from
        }
        .sound()
    }

    /// The camera after the pointer is dragged this many pixels across and down with the
    /// scene following it.
    #[must_use]
    pub fn panned(self, by_px: [f32; 2]) -> Self {
        let [across, down] = by_px.map(f64::from);
        if !(across.is_finite() && down.is_finite()) {
            return self;
        }
        let from = self.sound();
        from.shifted(-across * from.mm_per_px, -down * from.mm_per_px)
    }

    /// The camera magnified `factor` times, keeping still the point of the scene that is
    /// `about_px` pixels across and down from the middle of the viewport. The magnification
    /// stops at [`MIN_MM_PER_PX`] and [`MAX_MM_PER_PX`]; a factor that is not a positive
    /// number changes nothing.
    #[must_use]
    pub fn zoomed_about(self, factor: f32, about_px: [f32; 2]) -> Self {
        let factor = f64::from(factor);
        let [across, down] = about_px.map(f64::from);
        if !(factor.is_finite() && factor > 0.0 && across.is_finite() && down.is_finite()) {
            return self;
        }
        let from = self.sound();
        let mm_per_px = (from.mm_per_px / factor).clamp(MIN_MM_PER_PX, MAX_MM_PER_PX);
        // The point under the pointer is as many pixels from the middle after as before.
        let closer = from.mm_per_px - mm_per_px;
        Self {
            mm_per_px,
            ..from.shifted(across * closer, down * closer)
        }
    }

    /// The camera with this point of the patient in the middle of the viewport.
    #[must_use]
    pub fn centred_on(self, point: Point<Patient>) -> Self {
        Self {
            target: point,
            ..self
        }
        .sound()
    }

    /// The camera with its target moved this many millimetres to the right of the screen and
    /// down it.
    fn shifted(self, right_mm: f64, down_mm: f64) -> Self {
        let [right, down, _] = self.axes();
        let [x, y, z] = moved(moved(xyz(self.target), right_mm, right), down_mm, down);
        Self {
            target: Point::new(x, y, z),
            ..self
        }
    }

    /// The camera with every number made usable: a target, yaw or pitch that is not finite
    /// becomes 0, the yaw is brought within one turn and the pitch within a quarter turn
    /// either way, and the scale is held between its limits (1 mm a pixel if not finite).
    fn sound(self) -> Self {
        let [x, y, z] = xyz(self.target).map(|part| finite_or(part, 0.0));
        Self {
            target: Point::new(x, y, z),
            yaw: finite_or(self.yaw, 0.0).rem_euclid(TAU),
            pitch: finite_or(self.pitch, 0.0).clamp(-FRAC_PI_2, FRAC_PI_2),
            mm_per_px: finite_or(self.mm_per_px, 1.0).clamp(MIN_MM_PER_PX, MAX_MM_PER_PX),
        }
    }
}

/// The cameras for one full turn about the patient's head-to-foot axis in `frames` equal
/// steps, the first being `camera` itself. The patient turns the way a drag to the right
/// turns them.
pub fn turntable(camera: &Camera, frames: u32) -> Vec<Camera> {
    let step = TAU / f64::from(frames.max(1));
    let turned = |frame: u32| Camera {
        yaw: camera.yaw - f64::from(frame) * step,
        ..*camera
    };
    let after_the_first = (1..frames).map(|frame| turned(frame).sound());
    (0..frames.min(1))
        .map(|_| *camera)
        .chain(after_the_first)
        .collect()
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

#[cfg(test)]
mod tests;
