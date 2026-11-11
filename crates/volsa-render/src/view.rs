//! Where an image lies in a viewport: fitted, zoomed, panned, flipped and turned.

use voxa_core::{Affine, Screen, Voxel};

/// The furthest the image can be shrunk below the size at which it just fits.
pub const MIN_ZOOM: f32 = 0.1;
/// The furthest the image can be magnified beyond the size at which it just fits.
pub const MAX_ZOOM: f32 = 50.0;

/// How an image is placed in its viewport.
///
/// The image is first flipped, then turned, then scaled and shifted. Because flips come before
/// turns, use [`ViewTransform::flipped`] and [`ViewTransform::turned_clockwise`] to change the
/// picture as it is seen, in place of setting the fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// Magnification relative to the size at which the whole image just fits the viewport.
    pub zoom: f32,
    /// How far the middle of the image is from the middle of the viewport, in screen pixels.
    pub pan: [f32; 2],
    /// Mirror the image left to right, before any turn.
    pub flip_h: bool,
    /// Mirror the image top to bottom, before any turn.
    pub flip_v: bool,
    /// Quarter turns clockwise, 0 to 3.
    pub quarter_turns: u8,
}

impl ViewTransform {
    /// The whole image, as large as the viewport allows, in the middle, neither flipped nor
    /// turned.
    pub fn fit() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
            flip_h: false,
            flip_v: false,
            quarter_turns: 0,
        }
    }

    /// The transform from the image grid to the viewport.
    ///
    /// `image` is columns then rows; `viewport` is width then height in screen pixels;
    /// `pixel_aspect` is how many times taller than wide one pixel of the image is. Voxel
    /// coordinates count from voxel centres, so the image's first corner is voxel
    /// (-0.5, -0.5). Sizes or a zoom that are not positive finite numbers are read as 1, so
    /// the result can always be undone.
    pub fn image_to_screen(
        &self,
        image: [u32; 2],
        viewport: [f32; 2],
        pixel_aspect: f32,
    ) -> Affine<Voxel, Screen> {
        let [cols, rows] = image.map(f64::from);
        let [wide, high] = viewport.map(f64::from).map(|size| positive_or(size, 1.0));
        let aspect = positive_or(f64::from(pixel_aspect), 1.0);
        // The image measured in widths of one of its pixels, as it lies after any turn.
        let (across, down) = if self.quarter_turns % 2 == 1 {
            (rows * aspect, cols)
        } else {
            (cols, rows * aspect)
        };
        let fitted = positive_or((wide / across).min(high / down), 1.0);
        let scale = fitted * positive_or(f64::from(self.zoom), 1.0);

        let [col_x, col_y] = self.placed([1.0, 0.0]).map(|step| step * scale);
        let [row_x, row_y] = self.placed([0.0, aspect]).map(|step| step * scale);
        let [pan_x, pan_y] = self.pan.map(f64::from).map(|shift| finite_or(shift, 0.0));
        let (middle_col, middle_row) = ((cols - 1.0) / 2.0, (rows - 1.0) / 2.0);
        let origin = [
            wide / 2.0 + pan_x - middle_col * col_x - middle_row * row_x,
            high / 2.0 + pan_y - middle_col * col_y - middle_row * row_y,
            0.0,
        ];
        Affine::from_columns(
            [col_x, col_y, 0.0],
            [row_x, row_y, 0.0],
            [0.0, 0.0, 1.0],
            origin,
        )
    }

    /// The same view with the picture turned a quarter turn clockwise as it is seen.
    #[must_use]
    pub fn turned_clockwise(self) -> Self {
        Self {
            quarter_turns: (self.quarter_turns % 4 + 1) % 4,
            ..self
        }
    }

    /// The same view with the picture mirrored as it is seen: left to right when `horizontal`,
    /// otherwise top to bottom.
    #[must_use]
    pub fn flipped(self, horizontal: bool) -> Self {
        // Flips act on the image before it is turned. After an odd number of quarter turns
        // the image's rows lie across the screen, so the other flip is the one wanted.
        let lying_on_its_side = self.quarter_turns % 2 == 1;
        if horizontal == lying_on_its_side {
            Self {
                flip_v: !self.flip_v,
                ..self
            }
        } else {
            Self {
                flip_h: !self.flip_h,
                ..self
            }
        }
    }

    /// The same view magnified by `factor`, keeping still whatever lies under the point
    /// `about`, given in screen pixels from the middle of the viewport. The zoom stays
    /// between [`MIN_ZOOM`] and [`MAX_ZOOM`]; a factor that is not a number changes nothing.
    #[must_use]
    pub fn zoomed_about(self, factor: f32, about: [f32; 2]) -> Self {
        if !factor.is_finite() || !self.zoom.is_finite() || self.zoom <= 0.0 {
            return self;
        }
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let applied = zoom / self.zoom;
        let [about_x, about_y] = about;
        let [pan_x, pan_y] = self.pan;
        let pan = [
            about_x + (pan_x - about_x) * applied,
            about_y + (pan_y - about_y) * applied,
        ];
        Self { zoom, pan, ..self }
    }

    /// A direction in the image as it lies on the screen: flipped, then turned.
    fn placed(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let x = if self.flip_h { -x } else { x };
        let y = if self.flip_v { -y } else { y };
        // The screen's y runs downwards, so a clockwise quarter turn takes (x, y) to (-y, x).
        match self.quarter_turns % 4 {
            0 => [x, y],
            1 => [-y, x],
            2 => [-x, -y],
            _ => [y, -x],
        }
    }
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self::fit()
    }
}

pub(crate) fn positive_or(value: f64, otherwise: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        otherwise
    }
}

pub(crate) fn finite_or(value: f64, otherwise: f64) -> f64 {
    if value.is_finite() { value } else { otherwise }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxa_core::Point;

    const VIEWPORT: [f32; 2] = [400.0, 400.0];
    /// 200 columns by 100 rows.
    const IMAGE: [u32; 2] = [200, 100];
    /// The four corners of the image, half a voxel outside the centres of its corner voxels.
    const CORNERS: [(f64, f64); 4] = [(-0.5, -0.5), (199.5, -0.5), (-0.5, 99.5), (199.5, 99.5)];

    fn on_screen(view: &ViewTransform, aspect: f32, (col, row): (f64, f64)) -> (f64, f64) {
        let to_screen = view.image_to_screen(IMAGE, VIEWPORT, aspect);
        let p = to_screen.apply(Point::new(col, row, 0.0));
        (p.x, p.y)
    }

    /// Where the image's first corner (top left as stored) and last corner (bottom right) land.
    fn span(view: &ViewTransform, aspect: f32) -> ((f64, f64), (f64, f64)) {
        (
            on_screen(view, aspect, CORNERS[0]),
            on_screen(view, aspect, CORNERS[3]),
        )
    }

    #[test]
    fn fit_centres_the_image_at_the_largest_size_that_shows_all_of_it() {
        let view = ViewTransform::fit();
        // Two screen pixels per voxel: 400 wide, 200 high, in the middle of the viewport.
        assert_eq!(span(&view, 1.0), ((0.0, 100.0), (400.0, 300.0)));
        // Voxel coordinates count from voxel centres, half a voxel in from the corner.
        assert_eq!(on_screen(&view, 1.0, (0.0, 0.0)), (1.0, 101.0));
    }

    #[test]
    fn flipping_left_to_right_mirrors_x() {
        let view = ViewTransform {
            flip_h: true,
            ..ViewTransform::fit()
        };
        assert_eq!(span(&view, 1.0), ((400.0, 100.0), (0.0, 300.0)));
    }

    #[test]
    fn flipping_top_to_bottom_mirrors_y() {
        let view = ViewTransform {
            flip_v: true,
            ..ViewTransform::fit()
        };
        assert_eq!(span(&view, 1.0), ((0.0, 300.0), (400.0, 100.0)));
    }

    #[test]
    fn a_quarter_turn_swaps_the_fitted_extents() {
        let view = ViewTransform {
            quarter_turns: 1,
            ..ViewTransform::fit()
        };
        // Now 200 wide and 400 high; the first corner has turned clockwise to the top right.
        assert_eq!(span(&view, 1.0), ((300.0, 0.0), (100.0, 400.0)));
    }

    #[test]
    fn pixels_twice_as_tall_as_wide_double_the_height_on_screen() {
        assert_eq!(
            span(&ViewTransform::fit(), 2.0),
            ((0.0, 0.0), (400.0, 400.0))
        );
    }

    #[test]
    fn zoom_magnifies_about_the_centre_and_pan_shifts_the_result() {
        let view = ViewTransform {
            zoom: 2.0,
            pan: [10.0, -20.0],
            ..ViewTransform::fit()
        };
        assert_eq!(span(&view, 1.0), ((-190.0, -20.0), (610.0, 380.0)));
    }

    #[test]
    fn four_quarter_turns_come_back_to_the_start() {
        let start = ViewTransform {
            flip_h: true,
            zoom: 1.5,
            ..ViewTransform::fit()
        };
        let mut view = start;
        for _ in 0..4 {
            view = view.turned_clockwise();
        }
        assert_eq!(view, start);
    }

    #[test]
    fn turning_turns_the_picture_as_it_is_seen_even_when_flipped() {
        let flipped = ViewTransform {
            flip_h: true,
            ..ViewTransform::fit()
        };
        assert_eq!(
            on_screen(&flipped, 1.0, CORNERS[1]),
            (0.0, 100.0),
            "at the top left"
        );
        let turned = flipped.turned_clockwise();
        assert_eq!(
            on_screen(&turned, 1.0, CORNERS[1]),
            (300.0, 0.0),
            "now at the top right"
        );
    }

    #[test]
    fn flipping_mirrors_the_picture_as_it_is_seen_even_after_a_turn() {
        let turned = ViewTransform::fit().turned_clockwise();
        let (sideways, upended) = (turned.flipped(true), turned.flipped(false));
        for corner in CORNERS {
            let (x, y) = on_screen(&turned, 1.0, corner);
            assert_eq!(on_screen(&sideways, 1.0, corner), (400.0 - x, y));
            assert_eq!(on_screen(&upended, 1.0, corner), (x, 400.0 - y));
        }
    }

    #[test]
    fn zooming_about_a_point_keeps_what_is_under_it() {
        let view = ViewTransform {
            pan: [30.0, -10.0],
            ..ViewTransform::fit()
        };
        let about = [80.0, -40.0]; // from the middle of the viewport
        let zoomed = view.zoomed_about(1.5, about);
        assert_eq!(zoomed.zoom, 1.5);
        let under = |view: &ViewTransform| {
            let to_image = view
                .image_to_screen(IMAGE, VIEWPORT, 1.0)
                .inverse()
                .expect("can be undone");
            to_image.apply(Point::new(280.0, 160.0, 0.0))
        };
        let (before, after) = (under(&view), under(&zoomed));
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
    }

    #[test]
    fn zoom_stays_within_its_limits() {
        let fit = ViewTransform::fit();
        assert_eq!(fit.zoomed_about(1.0e9, [0.0, 0.0]).zoom, MAX_ZOOM);
        assert_eq!(fit.zoomed_about(0.0, [0.0, 0.0]).zoom, MIN_ZOOM);
        assert_eq!(fit.zoomed_about(f32::NAN, [5.0, 5.0]), fit);
    }

    #[test]
    fn sizes_of_nothing_still_give_a_transform_that_can_be_undone() {
        let cases: [([u32; 2], [f32; 2], f32); 5] = [
            ([0, 0], VIEWPORT, 1.0),
            (IMAGE, [0.0, 0.0], 1.0),
            (IMAGE, VIEWPORT, 0.0),
            (IMAGE, VIEWPORT, f32::NAN),
            (IMAGE, [f32::INFINITY, 400.0], 1.0),
        ];
        for (image, viewport, aspect) in cases {
            let broken = ViewTransform {
                zoom: f32::NAN,
                ..ViewTransform::fit()
            };
            for view in [ViewTransform::fit(), broken] {
                let to_screen = view.image_to_screen(image, viewport, aspect);
                assert!(
                    to_screen.inverse().is_some(),
                    "{image:?} {viewport:?} {aspect}"
                );
            }
        }
    }
}
