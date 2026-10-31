//! The value of the picture under the pointer.

use egui::Pos2;
use voxa_core::Point;
use voxa_dicom::Frame;
use voxa_render::ViewTransform;

use super::{Frames, Pane, pixel_aspect};

/// The value of the slice a viewport shows at `pointer`, a place in the window. `None` when
/// there is no pointer over it, the slice is not decoded yet, or the pointer is off the
/// picture.
pub(super) fn under_pointer(
    pane: &Pane<'_>,
    frames: &Frames<'_>,
    pointer: Option<Pos2>,
) -> Option<f32> {
    // The pointer is asked first: the slice is looked up only while it is over the viewport.
    let from_corner = pointer? - pane.cell.min;
    let viewport = pane.state.viewports.get(pane.at)?;
    let frame = frames(viewport.series.as_ref()?, viewport.slice)?;
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    let at = [from_corner.x, from_corner.y];
    value_at(&frame, &pane.shown, size, aspect, at)
}

/// The value of `frame` at `pointer`, in points from the top left corner of a viewport of
/// `size` that draws it through `shown` with pixels `aspect` times taller than wide. `None`
/// off the picture, for a picture stored in colour (its values only say how bright it is), and
/// when the frame lacks the pixel.
pub(super) fn value_at(
    frame: &Frame,
    shown: &ViewTransform,
    size: [f32; 2],
    aspect: f32,
    [x, y]: [f32; 2],
) -> Option<f32> {
    if frame.colour.is_some() {
        return None;
    }
    let to_pixel = shown
        .image_to_screen([frame.cols, frame.rows], size, aspect)
        .inverse()?;
    let at = to_pixel.apply(Point::new(f64::from(x), f64::from(y), 0.0));
    let col = pixel(at.x, frame.cols)?;
    let row = pixel(at.y, frame.rows)?;
    let place = row.checked_mul(usize::try_from(frame.cols).ok()?)?;
    frame.values.get(place.checked_add(col)?).copied()
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
