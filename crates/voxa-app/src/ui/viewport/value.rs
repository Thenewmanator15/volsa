//! The value of the picture under the pointer: which pixel it is over, and the one value asked
//! for. The slice itself is never built for it, as it is asked for whenever the pointer moves.

use egui::Pos2;
use voxa_core::{Affine, Point, Screen, Voxel};
use voxa_render::ViewTransform;

use super::{Pane, ValueAt, pixel_aspect};

/// The value of the slice a viewport shows under `pointer`, a place in the window, read by
/// `value_at`. `None` when there is no pointer over it, the pointer is off the picture, or the
/// value is not there to be read.
pub(super) fn under_pointer(
    pane: &Pane<'_>,
    value_at: &ValueAt<'_>,
    pointer: Option<Pos2>,
) -> Option<f32> {
    // The pointer is asked first: nothing is looked up while it is not over the viewport.
    let from_corner = pointer? - pane.cell.min;
    let viewport = pane.state.viewports.get(pane.at)?;
    let series = viewport.series.as_ref()?;
    let info = pane.state.shown_info(pane.at)?;
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    let at = [from_corner.x, from_corner.y];
    let pixel = pixel_at(&pane.shown, size, aspect, [info.cols, info.rows], at)?;
    value_at(series, viewport.slice, pixel)
}

/// The pixel, as its column and row, that `pointer` is over, in points from the top left corner
/// of a viewport of `size` that draws a picture of `image` (columns, rows) through `shown` with
/// pixels `aspect` times taller than wide. `None` off the picture.
pub(super) fn pixel_at(
    shown: &ViewTransform,
    size: [f32; 2],
    aspect: f32,
    image: [u32; 2],
    [x, y]: [f32; 2],
) -> Option<(usize, usize)> {
    let to_pixel = screen_to_image(shown, size, aspect, image)?;
    let at = to_pixel.apply(Point::new(f64::from(x), f64::from(y), 0.0));
    let [cols, rows] = image;
    Some((pixel(at.x, cols)?, pixel(at.y, rows)?))
}

/// From points of a viewport of `size`, counted from its top left corner, to the columns and
/// rows of the picture of `image` (columns, rows) it draws through `shown` with pixels `aspect`
/// times taller than wide, counting from the middles of pixels. `None` for a picture drawn so
/// that the way back cannot be found.
pub(super) fn screen_to_image(
    shown: &ViewTransform,
    size: [f32; 2],
    aspect: f32,
    image: [u32; 2],
) -> Option<Affine<Screen, Voxel>> {
    shown.image_to_screen(image, size, aspect).inverse()
}

/// Which of `count` pixels a coordinate lies in, if it lies in one. Coordinates count from the
/// middles of pixels, so the first pixel spans -0.5 to 0.5.
fn pixel(coord: f64, count: u32) -> Option<usize> {
    let nearest = (coord + 0.5).floor();
    // A coordinate that is not a number is in no range.
    (0.0..f64::from(count))
        .contains(&nearest)
        .then(|| whole(nearest))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "checked to lie between 0 and the number of pixels first"
)]
fn whole(coord: f64) -> usize {
    coord as usize
}
