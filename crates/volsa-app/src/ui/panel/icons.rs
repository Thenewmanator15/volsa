//! The icons of the folded panel's tabs, drawn with lines so that no picture file or icon
//! font is needed: an eye, a ruler, a cube, a tag and some sliders.

use std::f32::consts::{FRAC_PI_3, FRAC_PI_6};

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};

use super::Tab;

/// How far from the middle of its button an icon reaches, in points.
const REACH: f32 = 10.0;
/// The width of the lines an icon is drawn with, in points.
const LINE: f32 = 1.6;

/// Draw the icon of `tab` in the middle of `rect`, in `colour`.
pub fn paint(painter: &Painter, rect: Rect, tab: Tab, colour: Color32) {
    for shape in shapes(rect, tab, colour) {
        painter.add(shape);
    }
}

/// The shapes of the icon of `tab` in the middle of `rect`, in `colour`.
fn shapes(rect: Rect, tab: Tab, colour: Color32) -> Vec<Shape> {
    let stroke = Stroke::new(LINE, colour);
    let at = |x: f32, y: f32| rect.center() + vec2(x, y) * REACH;
    let outline = |points: &[[f32; 2]]| {
        let points: Vec<Pos2> = points.iter().map(|[x, y]| at(*x, *y)).collect();
        Shape::closed_line(points, stroke)
    };
    let line = |from: [f32; 2], to: [f32; 2]| {
        Shape::line_segment([at(from[0], from[1]), at(to[0], to[1])], stroke)
    };
    let ring = |middle: [f32; 2], radius: f32| {
        Shape::circle_stroke(at(middle[0], middle[1]), radius * REACH, stroke)
    };
    match tab {
        Tab::View => {
            // An eye: the lids meeting at the corners, and the pupil.
            let across = |i: u8| -1.0 + f32::from(i) / 4.0;
            let upper = (0..=8).map(|i| [across(i), -0.55 * (1.0 - across(i).powi(2))]);
            let lower = (0..=8)
                .rev()
                .map(|i| [across(i), 0.55 * (1.0 - across(i).powi(2))]);
            let lids: Vec<[f32; 2]> = upper.chain(lower).collect();
            vec![outline(&lids), ring([0.0, 0.0], 0.28)]
        }
        Tab::Measure => {
            // A ruler: a bar with ticks of two lengths along its upper edge.
            let bar = Rect::from_two_pos(at(-1.0, -0.4), at(1.0, 0.4));
            let mut shapes = vec![Shape::rect_stroke(bar, 1.0, stroke, StrokeKind::Inside)];
            for (x, length) in [(-0.6, 0.4), (-0.2, 0.2), (0.2, 0.4), (0.6, 0.2)] {
                shapes.push(line([x, -0.4], [x, -0.4 + length]));
            }
            shapes
        }
        Tab::Scene => {
            // A cube seen corner on: the hexagon of its outline and the three edges that
            // meet at the nearest corner.
            let corner = |i: u8| {
                let angle = FRAC_PI_3 * f32::from(i) + FRAC_PI_6;
                [angle.cos(), angle.sin()]
            };
            let hexagon: Vec<[f32; 2]> = (0..6).map(corner).collect();
            let mut shapes = vec![outline(&hexagon)];
            shapes.extend([1, 3, 5].map(|i| line([0.0, 0.0], corner(i))));
            shapes
        }
        Tab::Labels => {
            // A tag: a pointed label with a hole to hang it by.
            let label = [
                [-1.0, -0.55],
                [0.25, -0.55],
                [1.0, 0.0],
                [0.25, 0.55],
                [-1.0, 0.55],
            ];
            vec![outline(&label), ring([-0.55, 0.0], 0.14)]
        }
        Tab::Tools => {
            // Sliders: three lines, each with a knob at a different place along it.
            let knob = |x: f32, y: f32| Shape::circle_filled(at(x, y), 0.22 * REACH, colour);
            let rows = [(-0.6, -0.4), (0.0, 0.5), (0.6, -0.1)];
            let lines = rows.iter().map(|(y, _)| line([-1.0, *y], [1.0, *y]));
            lines
                .chain(rows.iter().map(|(y, x)| knob(*x, *y)))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::pos2;

    use super::*;

    #[test]
    fn each_icon_is_drawn_inside_its_button_and_the_icons_differ() {
        let button = Rect::from_min_size(pos2(100.0, 40.0), vec2(32.0, 32.0));
        let mut all = Vec::new();
        for tab in Tab::ALL {
            let drawn = shapes(button, tab, Color32::WHITE);
            assert!(!drawn.is_empty(), "{tab:?} is drawn");
            for shape in &drawn {
                let bounds = shape.visual_bounding_rect();
                assert!(
                    button.contains_rect(bounds),
                    "{tab:?}: {bounds:?} is outside {button:?}"
                );
            }
            let at: Vec<Rect> = drawn.iter().map(Shape::visual_bounding_rect).collect();
            assert!(!all.contains(&at), "{tab:?} is drawn as another tab is");
            all.push(at);
        }
    }
}
