//! Writing over a picture: words that can be read over anything, the letter at each edge, and
//! a scale bar. The 2D viewports and the 3D scene are written over in the same hand.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{Align, Align2, Color32, FontId, Galley, Pos2, Rect, Stroke, Ui, pos2, vec2};

use super::overlay::EdgeLabels;

/// How far in from the edge of a picture its words are written.
pub const INSET: f32 = 10.0;
/// The most of a picture's width a scale bar may take.
pub const BAR_SHARE: f32 = 0.3;
/// The room left in the middle of an edge for the letter written there.
pub const LETTER_ROOM: f32 = 14.0;
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
    /// One line of small type, cut short with an ellipsis if it is longer than `width`.
    fn line(&self, text: &str, colour: Color32, width: f32) -> Arc<Galley> {
        let mut job = LayoutJob::simple_singleline(text.to_owned(), self.small.clone(), colour);
        job.wrap = TextWrapping::truncate_at_width(width);
        self.painter.layout_job(job)
    }

    /// Write lines in a corner of a picture, one beneath another, each cut short if it is
    /// longer than `width`. `at` is the corner and `anchor` says which corner it is.
    pub fn write_lines(&self, at: Pos2, anchor: Align2, lines: &[String], width: f32) {
        let inked: Vec<_> = lines
            .iter()
            .map(|line| self.line(line, INK, width))
            .collect();
        let height: f32 = inked.iter().map(|words| words.size().y).sum();
        let mut top = if anchor.y() == Align::Max {
            at.y - height
        } else {
            at.y
        };
        for (line, words) in lines.iter().zip(inked) {
            let size = words.size();
            let left = if anchor.x() == Align::Max {
                at.x - size.x
            } else {
                at.x
            };
            let shadow = self.line(line, Color32::BLACK, width);
            self.painter
                .galley(pos2(left + 1.0, top + 1.0), shadow, Color32::BLACK);
            self.painter.galley(pos2(left, top), words, INK);
            top += size.y;
        }
    }

    /// Write some words downwards from the top left corner `at`, broken into lines no longer
    /// than `width`.
    pub fn write_wrapped(&self, at: Pos2, text: &str, width: f32) {
        let laid_out = |colour| {
            let (text, font) = (text.to_owned(), self.small.clone());
            self.painter.layout(text, font, colour, width)
        };
        self.painter.galley(
            at + vec2(1.0, 1.0),
            laid_out(Color32::BLACK),
            Color32::BLACK,
        );
        self.painter.galley(at, laid_out(INK), INK);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_too_long_for_its_room_is_cut_short_and_the_rest_are_written_whole() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
            let pen = Pen::over(ui, area);
            let long = "A description that goes on a great deal longer than its corner";
            let cut = pen.line(long, INK, 80.0);
            assert!(cut.elided && cut.size().x <= 81.0, "{}", cut.size().x);
            let short = pen.line("Slice 2 of 9", INK, 200.0);
            assert!(!short.elided);
            let lines = [long.to_owned(), "Slice 2 of 9".to_owned()];
            for anchor in [
                Align2::LEFT_TOP,
                Align2::RIGHT_TOP,
                Align2::LEFT_BOTTOM,
                Align2::RIGHT_BOTTOM,
            ] {
                pen.write_lines(area.center(), anchor, &lines, 80.0);
            }
            pen.write_wrapped(area.left_top(), long, 120.0);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
    }
}
