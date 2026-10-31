//! Annotations as they are seen in a plane view: which of them lie in the layer a panel
//! shows, and where their points fall in the panel.

use egui::{Align2, Color32, Pos2, Rect, Shape as Painted, Stroke, pos2, vec2};
use voxa_core::{Patient, Point};
use voxa_render::PlaneView;
use voxa_volume::Plane;

use super::super::pen::Pen;
use crate::annotations::{Annotation, Shape};

/// How nearly a region's own plane must face the way a panel's plane does for the region to
/// be shown there: the cosine of the angle between them.
const SAME_WAY: f64 = 0.99;
/// The radius of the dot at each placed point, in points.
const DOT: f32 = 3.0;
const LINE: f32 = 1.5;
/// How many straight pieces an ellipse is drawn in.
const PIECES: u8 = 48;

/// Whether an annotation lies in the layer a plane shows: every one of its points within
/// `half_layer_mm` of the plane, and, for a region, drawn in a plane that faces the same way.
pub fn shown_on(shape: &Shape, plane: &Plane, half_layer_mm: f64) -> bool {
    let near = |at: &[f64; 3]| plane.of(point(*at)).1.abs() <= half_layer_mm;
    match shape {
        Shape::Region {
            centre,
            across,
            down,
            ..
        } => {
            let [x, y, z] = cross(*across, *down);
            let [nx, ny, nz] = plane.normal();
            near(centre) && (x * nx + y * ny + z * nz).abs() >= SAME_WAY
        }
        other => other.points().iter().all(near),
    }
}

/// An annotation as it falls in a panel, in points from the panel's top left corner.
#[derive(Debug, Clone, PartialEq)]
pub enum Drawn {
    /// A ruler: its two ends.
    Line([[f32; 2]; 2]),
    /// An angle: the end of one arm, the vertex, the end of the other arm.
    Arms([[f32; 2]; 3]),
    /// A region: its middle and its half-widths across and down the panel.
    Ellipse {
        /// The middle.
        centre: [f32; 2],
        /// The half-widths.
        radii: [f32; 2],
    },
    /// A marker.
    Dot([f32; 2]),
    /// A shape seen at a slant: the points of its outline, going round.
    Outline(Vec<[f32; 2]>),
}

/// Where an annotation falls in a panel of `viewport` (width, height) points.
pub fn drawn(shape: &Shape, view: &PlaneView, viewport: [f32; 2]) -> Drawn {
    let seen = |at: &[f64; 3]| view.patient_to_screen(viewport, point(*at)).0;
    match shape {
        Shape::Ruler { a, b } => Drawn::Line([seen(a), seen(b)]),
        Shape::Angle { a, vertex, b } => Drawn::Arms([seen(a), seen(vertex), seen(b)]),
        Shape::Region {
            centre, radii_mm, ..
        } => Drawn::Ellipse {
            centre: seen(centre),
            radii: radii_mm.map(|mm| narrowed(mm / view.mm_per_px)),
        },
        Shape::Marker { at } => Drawn::Dot(seen(at)),
    }
}

/// Where an annotation falls in a picture whose points `seen` places, in points from its top
/// left corner. A region, which may be seen at a slant, or in a picture that is turned or
/// mirrored, is the outline of its ellipse.
pub fn drawn_through(shape: &Shape, seen: impl Fn([f64; 3]) -> [f32; 2]) -> Drawn {
    match shape {
        Shape::Ruler { a, b } => Drawn::Line([seen(*a), seen(*b)]),
        Shape::Angle { a, vertex, b } => Drawn::Arms([seen(*a), seen(*vertex), seen(*b)]),
        Shape::Marker { at } => Drawn::Dot(seen(*at)),
        Shape::Region {
            centre,
            across,
            down,
            radii_mm: [wide, high],
            ..
        } => {
            let round = (0..PIECES).map(|piece| {
                let turn = std::f64::consts::TAU * f64::from(piece) / f64::from(PIECES);
                let (along, up) = (wide * turn.cos(), high * turn.sin());
                let at = [0, 1, 2].map(|part| {
                    part_of(*centre, part)
                        + along * part_of(*across, part)
                        + up * part_of(*down, part)
                });
                seen(at)
            });
            Drawn::Outline(round.collect())
        }
    }
}

impl Drawn {
    /// Where words about the annotation are written: beside its last point, or above a
    /// region.
    pub fn label_at(&self) -> [f32; 2] {
        match *self {
            Self::Line([_, [x, y]]) | Self::Arms([_, [x, y], _]) | Self::Dot([x, y]) => {
                [x + 2.0 * DOT, y - 2.0 * DOT]
            }
            Self::Outline(ref round) => {
                // Above the highest point of it.
                let top = round.iter().copied().reduce(|top, at| {
                    let ([_, high], [_, y]) = (top, at);
                    if y < high { at } else { top }
                });
                let [x, y] = top.unwrap_or_default();
                [x, y - 2.0 * DOT]
            }
            Self::Ellipse {
                centre: [x, y],
                radii: [_, high],
            } => [x, y - high - 2.0 * DOT],
        }
    }
}

/// The words written beside an annotation on a picture: its name and what it measures. A
/// marker measures nothing, and where it is can be read in the Measure tab, so its
/// name stands alone.
pub fn beside(item: &Annotation, value: &str) -> String {
    match item.shape {
        Shape::Marker { .. } => item.label.clone(),
        _ => format!("{}: {value}", item.label),
    }
}

/// Where words `wide` points long begin so as to stay inside a panel that reaches from
/// `left` to `right`: at `at`, where they belong, or as far to the left of it as it takes; if
/// they are longer than the panel, at its left edge.
pub fn label_left(at: f32, wide: f32, (left, right): (f32, f32)) -> f32 {
    at.min(right - wide).max(left)
}

/// How far down the bottom of words `high` points tall is put so that they begin no higher
/// than `clear`, beneath what is written along the top of a panel: at `at`, where they
/// belong, or as far beneath it as it takes.
pub fn label_bottom(at: f32, high: f32, clear: f32) -> f32 {
    at.max(clear + high)
}

/// Paint an annotation over a panel at `cell`, in one colour, with words beside it. The
/// words are kept inside the panel, and out of the `top` points along its top where its
/// own words are.
pub fn paint(pen: &Pen, (cell, top): (Rect, f32), drawn: &Drawn, colour: Color32, words: &str) {
    let placed = |[x, y]: [f32; 2]| cell.min + vec2(x, y);
    let stroke = Stroke::new(LINE, colour);
    let dot = |at: [f32; 2]| {
        pen.painter.circle_filled(placed(at), DOT, colour);
    };
    match *drawn {
        Drawn::Line([a, b]) => {
            pen.painter.line_segment([placed(a), placed(b)], stroke);
            for at in [a, b] {
                dot(at);
            }
        }
        Drawn::Arms([a, vertex, b]) => {
            pen.painter
                .line_segment([placed(a), placed(vertex)], stroke);
            pen.painter
                .line_segment([placed(vertex), placed(b)], stroke);
            for at in [a, vertex, b] {
                dot(at);
            }
        }
        Drawn::Ellipse {
            centre: [x, y],
            radii: [wide, high],
        } => {
            let round = (0..PIECES).map(|piece| {
                let turn = std::f32::consts::TAU * f32::from(piece) / f32::from(PIECES);
                placed([x + wide * turn.cos(), y + high * turn.sin()])
            });
            let outline: Vec<Pos2> = round.collect();
            pen.painter.add(Painted::closed_line(outline, stroke));
        }
        Drawn::Dot(at) => {
            pen.painter.circle_stroke(placed(at), 2.0 * DOT, stroke);
            dot(at);
        }
        Drawn::Outline(ref round) => {
            let outline: Vec<Pos2> = round.iter().map(|at| placed(*at)).collect();
            pen.painter.add(Painted::closed_line(outline, stroke));
        }
    }
    if !words.is_empty() {
        let at = placed(drawn.label_at());
        let laid_out = pen
            .painter
            .layout_no_wrap(words.to_owned(), pen.small.clone(), colour);
        // Beside the point, moved as far as it takes to stay inside the panel.
        let inside = cell.shrink(DOT);
        let size = laid_out.size();
        let left = label_left(at.x, size.x, (inside.left(), inside.right()));
        let bottom = label_bottom(at.y, size.y, inside.top() + top);
        pen.write(pos2(left, bottom), Align2::LEFT_BOTTOM, words, &pen.small);
    }
}

fn point([x, y, z]: [f64; 3]) -> Point<Patient> {
    Point::new(x, y, z)
}

fn part_of([x, y, z]: [f64; 3], part: usize) -> f64 {
    match part {
        0 => x,
        1 => y,
        _ => z,
    }
}

fn cross([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a length on the screen is small"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}
