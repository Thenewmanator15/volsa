//! A slice stored in colour, drawn in its own colours: no window, the same view.

use egui::{Color32, ColorImage, Mesh, Rect, Shape, TextureHandle, TextureOptions, Ui, pos2, vec2};
use voxa_core::Point;
use voxa_dicom::Frame;
use voxa_render::ViewTransform;

use super::narrowed;

/// The picture of a frame stored in colour. `None` for a grey frame, and for one whose
/// colours are not as many as its pixels.
pub fn picture(frame: &Frame) -> Option<ColorImage> {
    let colours = frame.colour.as_ref()?;
    let size = [frame.cols as usize, frame.rows as usize];
    let whole = colours.len() == size[0] * size[1] && !colours.is_empty();
    let bytes: Vec<u8> = colours.iter().flatten().copied().collect();
    whole.then(|| ColorImage::from_rgb(size, &bytes))
}

/// The picture as something the interface can draw; `None` as [`picture`].
pub fn texture(ctx: &egui::Context, frame: &Frame) -> Option<TextureHandle> {
    picture(frame).map(|image| ctx.load_texture("colour slice", image, TextureOptions::LINEAR))
}

/// Where the four corners of an image lie in a viewport, from the viewport's own top left
/// corner: the outer corner of the first pixel, the end of the first row, the far corner, and
/// the start of the last row. `image` is columns then rows; the rest is as the grey slice is
/// drawn, so a picture in colour lies exactly where a grey one would.
pub fn corners(
    view: &ViewTransform,
    image: [u32; 2],
    viewport: [f32; 2],
    pixel_aspect: f32,
) -> [[f32; 2]; 4] {
    let to_screen = view.image_to_screen(image, viewport, pixel_aspect);
    let [cols, rows] = image.map(f64::from);
    let outer = [
        [-0.5, -0.5],
        [cols - 0.5, -0.5],
        [cols - 0.5, rows - 0.5],
        [-0.5, rows - 0.5],
    ];
    outer.map(|[column, row]| {
        let at = to_screen.apply(Point::new(column, row, 0.0));
        [narrowed(at.x), narrowed(at.y)]
    })
}

/// Draw the picture in a viewport's cell, under the viewport's view.
pub fn draw(ui: &Ui, cell: Rect, view: &ViewTransform, texture: &TextureHandle, aspect: f32) {
    let [cols, rows] = texture.size().map(|side| u32::try_from(side).unwrap_or(1));
    let placed = corners(view, [cols, rows], [cell.width(), cell.height()], aspect);
    let of_the_picture = [
        pos2(0.0, 0.0),
        pos2(1.0, 0.0),
        pos2(1.0, 1.0),
        pos2(0.0, 1.0),
    ];
    let mut mesh = Mesh::with_texture(texture.id());
    for ([x, y], uv) in placed.into_iter().zip(of_the_picture) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: cell.min + vec2(x, y),
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().with_clip_rect(cell).add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(found: [[f32; 2]; 4], wanted: [[f32; 2]; 4]) {
        let close = |a: f32, b: f32| (a - b).abs() < 0.01;
        let all = found
            .iter()
            .zip(wanted)
            .all(|(found, wanted)| close(found[0], wanted[0]) && close(found[1], wanted[1]));
        assert!(all, "{found:?} is not {wanted:?}");
    }

    #[test]
    fn a_picture_fitted_to_a_viewport_of_its_own_shape_fills_it() {
        let found = corners(&ViewTransform::fit(), [4, 2], [400.0, 200.0], 1.0);
        near(
            found,
            [[0.0, 0.0], [400.0, 0.0], [400.0, 200.0], [0.0, 200.0]],
        );
    }

    #[test]
    fn a_mirrored_picture_has_its_first_corner_on_the_other_side() {
        let view = ViewTransform::fit().flipped(true);
        let found = corners(&view, [4, 2], [400.0, 200.0], 1.0);
        near(
            found,
            [[400.0, 0.0], [0.0, 0.0], [0.0, 200.0], [400.0, 200.0]],
        );
    }

    #[test]
    fn a_picture_turned_a_quarter_has_its_first_corner_at_the_top_right() {
        let view = ViewTransform::fit().turned_clockwise();
        let found = corners(&view, [4, 2], [200.0, 400.0], 1.0);
        near(
            found,
            [[200.0, 0.0], [200.0, 400.0], [0.0, 400.0], [0.0, 0.0]],
        );
    }

    #[test]
    fn only_a_frame_in_colour_has_a_picture_of_its_own() {
        let colours = vec![[200, 10, 20], [10, 190, 30]];
        let frame = Frame {
            rows: 1,
            cols: 2,
            values: vec![0.0; 2],
            colour: Some(colours),
        };
        let image = picture(&frame).expect("in colour");
        assert_eq!(image.size, [2, 1]);
        assert_eq!(image.pixels[1], Color32::from_rgb(10, 190, 30));
        let grey = Frame {
            colour: None,
            ..frame.clone()
        };
        assert!(picture(&grey).is_none());
        let short = Frame {
            colour: Some(vec![[1, 2, 3]]),
            ..frame
        };
        assert!(picture(&short).is_none(), "fewer colours than pixels");
    }
}
