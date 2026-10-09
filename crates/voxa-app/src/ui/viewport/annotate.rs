//! What is written over a slice: corner text, a letter at each edge, and a scale bar.

use egui::{Align2, Color32, Pos2, Rect, Stroke, Ui, vec2};
use voxa_core::SeriesInfo;

use super::super::overlay::{self, EdgeLabels};
use super::{Pane, narrowed, pixel_aspect};

/// How far in from the edge of a viewport its words are written.
const INSET: f32 = 10.0;
/// The most of a viewport's width a scale bar may take.
const BAR_SHARE: f32 = 0.3;
/// The colour of words and lines drawn over a picture.
const INK: Color32 = Color32::from_gray(230);

/// Writes words over a picture.
struct Pen {
    painter: egui::Painter,
    small: egui::FontId,
    body: egui::FontId,
}

impl Pen {
    /// Write some words, with a dark copy just behind so they can be read over a bright
    /// picture.
    fn write(&self, at: Pos2, anchor: Align2, text: &str, font: &egui::FontId) {
        let shadow = at + vec2(1.0, 1.0);
        self.painter
            .text(shadow, anchor, text, font.clone(), Color32::BLACK);
        self.painter.text(at, anchor, text, font.clone(), INK);
    }
}

/// Write over a slice: what it is in the corners, and, when the files say where the slice
/// lies, a letter at each edge and a scale bar.
pub(super) fn overlays(ui: &Ui, pane: &Pane<'_>) {
    let Some((info, study)) = pane.series() else {
        return;
    };
    let Some(viewport) = pane.state.viewports.get(pane.at) else {
        return;
    };
    let pen = Pen {
        painter: ui.painter_at(pane.cell),
        small: egui::TextStyle::Small.resolve(ui.style()),
        body: egui::TextStyle::Body.resolve(ui.style()),
    };
    let inside = pane.cell.shrink(INSET);
    let placement = info.voxel_to_patient.as_ref();
    let position = placement.map(|placement| overlay::slice_position(placement, viewport.slice));
    let show_patient = pane.state.show_patient;
    let corners = overlay::corner_text(
        study,
        info,
        viewport.slice,
        position,
        viewport.wl,
        show_patient,
    );
    let anchors = [
        (inside.left_top(), Align2::LEFT_TOP),
        (inside.right_top(), Align2::RIGHT_TOP),
        (inside.left_bottom(), Align2::LEFT_BOTTOM),
        (inside.right_bottom(), Align2::RIGHT_BOTTOM),
    ];
    for (lines, (at, anchor)) in corners.iter().zip(anchors) {
        pen.write(at, anchor, &lines.join("\n"), &pen.small);
    }
    if let Some(placement) = placement {
        let radiological = pane.state.radiological;
        letters(
            &pen,
            inside,
            &overlay::orientation_labels(placement, &viewport.view, radiological),
        );
        scale_bar(&pen, pane, info, inside);
    }
}

/// Write the letter for each edge of a viewport at the middle of that edge.
fn letters(pen: &Pen, inside: Rect, labels: &EdgeLabels) {
    pen.write(
        inside.left_center(),
        Align2::LEFT_CENTER,
        &labels.left,
        &pen.body,
    );
    pen.write(
        inside.right_center(),
        Align2::RIGHT_CENTER,
        &labels.right,
        &pen.body,
    );
    pen.write(
        inside.center_top(),
        Align2::CENTER_TOP,
        &labels.top,
        &pen.body,
    );
    pen.write(
        inside.center_bottom(),
        Align2::CENTER_BOTTOM,
        &labels.bottom,
        &pen.body,
    );
}

/// Draw a bar of a round number of millimetres above the bottom edge of a viewport.
fn scale_bar(pen: &Pen, pane: &Pane<'_>, info: &SeriesInfo, inside: Rect) {
    // One column of the image spans its spacing in millimetres and this many screen points.
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    let to_screen = pane
        .shown
        .image_to_screen([info.cols, info.rows], size, aspect);
    let [[x, y, _], _, _] = to_screen.columns();
    let [between_cols, _, _] = info.spacing_mm;
    let mm_per_point = narrowed(between_cols / (x * x + y * y).sqrt());
    let room = pane.cell.width() * BAR_SHARE;
    let Some((length, label)) = overlay::scale_bar(mm_per_point, room) else {
        return;
    };
    let middle = inside.center_bottom() - vec2(0.0, 28.0);
    let half = vec2(length / 2.0, 0.0);
    pen.painter
        .line_segment([middle - half, middle + half], Stroke::new(2.0, INK));
    pen.write(
        middle - vec2(0.0, 4.0),
        Align2::CENTER_BOTTOM,
        &label,
        &pen.small,
    );
}
