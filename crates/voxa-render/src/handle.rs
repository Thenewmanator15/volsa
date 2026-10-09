//! Where the things a person takes hold of in the scene are on the screen, and what a drag on
//! one of them means in the patient: the cut planes, and the slice shown in 2D.

use voxa_core::{Affine, Patient, Point, Voxel};

use crate::camera::Camera;
use crate::scene::CutPlane;
use crate::vector::{dot, unit};

/// How far from the centre of a handle its knob is drawn, in pixels.
const KNOB_REACH: f64 = 60.0;
/// How short the normal of a plane must look, of its true length of 1, before it is taken as
/// pointing at the viewer, where a drag along it is no longer a drag across the screen.
const END_ON: f64 = 0.2;

/// Where a plane can be taken hold of on the screen, in pixels from the top left corner of the
/// viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handle {
    /// The plane's own point: dragging here moves the plane along its normal.
    pub centre: [f32; 2],
    /// The end of the plane's normal as it is seen: dragging here turns the plane. When the
    /// normal points at the viewer or away, it is straight above the centre.
    pub knob: [f32; 2],
}

/// The normal of a plane as the camera sees it: how far a step of one millimetre along it
/// goes to the right of the screen and down it, in millimetres. `None` for no direction.
fn as_seen(camera: &Camera, normal: [f64; 3]) -> Option<([f64; 2], f64)> {
    let normal = unit(normal)?;
    let [right, down, forward] = camera.axes();
    Some((
        [dot(normal, right), dot(normal, down)],
        dot(normal, forward),
    ))
}

/// Where a point of the patient is on the screen, in pixels from the top left corner of a
/// viewport of this width and height.
pub fn on_screen(camera: &Camera, viewport: [f32; 2], point: Point<Patient>) -> [f32; 2] {
    let to_screen = camera.screen_to_patient(viewport).inverse();
    let at = to_screen.map_or(Point::new(0.0, 0.0, 0.0), |to_screen| {
        to_screen.apply(point)
    });
    [narrowed(at.x), narrowed(at.y)]
}

/// Where a cut plane can be taken hold of.
pub fn cut_handle(camera: &Camera, viewport: [f32; 2], cut: &CutPlane) -> Handle {
    let centre = on_screen(camera, viewport, cut.point);
    let seen = as_seen(camera, cut.normal).map(|([across, down], _)| (across, down));
    let length = seen.map_or(0.0, |(across, down)| across.hypot(down));
    let (across, down) = match seen {
        Some((across, down)) if length >= END_ON => (across / length, down / length),
        _ => (0.0, -1.0),
    };
    let [x, y] = centre;
    let knob = [
        x + narrowed(across * KNOB_REACH),
        y + narrowed(down * KNOB_REACH),
    ];
    Handle { centre, knob }
}

/// How many millimetres along its normal a plane is moved by a drag of this many pixels
/// across and down: the plane's point follows the pointer along the normal as it is seen.
/// Where the normal points at the viewer, or away, dragging down brings the plane nearer.
pub fn drag_to_mm(camera: &Camera, normal: [f64; 3], drag: [f32; 2]) -> f64 {
    let [across, down] = drag.map(f64::from);
    let Some(([seen_across, seen_down], away)) = as_seen(camera, normal) else {
        return 0.0;
    };
    if !(across.is_finite() && down.is_finite()) {
        return 0.0;
    }
    let [a_pixel_across, _, _] = camera.screen_to_patient([0.0; 2]).columns();
    let mm_per_px = dot(a_pixel_across, a_pixel_across).sqrt();
    let seen = seen_across.hypot(seen_down);
    if seen < END_ON {
        // Nearer the viewer is along a normal that points at them, against one that does not.
        return if away < 0.0 {
            down * mm_per_px
        } else {
            -down * mm_per_px
        };
    }
    // The less of the normal's length is seen, the further along it a pixel of drag goes.
    (across * seen_across + down * seen_down) * mm_per_px / (seen * seen)
}

/// How many slices a drag of the slice's handle moves on by, towards later slices if the
/// normal points that way. `spacing_mm` is the distance between slices. What is left over
/// short of a whole slice is kept in `carried`, in millimetres, for the next drag.
pub fn slice_drag_to_scroll(
    camera: &Camera,
    normal: [f64; 3],
    spacing_mm: f64,
    drag: [f32; 2],
    carried: &mut f64,
) -> i32 {
    if !(spacing_mm.is_finite() && spacing_mm > 0.0) {
        return 0;
    }
    *carried += drag_to_mm(camera, normal, drag);
    let slices = (*carried / spacing_mm).trunc();
    *carried -= slices * spacing_mm;
    whole(slices)
}

/// The four corners of a slice as they are on the screen, going round it. `placement` is the
/// series' own, `size` its columns and rows, and `slice` counts from 0.
pub fn slice_outline(
    camera: &Camera,
    viewport: [f32; 2],
    placement: &Affine<Voxel, Patient>,
    size: [u32; 2],
    slice: usize,
) -> [[f32; 2]; 4] {
    let [cols, rows] = size.map(|count| f64::from(count) - 0.5);
    let slice = u32::try_from(slice).map_or(0.0, f64::from);
    let corners = [[-0.5, -0.5], [cols, -0.5], [cols, rows], [-0.5, rows]];
    corners.map(|[col, row]| {
        on_screen(
            camera,
            viewport,
            placement.apply(Point::new(col, row, slice)),
        )
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a place on the screen is a small number"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a whole number of slices, held in range"
)]
fn whole(slices: f64) -> i32 {
    slices.clamp(-100_000.0, 100_000.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::NamedView;

    const VIEWPORT: [f32; 2] = [400.0, 300.0];

    fn camera(view: NamedView) -> Camera {
        Camera::looking(view, Point::new(0.0, 0.0, 0.0), 0.5)
    }

    fn cut(point: [f64; 3], normal: [f64; 3]) -> CutPlane {
        let [x, y, z] = point;
        CutPlane {
            point: Point::new(x, y, z),
            normal,
        }
    }

    /// A normal that points to the left of the screen from the patient's left side, and at
    /// the viewer from the front.
    const TO_THE_FRONT: [f64; 3] = [0.0, -1.0, 0.0];

    #[test]
    fn a_point_of_the_patient_is_found_on_the_screen() {
        let from_the_front = camera(NamedView::Anterior);
        assert_eq!(
            on_screen(&from_the_front, VIEWPORT, Point::new(0.0, 0.0, 0.0)),
            [200.0, 150.0]
        );
        // 10 mm to the patient's left and 5 mm up, at half a millimetre a pixel.
        assert_eq!(
            on_screen(&from_the_front, VIEWPORT, Point::new(10.0, 7.0, 5.0)),
            [220.0, 140.0]
        );
    }

    #[test]
    fn a_cuts_knob_is_sixty_pixels_along_its_normal_as_seen() {
        let handle = cut_handle(
            &camera(NamedView::Left),
            VIEWPORT,
            &cut([0.0; 3], TO_THE_FRONT),
        );
        assert_eq!(handle.centre, [200.0, 150.0]);
        assert_eq!(handle.knob, [140.0, 150.0], "to the left of the screen");
        let off_centre = cut([0.0, 10.0, 0.0], [0.0, -3.0, 0.0]);
        let handle = cut_handle(&camera(NamedView::Left), VIEWPORT, &off_centre);
        assert_eq!(
            (handle.centre, handle.knob),
            ([220.0, 150.0], [160.0, 150.0])
        );
    }

    #[test]
    fn a_cut_that_faces_the_viewer_has_its_knob_straight_above() {
        let cut = cut([0.0; 3], TO_THE_FRONT);
        let handle = cut_handle(&camera(NamedView::Anterior), VIEWPORT, &cut);
        assert_eq!(handle.knob, [200.0, 90.0]);
        let none = CutPlane {
            normal: [0.0; 3],
            ..cut
        };
        assert_eq!(
            cut_handle(&camera(NamedView::Left), VIEWPORT, &none).knob,
            [200.0, 90.0]
        );
    }

    #[test]
    fn a_drag_along_the_normal_as_seen_moves_the_plane_that_far() {
        let from_the_left = camera(NamedView::Left);
        assert_eq!(drag_to_mm(&from_the_left, TO_THE_FRONT, [-20.0, 0.0]), 10.0);
        assert_eq!(drag_to_mm(&from_the_left, TO_THE_FRONT, [20.0, 0.0]), -10.0);
        assert_eq!(
            drag_to_mm(&from_the_left, TO_THE_FRONT, [0.0, 35.0]),
            0.0,
            "across it"
        );
        assert_eq!(
            drag_to_mm(&from_the_left, [0.0; 3], [20.0, 0.0]),
            0.0,
            "no direction"
        );
        assert_eq!(
            drag_to_mm(&from_the_left, TO_THE_FRONT, [f32::NAN, 0.0]),
            0.0
        );
    }

    #[test]
    fn a_plane_seen_at_a_slant_keeps_its_point_under_the_pointer() {
        // Turned 60 degrees, half of the normal's length is seen, so a pixel goes twice as far.
        let slanted = camera(NamedView::Left).orbited([120.0, 0.0]);
        let moved = drag_to_mm(&slanted, TO_THE_FRONT, [-20.0, 0.0]);
        assert!((moved - 20.0).abs() < 1e-9, "{moved}");
    }

    #[test]
    fn a_plane_facing_the_viewer_is_moved_by_dragging_down_and_up() {
        let from_the_front = camera(NamedView::Anterior);
        assert_eq!(drag_to_mm(&from_the_front, TO_THE_FRONT, [0.0, 20.0]), 10.0);
        assert_eq!(drag_to_mm(&from_the_front, TO_THE_FRONT, [50.0, 0.0]), 0.0);
        // Facing away, dragging down still brings it nearer: against its normal.
        assert_eq!(
            drag_to_mm(&from_the_front, [0.0, 1.0, 0.0], [0.0, 20.0]),
            -10.0
        );
    }

    #[test]
    fn dragging_the_slice_moves_on_a_slice_for_each_spacing_and_carries_the_rest() {
        let from_the_front = camera(NamedView::Anterior);
        let up_the_screen = [0.0, 0.0, 1.0];
        let mut carried = 0.0;
        let scrolled = |drag: [f32; 2], carried: &mut f64| {
            slice_drag_to_scroll(&from_the_front, up_the_screen, 5.0, drag, carried)
        };
        assert_eq!((scrolled([0.0, -10.0], &mut carried), carried), (1, 0.0));
        assert_eq!((scrolled([0.0, -6.0], &mut carried), carried), (0, 3.0));
        assert_eq!((scrolled([0.0, -6.0], &mut carried), carried), (1, 1.0));
        assert_eq!(
            (scrolled([0.0, 30.0], &mut carried), carried),
            (-2, -4.0),
            "and back down"
        );
        let mut carried = 0.0;
        let nothing = slice_drag_to_scroll(
            &from_the_front,
            up_the_screen,
            0.0,
            [0.0, -9.0],
            &mut carried,
        );
        assert_eq!(
            (nothing, carried),
            (0, 0.0),
            "slices no distance apart cannot be counted"
        );
    }

    #[test]
    fn the_outline_of_a_slice_is_its_four_corners_on_the_screen() {
        // 4 x 2 voxels of 10 mm, lying flat, 20 mm up, seen from the front: a line on its edge.
        let flat = Affine::from_columns(
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
            [0.0; 3],
        );
        let front = slice_outline(&camera(NamedView::Anterior), VIEWPORT, &flat, [4, 2], 2);
        assert_eq!(
            front,
            [
                [190.0, 110.0],
                [270.0, 110.0],
                [270.0, 110.0],
                [190.0, 110.0]
            ]
        );
        // From below it is seen whole: 40 mm across and 20 mm down, at two pixels a millimetre.
        let below = slice_outline(&camera(NamedView::Inferior), VIEWPORT, &flat, [4, 2], 2);
        assert_eq!(
            below,
            [
                [190.0, 140.0],
                [270.0, 140.0],
                [270.0, 180.0],
                [190.0, 180.0]
            ]
        );
    }
}
