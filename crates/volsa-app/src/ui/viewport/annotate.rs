//! What is written over a slice: corner text, a letter at each edge, and a scale bar.

use egui::{Align2, Rect, Stroke, Ui, vec2};
use volsa_core::{Point, SeriesInfo, StudyInfo};

use super::super::pen::{self, Pen};
use super::super::{overlay, theme};
use super::{Pane, narrowed, pixel_aspect};
use crate::planes::position_in_viewport;
use crate::profile::unit_of;

/// How far from the position the lines that mark it begin, and how long they are, in points.
const CLEAR: f32 = 4.0;
const ARM: f32 = 8.0;

/// Write over a slice: what it is in the corners, with the `value` under the pointer if there
/// is one, and, when the files say where the slice lies, a letter at each edge and a scale bar.
/// Says where the corners' writing and the scale bar stand, for what is measured on the slice
/// to be written clear of them. The corner with the value under the pointer is kept for it
/// whether or not the pointer is over the slice, so that what is measured does not jump aside
/// as the pointer comes and goes.
pub(super) fn overlays(ui: &Ui, pane: &Pane<'_>, value: Option<f32>) -> Vec<Rect> {
    let Some((info, study)) = pane.series() else {
        return Vec::new();
    };
    let Some(viewport) = pane.state.viewports.get(pane.at) else {
        return Vec::new();
    };
    let pen = Pen::over(ui, pane.cell);
    let inside = pane.cell.shrink(pen::INSET);
    let placement = info.voxel_to_patient.as_ref();
    let corners = corners(pane, info, study, value);
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
    let kept = self::corners(pane, info, study, Some(value.unwrap_or_default()));
    let mut taken: Vec<Rect> = kept
        .iter()
        .zip(anchors)
        .map(|(lines, (at, anchor))| pen.lines_rect(at, anchor, lines, room))
        .collect();
    if let Some(placement) = placement {
        let radiological = pane.state.radiological;
        let labels = overlay::orientation_labels(placement, &viewport.view, radiological);
        taken.extend(pen::letters(&pen, inside, &labels));
        taken.extend(scale_bar(&pen, pane, info, inside));
    }
    mark_position(&pen, pane, info);
    taken
}

/// The lines written in the four corners of a viewport. The last line of the bottom left corner
/// is the `value` under the pointer, in the unit the scan's files give it, when there is one.
pub(super) fn corners(
    pane: &Pane<'_>,
    info: &SeriesInfo,
    study: &StudyInfo,
    value: Option<f32>,
) -> [Vec<String>; 4] {
    let Some(viewport) = pane.state.viewports.get(pane.at) else {
        return Default::default();
    };
    let placement = info.voxel_to_patient.as_ref();
    let position = placement.map(|placement| overlay::slice_position(placement, viewport.slice));
    let show_patient = pane.state.show_patient;
    let wl = viewport.wl;
    let mut corners = overlay::corner_text(study, info, viewport.slice, position, wl, show_patient);
    let said = value.and_then(|value| overlay::value_line(value, &unit_of(info)));
    corners[2].extend(said.filter(|_| !info.colour));
    corners
}

/// The four short lines that mark the planes' position about a point of a viewport, each
/// along the point's row or column and stopping short of it, so that what is there shows.
pub(super) fn position_mark([x, y]: [f32; 2]) -> [[[f32; 2]; 2]; 4] {
    let (near, far) = (CLEAR, CLEAR + ARM);
    [
        [[x - far, y], [x - near, y]],
        [[x + near, y], [x + far, y]],
        [[x, y - far], [x, y - near]],
        [[x, y + near], [x, y + far]],
    ]
}

/// Mark the planes' position on a linked viewport whose slice it lies in.
fn mark_position(pen: &Pen, pane: &Pane<'_>, info: &SeriesInfo) {
    let Some([column, row]) = position_in_viewport(pane.state, pane.at) else {
        return;
    };
    let size = [pane.cell.width(), pane.cell.height()];
    let aspect = pixel_aspect(pane.state, pane.at);
    let to_screen = pane
        .shown
        .image_to_screen([info.cols, info.rows], size, aspect);
    let at = to_screen.apply(Point::new(column, row, 0.0));
    let stroke = Stroke::new(1.5, theme::ACCENT);
    for [from, to] in position_mark([narrowed(at.x), narrowed(at.y)]) {
        let placed = |[x, y]: [f32; 2]| pane.cell.min + vec2(x, y);
        pen.painter.line_segment([placed(from), placed(to)], stroke);
    }
}

/// Draw a bar of a round number of millimetres above the bottom edge of a viewport, if there
/// is room for one, and say where it stands.
fn scale_bar(pen: &Pen, pane: &Pane<'_>, info: &SeriesInfo, inside: Rect) -> Option<Rect> {
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
    let (length, label) = overlay::scale_bar(mm_per_point, room)?;
    Some(pen::bar(pen, inside, length, &label))
}
