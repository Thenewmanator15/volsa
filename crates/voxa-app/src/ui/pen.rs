//! Writing over a picture: words that can be read over anything, the letter at each edge, and
//! a scale bar. The 2D viewports and the 3D scene are written over in the same hand.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Ui, vec2};

use super::overlay::EdgeLabels;

/// How far in from the edge of a picture its words are written.
pub const INSET: f32 = 10.0;
/// The most of a picture's width a scale bar may take.
pub const BAR_SHARE: f32 = 0.3;
/// The colour of words and lines drawn over a picture.
const INK: Color32 = Color32::from_gray(230);

/// Writes words over a picture.
pub struct Pen {
    /// Draws within the picture and nowhere else.
    pub painter: egui::Painter,
    /// The type for what is in the corners.
    pub small: FontId,
    /// The type for the letters at the edges.
    pub body: FontId,
}

impl Pen {
    /// A pen that writes within `area`.
    pub fn over(ui: &Ui, area: Rect) -> Self {
        Self {
            painter: ui.painter_at(area),
            small: egui::TextStyle::Small.resolve(ui.style()),
            body: egui::TextStyle::Body.resolve(ui.style()),
        }
    }

    /// Write some words, with a dark copy just behind so they can be read over a bright
    /// picture.
    pub fn write(&self, at: Pos2, anchor: Align2, text: &str, font: &FontId) {
        let shadow = at + vec2(1.0, 1.0);
        self.painter
            .text(shadow, anchor, text, font.clone(), Color32::BLACK);
        self.painter.text(at, anchor, text, font.clone(), INK);
    }
}

impl Pen {
    /// Write some words upwards from the bottom left corner `at`, broken into lines no longer
    /// than `width`.
    pub fn write_in_lines(&self, at: Pos2, text: &str, width: f32) {
        let laid_out = |colour| {
            let (text, font) = (text.to_owned(), self.small.clone());
            self.painter.layout(text, font, colour, width)
        };
        let (shadow, words) = (laid_out(Color32::BLACK), laid_out(INK));
        let top_left = at - vec2(0.0, words.size().y);
        self.painter
            .galley(top_left + vec2(1.0, 1.0), shadow, Color32::BLACK);
        self.painter.galley(top_left, words, INK);
    }
}

/// Write the letter for each edge of a picture at the middle of that edge.
pub fn letters(pen: &Pen, inside: Rect, labels: &EdgeLabels) {
    let edges = [
        (inside.left_center(), Align2::LEFT_CENTER, &labels.left),
        (inside.right_center(), Align2::RIGHT_CENTER, &labels.right),
        (inside.center_top(), Align2::CENTER_TOP, &labels.top),
        (
            inside.center_bottom(),
            Align2::CENTER_BOTTOM,
            &labels.bottom,
        ),
    ];
    for (at, anchor, letter) in edges {
        pen.write(at, anchor, letter, &pen.body);
    }
}

/// Draw a scale bar `length` points long, with its label, above the bottom edge of a picture.
pub fn bar(pen: &Pen, inside: Rect, length: f32, label: &str) {
    let middle = inside.center_bottom() - vec2(0.0, 28.0);
    let half = vec2(length / 2.0, 0.0);
    pen.painter
        .line_segment([middle - half, middle + half], Stroke::new(2.0, INK));
    pen.write(
        middle - vec2(0.0, 4.0),
        Align2::CENTER_BOTTOM,
        label,
        &pen.small,
    );
}
