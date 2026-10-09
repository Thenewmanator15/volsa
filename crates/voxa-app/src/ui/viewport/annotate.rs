//! What is written over a slice: corner text, a letter at each edge, and a scale bar.

use egui::{Align2, Rect, Ui};
use voxa_core::SeriesInfo;

use super::super::overlay;
use super::super::pen::{self, Pen};
use super::{Pane, narrowed, pixel_aspect};

/// Write over a slice: what it is in the corners, and, when the files say where the slice
/// lies, a letter at each edge and a scale bar.
pub(super) fn overlays(ui: &Ui, pane: &Pane<'_>) {
    let Some((info, study)) = pane.series() else {
        return;
    };
    let Some(viewport) = pane.state.viewports.get(pane.at) else {
        return;
    };
    let pen = Pen::over(ui, pane.cell);
    let inside = pane.cell.shrink(pen::INSET);
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
    // Each corner has half the width, less room for the letter in the middle of the edge.
    let room = inside.width() / 2.0 - pen::LETTER_ROOM;
    for (lines, (at, anchor)) in corners.iter().zip(anchors) {
        pen.write_lines(at, anchor, lines, room);
    }
    if let Some(placement) = placement {
        let radiological = pane.state.radiological;
        let labels = overlay::orientation_labels(placement, &viewport.view, radiological);
        pen::letters(&pen, inside, &labels);
        scale_bar(&pen, pane, info, inside);
    }
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
    let room = pane.cell.width() * pen::BAR_SHARE;
    if let Some((length, label)) = overlay::scale_bar(mm_per_point, room) {
        pen::bar(pen, inside, length, &label);
    }
}
