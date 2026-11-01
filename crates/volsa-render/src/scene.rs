//! What a 3D picture of a volume is drawn from, apart from the volume itself: where it is seen
//! from, through which window, in which look, and the planes that cut it open.

use volsa_core::{Affine, Patient, Point, Voxel, WindowLevel};

use crate::camera::{Camera, ORBIT_DEGREES_PER_PX};
use crate::vector::{cross, dot, moved, turned_about, unit};

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
    /// Labelled parts as lit solids, each in its own colour. The scan's values play no
    /// part; [`PartsPipeline`](crate::PartsPipeline) draws it, and the volume pipeline,
    /// asked for it, draws a surface.
    Parts,
}

/// A plane that cuts the volume open. Everything on the side the normal points to is removed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutPlane {
    /// A point on the plane.
    pub point: Point<Patient>,
    /// The direction, of length 1, towards the part that is removed.
    pub normal: [f64; 3],
}

impl CutPlane {
    /// A cut straight across the view, through the camera's target: the half of the volume
    /// nearer the viewer is removed.
    pub fn facing(camera: &Camera) -> Self {
        let [_, _, forward] = camera.axes();
        Self {
            point: camera.target,
            normal: forward.map(|part| -part),
        }
    }

    /// The cut moved along its normal by this many millimetres: a positive distance removes
    /// more of the volume, a negative one puts some back.
    #[must_use]
    pub fn moved(self, by_mm: f64) -> Self {
        let Some(normal) = unit(self.normal).filter(|_| by_mm.is_finite()) else {
            return self;
        };
        let [x, y, z] = moved([self.point.x, self.point.y, self.point.z], -by_mm, normal);
        Self {
            point: Point::new(x, y, z),
            ..self
        }
    }

    /// The cut turned about its own point as the pointer is dragged this many pixels across
    /// and down: the tip of its normal follows the pointer, [`ORBIT_DEGREES_PER_PX`] to the
    /// pixel. The normal is left with a length of 1.
    #[must_use]
    pub fn turned(self, camera: &Camera, by_px: [f32; 2]) -> Self {
        let [across, down] = by_px.map(f64::from);
        let dragged = across.is_finite() && down.is_finite();
        let Some(normal) = unit(self.normal).filter(|_| dragged) else {
            return self;
        };
        let [right, below, _] = camera.axes();
        let above = below.map(|part| -part);
        let turn = ORBIT_DEGREES_PER_PX.to_radians();
        let normal = turned_about(
            turned_about(normal, above, across * turn),
            right,
            down * turn,
        );
        Self {
            normal: unit(normal).unwrap_or(normal),
            ..self
        }
    }
}

/// The plane of the slice shown in the 2D viewport, drawn inside the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlicePlane {
    /// A point on the plane.
    pub point: Point<Patient>,
    /// The direction, of length 1, straight out of the slice.
    pub normal: [f64; 3],
}

impl SlicePlane {
    /// The plane of one slice of a series: through the middle of the slice, facing straight
    /// out of it towards the slices that come after. `placement` is the series' own, `size` its columns and rows, and `slice`
    /// counts from 0. `None` when the placement gives the slice no plane.
    pub fn of_slice(
        placement: &Affine<Voxel, Patient>,
        size: [u32; 2],
        slice: usize,
    ) -> Option<Self> {
        let [along_columns, along_rows, to_the_next] = placement.columns();
        let out_of_it = unit(cross(along_columns, along_rows))?;
        // Of the two ways out of the slice, the one towards the slices after it.
        let facing = if dot(out_of_it, to_the_next) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let normal = out_of_it.map(|part| part * facing);
        let [cols, rows] = size.map(|count| (f64::from(count) - 1.0) / 2.0);
        let slice = f64::from(u32::try_from(slice).ok()?);
        Some(Self {
            point: placement.apply(Point::new(cols, rows, slice)),
            normal,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::NamedView;

    fn close(found: [f64; 3], expected: [f64; 3]) -> bool {
        found
            .into_iter()
            .zip(expected)
            .all(|(found, expected)| (found - expected).abs() < 1e-9)
    }

    fn from_the_front() -> Camera {
        Camera::looking(NamedView::Anterior, Point::new(1.0, 2.0, 3.0), 1.0)
    }

    #[test]
    fn a_cut_facing_the_camera_removes_the_half_nearer_the_viewer() {
        let cut = CutPlane::facing(&from_the_front());
        assert_eq!(cut.point, Point::new(1.0, 2.0, 3.0), "through the target");
        assert!(close(cut.normal, [0.0, -1.0, 0.0]), "{:?}", cut.normal);
        let from_above = Camera::looking(NamedView::Superior, Point::new(0.0, 0.0, 0.0), 1.0);
        assert!(close(CutPlane::facing(&from_above).normal, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn moving_a_cut_by_a_positive_distance_removes_more() {
        let cut = CutPlane::facing(&from_the_front());
        let deeper = cut.moved(5.0);
        assert_eq!(
            deeper.point,
            Point::new(1.0, 7.0, 3.0),
            "5 mm against its normal"
        );
        assert_eq!(deeper.normal, cut.normal);
        assert_eq!(
            cut.moved(-5.0).point,
            Point::new(1.0, -3.0, 3.0),
            "and back out"
        );
        assert_eq!(cut.moved(f64::NAN), cut);
        let long = CutPlane {
            normal: [0.0, -4.0, 0.0],
            ..cut
        };
        assert_eq!(
            long.moved(5.0).point,
            deeper.point,
            "millimetres, whatever the length"
        );
    }

    #[test]
    fn turning_a_cut_takes_the_tip_of_its_normal_with_the_pointer() {
        let camera = from_the_front();
        let cut = CutPlane::facing(&camera);
        let to_the_right = cut.turned(&camera, [180.0, 0.0]);
        assert!(
            close(to_the_right.normal, [1.0, 0.0, 0.0]),
            "{:?}",
            to_the_right.normal
        );
        let downwards = cut.turned(&camera, [0.0, 180.0]);
        assert!(
            close(downwards.normal, [0.0, 0.0, -1.0]),
            "{:?}",
            downwards.normal
        );
        assert_eq!(
            to_the_right.point, cut.point,
            "it turns about its own point"
        );
        assert_eq!(cut.turned(&camera, [f32::NAN, 0.0]), cut);
    }

    #[test]
    fn a_turned_cut_keeps_a_normal_of_length_one() {
        let camera = from_the_front().orbited([40.0, -25.0]);
        let mut cut = CutPlane {
            normal: [0.0, -3.0, 0.0],
            ..CutPlane::facing(&camera)
        };
        for _ in 0..200 {
            cut = cut.turned(&camera, [7.0, -3.0]);
        }
        let [x, y, z] = cut.normal;
        assert!(
            ((x * x + y * y + z * z).sqrt() - 1.0).abs() < 1e-9,
            "{:?}",
            cut.normal
        );
    }

    #[test]
    fn a_cut_with_no_direction_is_left_as_it_is() {
        let flat = CutPlane {
            point: Point::new(0.0, 0.0, 0.0),
            normal: [0.0; 3],
        };
        assert_eq!(flat.moved(5.0), flat);
        assert_eq!(flat.turned(&from_the_front(), [10.0, 10.0]), flat);
    }

    #[test]
    fn the_plane_of_a_slice_passes_through_its_middle_and_faces_out_of_it() {
        use volsa_core::Affine;
        // 4 x 4 voxels of 1 mm in slices 5 mm apart, going up the patient.
        let placed = |step: [f64; 3]| {
            Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], step, [-10.0, -20.0, 0.0])
        };
        let plane = SlicePlane::of_slice(&placed([0.0, 0.0, 5.0]), [4, 4], 3).expect("placed");
        assert_eq!(plane.point, Point::new(-8.5, -18.5, 15.0));
        assert!(close(plane.normal, [0.0, 0.0, 1.0]), "{:?}", plane.normal);
        // In a tilted stack the slices still lie flat: the plane faces out of the slice, not
        // along the stack.
        let tilted = SlicePlane::of_slice(&placed([0.0, 1.0, 5.0]), [4, 4], 2).expect("placed");
        assert_eq!(tilted.point, Point::new(-8.5, -16.5, 10.0));
        assert!(close(tilted.normal, [0.0, 0.0, 1.0]), "{:?}", tilted.normal);
        // It faces the way the slices are numbered: a step along it is a step to a later one.
        let downwards = SlicePlane::of_slice(&placed([0.0, 0.0, -5.0]), [4, 4], 1).expect("placed");
        assert!(
            close(downwards.normal, [0.0, 0.0, -1.0]),
            "{:?}",
            downwards.normal
        );
        let flat =
            Affine::from_columns([1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
        assert_eq!(
            SlicePlane::of_slice(&flat, [4, 4], 0),
            None,
            "no plane to speak of"
        );
    }
}
