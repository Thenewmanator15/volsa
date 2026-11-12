//! What a panel shows of a plane through a volume: which plane, at what scale, and which
//! part of it is in the middle of the panel.

use volsa_core::{Patient, Point};
use volsa_volume::Plane;

/// The least a plane view can be zoomed to: the whole volume at half size.
pub const MIN_PLANE_ZOOM: f64 = 0.5;
/// The most a plane view can be zoomed to.
pub const MAX_PLANE_ZOOM: f64 = 40.0;
/// How much room is left round the volume when the whole of it is shown: 4% of its size.
const MARGIN: f64 = 1.04;

/// What a panel shows: a plane, how many millimetres of it a pixel covers, and where on the
/// plane the middle of the panel is, in millimetres across and down from the plane's centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneView {
    /// The plane shown.
    pub plane: Plane,
    /// Millimetres to the pixel.
    pub mm_per_px: f64,
    /// The point of the plane in the middle of the panel.
    pub offset_mm: [f64; 2],
}

impl PlaneView {
    /// A plane in a panel of `viewport` (width, height) pixels. At a `zoom` of 1 the whole
    /// of the volume's box, whose `corners` are given, is shown as it falls on the plane,
    /// with a small margin; at 2 everything is twice as large. The zoom is held between
    /// [`MIN_PLANE_ZOOM`] and [`MAX_PLANE_ZOOM`], and one that is not a number counts as 1.
    pub fn showing(
        plane: Plane,
        corners: &[Point<Patient>; 8],
        viewport: [f32; 2],
        zoom: f64,
        offset_mm: [f64; 2],
    ) -> Self {
        // How far the box reaches either side of the plane's centre, across and down.
        let reach = corners.iter().fold([0.0_f64; 2], |[wide, high], corner| {
            let ([across, down], _) = plane.of(*corner);
            [wide.max(across.abs()), high.max(down.abs())]
        });
        let [wide, high] = reach.map(|half| (2.0 * half).max(f64::MIN_POSITIVE));
        let [across, down] = viewport.map(|side| f64::from(side).max(1.0));
        let whole = (wide / across).max(high / down) * MARGIN;
        let zoom = if zoom.is_finite() { zoom } else { 1.0 };
        Self {
            plane,
            mm_per_px: whole / zoom.clamp(MIN_PLANE_ZOOM, MAX_PLANE_ZOOM),
            offset_mm,
        }
    }

    /// The point of the patient under a pixel of the panel, counted from its top left corner.
    pub fn screen_to_patient(&self, viewport: [f32; 2], at: [f32; 2]) -> Point<Patient> {
        let [across, down] = self.on_plane(viewport, at);
        self.plane.at([across, down])
    }

    /// Where on the plane a pixel of the panel is, in millimetres across and down from the
    /// plane's centre.
    pub fn on_plane(&self, [wide, high]: [f32; 2], [x, y]: [f32; 2]) -> [f64; 2] {
        let [across, down] = self.offset_mm;
        [
            across + f64::from(x - wide / 2.0) * self.mm_per_px,
            down + f64::from(y - high / 2.0) * self.mm_per_px,
        ]
    }

    /// Where in the panel a point of the patient falls, in pixels from its top left corner,
    /// and how many millimetres the point is from the plane along its normal.
    pub fn patient_to_screen(
        &self,
        [wide, high]: [f32; 2],
        point: Point<Patient>,
    ) -> ([f32; 2], f64) {
        let ([across, down], off) = self.plane.of(point);
        let [middle_across, middle_down] = self.offset_mm;
        let pixels =
            |mm: f64, middle: f64, side: f32| narrowed((mm - middle) / self.mm_per_px) + side / 2.0;
        (
            [
                pixels(across, middle_across, wide),
                pixels(down, middle_down, high),
            ],
            off,
        )
    }
}

/// A place on the screen as the single-precision number the interface works in.
#[expect(clippy::cast_possible_truncation, reason = "pixels are small numbers")]
fn narrowed(pixels: f64) -> f32 {
    pixels as f32
}

#[cfg(test)]
mod tests {
    use volsa_volume::{PlaneKind, plane_of};

    use super::*;
    use crate::testing::ball;

    const PANEL: [f32; 2] = [200.0, 100.0];

    fn axial() -> (Plane, [Point<Patient>; 8]) {
        let volume = ball();
        let middle = Point::new(0.0, 0.0, 0.0);
        let placement = volume.voxel_to_patient();
        let plane = plane_of(&placement, volume.dims(), PlaneKind::Axial, middle, true);
        (plane.expect("a plane"), volume.corners())
    }

    #[test]
    fn at_a_zoom_of_one_the_whole_volume_is_shown_with_a_margin() {
        let (plane, corners) = axial();
        let view = PlaneView::showing(plane, &corners, PANEL, 1.0, [0.0; 2]);
        // The ball's volume is 48 mm each way; the panel's height is what limits it.
        assert!(
            (view.mm_per_px - 48.0 * 1.04 / 100.0).abs() < 1.0e-12,
            "{}",
            view.mm_per_px
        );
        assert_eq!(
            view.patient_to_screen(PANEL, plane.centre),
            ([100.0, 50.0], 0.0)
        );
    }

    #[test]
    fn a_point_of_the_patient_and_its_pixel_are_found_from_each_other() {
        let (plane, corners) = axial();
        let view = PlaneView::showing(plane, &corners, PANEL, 1.0, [0.0; 2]);
        let ([x, y], off) = view.patient_to_screen(PANEL, plane.at([10.0, 0.0]));
        let expected = 100.0 + 10.0 / view.mm_per_px;
        assert!(
            (f64::from(x) - expected).abs() < 1.0e-4 && y == 50.0 && off == 0.0,
            "{x}, {y}"
        );
        let back = view.screen_to_patient(PANEL, [x, y]);
        let to = plane.at([10.0, 0.0]);
        let apart = (back.x - to.x).abs() + (back.y - to.y).abs() + (back.z - to.z).abs();
        assert!(apart < 1.0e-4, "{back:?}");
        // A point above the plane is in the same place on it, and is said to be above.
        let [nx, ny, nz] = plane.normal();
        let above = Point::new(to.x + 3.0 * nx, to.y + 3.0 * ny, to.z + 3.0 * nz);
        let (same, off) = view.patient_to_screen(PANEL, above);
        assert!((same[0] - x).abs() < 1.0e-4 && (off - 3.0).abs() < 1.0e-12);
    }

    #[test]
    fn zoom_makes_it_larger_and_the_offset_chooses_the_middle() {
        let (plane, corners) = axial();
        let whole = PlaneView::showing(plane, &corners, PANEL, 1.0, [0.0; 2]);
        let twice = PlaneView::showing(plane, &corners, PANEL, 2.0, [0.0; 2]);
        assert!((twice.mm_per_px - whole.mm_per_px / 2.0).abs() < 1.0e-12);
        let moved = PlaneView::showing(plane, &corners, PANEL, 1.0, [5.0, 0.0]);
        let (middle, _) = moved.patient_to_screen(PANEL, plane.at([5.0, 0.0]));
        assert_eq!(middle, [100.0, 50.0]);
        assert_eq!(moved.on_plane(PANEL, [100.0, 50.0]), [5.0, 0.0]);
    }

    #[test]
    fn a_zoom_out_of_bounds_or_a_panel_of_no_size_still_gives_a_scale() {
        let (plane, corners) = axial();
        let scale =
            |zoom, panel| PlaneView::showing(plane, &corners, panel, zoom, [0.0; 2]).mm_per_px;
        let whole = scale(1.0, PANEL);
        assert_eq!(scale(1.0e9, PANEL), whole / MAX_PLANE_ZOOM);
        assert_eq!(scale(0.0, PANEL), whole / MIN_PLANE_ZOOM);
        assert_eq!(scale(f64::NAN, PANEL), whole);
        assert!(scale(1.0, [0.0, 0.0]).is_finite() && scale(1.0, [0.0, 0.0]) > 0.0);
    }
}
