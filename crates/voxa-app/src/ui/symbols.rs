//! The small symbols of the bars, drawn with lines so that no picture file or icon font is
//! needed: the default fonts have no glyph for several of them. Three dots, a sun, a moon,
//! a screen, a magnifying glass, an `i` in a ring and a warning triangle.

use std::f32::consts::TAU;

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

/// How wide and tall the room reserved for a symbol is, in points.
pub const SIZE: f32 = 16.0;
/// The width of the lines a symbol is drawn with, in points.
const LINE: f32 = 1.5;
/// How many corners the curve of a moon has along each of its two edges.
const CURVE: u8 = 14;

/// A symbol of the bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    /// Three dots in a row: the menu.
    More,
    /// A sun: the light theme.
    Sun,
    /// A moon: the dark theme.
    Moon,
    /// A screen: the theme the system chooses.
    Screen,
    /// A magnifying glass: search.
    Search,
    /// An `i` in a ring: a notice that is information.
    Info,
    /// A triangle with a mark in it: a notice to look at.
    Warning,
}

/// Draw `symbol` in the middle of `rect`, in `colour`.
pub fn paint(painter: &Painter, rect: Rect, symbol: Symbol, colour: Color32) {
    for shape in shapes(rect, symbol, colour) {
        painter.add(shape);
    }
}

/// The shapes of `symbol` in the middle of `rect`, in `colour`.
fn shapes(rect: Rect, symbol: Symbol, colour: Color32) -> Vec<Shape> {
    let reach = rect.width().min(rect.height()) / 2.0;
    let stroke = Stroke::new(LINE, colour);
    let at = |x: f32, y: f32| rect.center() + vec2(x, y) * reach;
    let line = |from: [f32; 2], to: [f32; 2]| {
        Shape::line_segment([at(from[0], from[1]), at(to[0], to[1])], stroke)
    };
    let dot = |x: f32, y: f32, radius: f32| Shape::circle_filled(at(x, y), radius * reach, colour);
    let ring = |x: f32, y: f32, radius: f32| Shape::circle_stroke(at(x, y), radius * reach, stroke);
    let outline = |points: &[[f32; 2]]| {
        let points: Vec<Pos2> = points.iter().map(|[x, y]| at(*x, *y)).collect();
        Shape::closed_line(points, stroke)
    };
    match symbol {
        Symbol::More => [-0.75, 0.0, 0.75]
            .into_iter()
            .map(|x| dot(x, 0.0, 0.17))
            .collect(),
        Symbol::Sun => {
            let ray = |i: u8| {
                let angle = TAU * f32::from(i) / 8.0;
                let (sin, cos) = angle.sin_cos();
                line([cos * 0.68, sin * 0.68], [cos * 0.98, sin * 0.98])
            };
            std::iter::once(dot(0.0, 0.0, 0.4))
                .chain((0..8).map(ray))
                .collect()
        }
        Symbol::Moon => vec![outline(&crescent())],
        Symbol::Screen => {
            let panel = Rect::from_two_pos(at(-0.95, -0.7), at(0.95, 0.35));
            vec![
                Shape::rect_stroke(panel, 1.0, stroke, egui::StrokeKind::Inside),
                line([0.0, 0.35], [0.0, 0.75]),
                line([-0.5, 0.8], [0.5, 0.8]),
            ]
        }
        Symbol::Search => vec![ring(-0.2, -0.2, 0.58), line([0.22, 0.22], [0.92, 0.92])],
        Symbol::Info => vec![
            ring(0.0, 0.0, 0.95),
            dot(0.0, -0.45, 0.1),
            line([0.0, -0.1], [0.0, 0.5]),
        ],
        Symbol::Warning => vec![
            outline(&[[0.0, -0.9], [0.98, 0.8], [-0.98, 0.8]]),
            line([0.0, -0.3], [0.0, 0.25]),
            dot(0.0, 0.52, 0.08),
        ],
    }
}

/// The corners of a crescent in a square from -1 to 1: the part of a circle that a smaller
/// circle, nearer its upper right, does not cover.
fn crescent() -> Vec<[f32; 2]> {
    let (whole, cut_at, cut) = (0.95_f32, [0.45_f32, -0.4_f32], 0.8_f32);
    let apart = cut_at[0].hypot(cut_at[1]);
    // Where the two circles' edges cross.
    let along = (whole * whole - cut * cut + apart * apart) / (2.0 * apart);
    let across = (whole * whole - along * along).max(0.0).sqrt();
    let (ux, uy) = (cut_at[0] / apart, cut_at[1] / apart);
    let crossing = |side: f32| {
        [
            along * ux - side * across * uy,
            along * uy + side * across * ux,
        ]
    };
    let (first, last) = (crossing(1.0), crossing(-1.0));
    let angle_of = |from: [f32; 2], to: [f32; 2]| (to[1] - from[1]).atan2(to[0] - from[0]);
    // The long way round the whole circle, from one crossing to the other, away from the cut.
    let start = angle_of([0.0, 0.0], first);
    let mut end = angle_of([0.0, 0.0], last);
    if end < start {
        end += TAU;
    }
    let steps = f32::from(CURVE);
    let outer = (0..=CURVE).map(|i| {
        let angle = start + (end - start) * f32::from(i) / steps;
        [whole * angle.cos(), whole * angle.sin()]
    });
    // And back along the cut, on the side of it that is inside the whole circle.
    let from = angle_of(cut_at, last);
    let mut to = angle_of(cut_at, first);
    if to > from {
        to -= TAU;
    }
    let inner = (0..=CURVE).map(|i| {
        let angle = from + (to - from) * f32::from(i) / steps;
        [cut_at[0] + cut * angle.cos(), cut_at[1] + cut * angle.sin()]
    });
    // The outer edge ends where the inner begins, and the inner where the outer begins.
    outer.chain(inner).collect()
}

#[cfg(test)]
mod tests {
    use egui::pos2;

    use super::*;

    const ALL: [Symbol; 7] = [
        Symbol::More,
        Symbol::Sun,
        Symbol::Moon,
        Symbol::Screen,
        Symbol::Search,
        Symbol::Info,
        Symbol::Warning,
    ];

    #[test]
    fn each_symbol_is_drawn_inside_its_room_and_the_symbols_differ() {
        let room = Rect::from_min_size(pos2(100.0, 40.0), vec2(SIZE, SIZE));
        let mut all = Vec::new();
        for symbol in ALL {
            let drawn = shapes(room, symbol, Color32::WHITE);
            assert!(!drawn.is_empty(), "{symbol:?} is drawn");
            for shape in &drawn {
                let bounds = shape.visual_bounding_rect();
                // A line is as wide as it is drawn, half of it either side of its path.
                assert!(
                    room.expand(LINE).contains_rect(bounds),
                    "{symbol:?}: {bounds:?} is outside {room:?}"
                );
            }
            let at: Vec<Rect> = drawn.iter().map(Shape::visual_bounding_rect).collect();
            assert!(!all.contains(&at), "{symbol:?} is drawn as another is");
            all.push(at);
        }
    }

    #[test]
    fn the_moon_is_a_crescent_that_leaves_the_middle_of_its_circle_clear() {
        let corners = crescent();
        assert!(corners.len() > 10, "{corners:?}");
        // Every corner is on the whole circle or on the cut, and none is inside the cut.
        for [x, y] in &corners {
            let from_cut = (x - 0.45).hypot(y + 0.4);
            assert!(from_cut >= 0.8 - 1e-3, "({x}, {y}) is inside the cut");
            assert!(
                x.hypot(*y) <= 0.95 + 1e-3,
                "({x}, {y}) is outside the circle"
            );
        }
    }
}
