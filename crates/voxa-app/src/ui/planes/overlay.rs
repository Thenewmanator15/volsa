//! What is written and drawn over a plane view: pure functions.

use egui::{Color32, WidgetInfo, WidgetType};
use voxa_core::{Patient, Point, Volume};
use voxa_render::PlaneView;
use voxa_volume::{Plane, PlaneKind, sample};

use super::super::overlay::{EdgeLabels, letter, value_line};
use super::super::theme;
use crate::naming::plain_name;
use crate::profile::unit_of;
use crate::state::AppState;

/// How much room is left clear round the position where the lines of the cross would meet,
/// in points, so that what is at the position can be seen.
pub const CLEAR: f32 = 6.0;
/// A picture is said to be interpolated when its pixels run across layers more than this
/// many times as far apart as the finest voxels.
const COARSE: f64 = 1.5;

/// What a screen reader is told of a plane's panel, a picture with no words of its own: its
/// plane, then the plain name of the scan in the scene.
pub fn widget(state: &AppState, kind: PlaneKind) -> WidgetInfo {
    let scan = state.scene.series.as_ref().and_then(|id| state.series(id));
    let said = match scan {
        Some(listed) => format!("{} plane, {}", kind.name(), plain_name(&listed.info)),
        None => format!("{} plane, no scan", kind.name()),
    };
    WidgetInfo::labeled(WidgetType::Image, true, said)
}

/// The colour a plane is known by, in its own panel's caption, on the other two panels and
/// in the 3D scene.
pub fn plane_colour(kind: PlaneKind) -> Color32 {
    match kind {
        PlaneKind::Axial => theme::AXIAL,
        PlaneKind::Coronal => theme::CORONAL,
        PlaneKind::Sagittal => theme::SAGITTAL,
    }
}

/// Which of the other two planes is seen in a panel as the level line through the position,
/// and which as the upright one.
pub fn crossing(kind: PlaneKind) -> (PlaneKind, PlaneKind) {
    match kind {
        PlaneKind::Axial => (PlaneKind::Coronal, PlaneKind::Sagittal),
        PlaneKind::Coronal => (PlaneKind::Axial, PlaneKind::Sagittal),
        PlaneKind::Sagittal => (PlaneKind::Axial, PlaneKind::Coronal),
    }
}

/// What a panel says of its plane: its name, then on which side of the scanner's origin it
/// lies and how far, as "Axial · S 12.5 mm".
pub fn plane_caption(kind: PlaneKind, plane: &Plane, position: Point<Patient>) -> String {
    let [x, y, z] = plane.normal();
    let along = x * position.x + y * position.y + z * position.z;
    let side = if along < 0.0 { -1.0 } else { 1.0 };
    let towards = letter([x * side, y * side, z * side]);
    format!("{} · {towards} {:.1} mm", kind.name(), along.abs())
}

/// The letter at each edge of a plane's panel.
pub fn plane_letters(plane: &Plane) -> EdgeLabels {
    let towards = |direction: [f64; 3]| letter(direction).to_owned();
    let away_from = |direction: [f64; 3]| letter(direction.map(|part| -part)).to_owned();
    EdgeLabels {
        left: away_from(plane.across),
        right: towards(plane.across),
        top: away_from(plane.down),
        bottom: towards(plane.down),
    }
}

/// What a panel says of the value under the pointer: `Value -12 HU`, or in the unit the scan's
/// files give, or with none. It is worked out from the scene's volume `scan` at `at`, a point
/// of the patient. `None` when there is no pointer over the panel, no volume yet, the point is
/// outside the volume, or the scene shows no scan.
pub fn value_under(
    state: &AppState,
    scan: Option<&Volume>,
    at: Option<Point<Patient>>,
) -> Option<String> {
    let (scan, at) = scan.zip(at)?;
    let info = &state.series(state.scene.series.as_ref()?)?.info;
    value_line(sample(scan, at)?, &unit_of(info))
}

/// The lines of the cross through the position in a panel, each from one point of the panel
/// to another.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cross {
    /// The lines along the row of the position: to its left and to its right.
    pub level: Vec<[[f32; 2]; 2]>,
    /// The lines along its column: above and below.
    pub upright: Vec<[[f32; 2]; 2]>,
}

/// The room kept clear of the cross at the edges of a panel, in points, for what is written
/// there: the letters at the sides and the top, and the scale bar and a note at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Room {
    /// At the left and at the right.
    pub sides: f32,
    /// At the top.
    pub top: f32,
    /// At the bottom.
    pub bottom: f32,
}

/// The cross through the position in a panel of `viewport` (width, height) points: lines
/// from where the room at the edges ends to [`CLEAR`] short of the position. A position off
/// one side still has its line across the panel; one whose line would run through the room
/// at an edge has no such line.
pub fn crosshair(
    view: &PlaneView,
    viewport: [f32; 2],
    position: Point<Patient>,
    room: Room,
) -> Cross {
    let ([x, y], _) = view.patient_to_screen(viewport, position);
    let [wide, high] = viewport;
    let (left, right) = (room.sides, wide - room.sides);
    let (top, bottom) = (room.top, high - room.bottom);
    // The stretches of a line from `from` to `to` left clear of a point `at` along it.
    let either_side = |at: f32, from: f32, to: f32| {
        let before = (from, (at - CLEAR).min(to));
        let after = ((at + CLEAR).max(from), to);
        [before, after].into_iter().filter(|(from, to)| from < to)
    };
    let within = |at: f32, from: f32, to: f32| (from..=to).contains(&at);
    let level = either_side(x, left, right).map(|(from, to)| [[from, y], [to, y]]);
    let upright = either_side(y, top, bottom).map(|(from, to)| [[x, from], [x, to]]);
    Cross {
        level: level.filter(|_| within(y, top, bottom)).collect(),
        upright: upright.filter(|_| within(x, left, right)).collect(),
    }
}

/// What a panel says when its picture is drawn from a curve through the measured values and
/// nothing else needs saying of it.
pub const SMOOTHED: &str = "Smoothed: a curve, not more detail";

/// What a panel says of how its picture was worked out: `interpolated`, the note that it
/// runs across coarse layers, if it does, and, when it is drawn `smooth`, that too. It is
/// kept short, as it is written over the picture; the Smooth switch says it in full.
pub fn panel_note(interpolated: Option<String>, smooth: bool) -> Option<String> {
    match (interpolated, smooth) {
        (Some(note), true) => Some(note.replacen("Interpolated:", "Interpolated and smoothed:", 1)),
        (None, true) => Some(SMOOTHED.to_owned()),
        (note, false) => note,
    }
}

/// What a panel says when its picture runs across layers of voxels much farther apart than
/// the finest: what is between them is worked out, not measured. It is short, as it is
/// written over the picture; the 3D scene beside it says the same in full. `pitch_across`
/// and `pitch_down` are how far apart the layers are along the picture's sides.
pub fn interpolated_note(
    spacing_mm: [f64; 3],
    pitch_across: f64,
    pitch_down: f64,
) -> Option<String> {
    let finest = spacing_mm.into_iter().fold(f64::INFINITY, f64::min);
    let coarsest = pitch_across.max(pitch_down);
    (coarsest > COARSE * finest).then(|| format!("Interpolated: layers {coarsest:.1} mm apart"))
}
