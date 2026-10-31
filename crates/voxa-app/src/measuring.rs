//! Measuring on the slices of a scan as well as on its planes and in 3D. The files alone say
//! in whose millimetres a slice is measured: in the patient's, where they say where its slices
//! lie; within the picture, from the Pixel Spacing, where that is all they give; and not at all
//! where they give no Pixel Spacing, a detector's spacing not being the body's. Here too is
//! what a region drawn on a slice holds, from the slice's own values, and, when nothing can be
//! measured, why not.

use serde::{Deserialize, Serialize};
use voxa_core::{Affine, Patient, Point, SeriesInfo, SeriesProblem, Voxel};
use voxa_volume::{Plane, pitch_along};

use crate::planes::grid;
use crate::state::AppState;

mod region;

pub use region::{SliceValues, frame_ellipse_stats};

/// What the tools say of a scan that cannot be measured in millimetres: its files give no
/// Pixel Spacing, or one that is not a length.
pub const NO_SCALE: &str = "Measuring needs a scan with a scale in its files.";
/// What they say of a series whose slices are of several sizes or lie in several planes: the
/// spacing read from the first of them need not hold for the rest.
pub const NO_ONE_SCALE: &str = "Measuring needs slices of one size and plane, and these differ.";
/// What they say while the slice to measure on has still to be shown.
pub const NOT_SHOWN: &str = "Measuring starts once the slice is shown.";
/// What the region tool says of a picture in colour, whose values only say how bright each
/// pixel is: lengths, angles and markers can be measured on it, values in a region cannot.
pub const IN_COLOUR: &str = "A picture in colour has no values to measure in a region.";
/// What the list of measurements says of a scan shown as a plain stack whose files say where
/// its slices lie: between its first slice and its last, only roughly.
pub const PLAIN_STACK: &str = "This scan is shown as a plain stack: what is measured within one \
                               slice holds, but where its slices lie is only roughly known.";

/// Whose millimetres a measurement is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Scale {
    /// The patient's, as DICOM places them: the files say where the slices lie.
    #[default]
    #[serde(rename = "patient")]
    Patient,
    /// The picture's own, from the file's Pixel Spacing alone: the files do not say where the
    /// slices lie.
    #[serde(rename = "pixel spacing")]
    PixelSpacing,
}

impl Scale {
    /// The scale as the saved measurements name it: `patient` or `pixel spacing`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Patient => "patient",
            Self::PixelSpacing => "pixel spacing",
        }
    }

    /// What is said after a measurement's value at this scale, in the list and on the
    /// picture: nothing in the patient's millimetres, and where they come from otherwise.
    pub fn said_after(self) -> &'static str {
        match self {
            Self::Patient => "",
            Self::PixelSpacing => " (from the file's pixel spacing)",
        }
    }
}

/// Where the pixels of a series' slices lie, and in whose millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlicePlacement {
    /// Where the voxel at (column, row, slice) lies: in the patient; or, from the pixel spacing
    /// alone, in the picture, `x` being the column times the distance between columns, `y`
    /// the row times the distance between rows, and `z` the slice.
    pub placement: Affine<Voxel, Patient>,
    /// Whose millimetres those are.
    pub scale: Scale,
}

/// Where the slices of a series lie, if they can be measured on: in the patient when the files
/// say where they lie and give their Pixel Spacing; within the picture when they give the
/// spacing alone; and `None` without a spacing, with one that is not a length above zero, or
/// for slices of several sizes or planes, which no one spacing can be trusted to hold for.
pub fn placement_of(info: &SeriesInfo) -> Option<SlicePlacement> {
    let [between_cols, between_rows, _] = info.spacing_mm;
    let a_length = |mm: f64| mm.is_finite() && mm > 0.0;
    let spaced = info.spacing_given && a_length(between_cols) && a_length(between_rows);
    if !spaced || !one_grid(info) {
        return None;
    }
    let at = match info.voxel_to_patient {
        Some(placement) => SlicePlacement {
            placement,
            scale: Scale::Patient,
        },
        None => SlicePlacement {
            placement: Affine::from_columns(
                [between_cols, 0.0, 0.0],
                [0.0, between_rows, 0.0],
                [0.0, 0.0, 1.0],
                [0.0; 3],
            ),
            scale: Scale::PixelSpacing,
        },
    };
    // A placement that cannot be undone puts the whole slice at one point.
    at.placement.inverse().map(|_| at)
}

/// Whether the slices of a series share one grid of pixels, neither of several sizes nor in
/// several planes, so that the spacing read from the first holds for them all.
fn one_grid(info: &SeriesInfo) -> bool {
    !info.problems.iter().any(|problem| {
        matches!(
            problem,
            SeriesProblem::MixedSizes | SeriesProblem::MixedOrientations
        )
    })
}

/// The plane slice `slice` lies in: its middle the middle of a picture of `size` (columns,
/// rows), its sides along the slice's rows and columns.
pub fn slice_plane(at: &SlicePlacement, slice: usize, size: [u32; 2]) -> Plane {
    let [cols, rows] = size.map(f64::from);
    let middle = Point::new((cols - 1.0) / 2.0, (rows - 1.0) / 2.0, slice as f64);
    let [along_rows, along_columns, _] = at.placement.columns();
    let across = unit(along_rows).unwrap_or([1.0, 0.0, 0.0]);
    // The sides of a picture are square to each other, even where a grid's are not quite.
    let lean = dot(along_columns, across);
    let square = [0, 1, 2].map(|part| part_of(along_columns, part) - lean * part_of(across, part));
    Plane {
        centre: at.placement.apply(middle),
        across,
        down: unit(square).unwrap_or([0.0, 1.0, 0.0]),
    }
}

/// How far from a slice's plane a point may be and still be on that slice: half the distance
/// between slices along the plane's normal, and a hair more for what lies exactly between two.
pub fn half_slice(at: &SlicePlacement, plane: &Plane) -> f64 {
    let pitch = pitch_along(&at.placement, plane.normal());
    pitch.map_or(0.5, |pitch| pitch / 2.0) + 1.0e-6
}

/// Why nothing can be measured now, or `None` when something can: on the planes and in 3D
/// when the scene's volume is ready, and otherwise on the slices the active viewport shows,
/// once one of them is shown, when [`placement_of`] says they can be measured on.
pub fn why_not_measurable(state: &AppState) -> Option<&'static str> {
    if grid(state).is_some() {
        return None;
    }
    let Some(info) = state.shown_info(state.active) else {
        return Some(NO_SCALE);
    };
    if placement_of(info).is_none() {
        let several = info.spacing_given && !one_grid(info);
        return Some(if several { NO_ONE_SCALE } else { NO_SCALE });
    }
    // A viewport's window is chosen once a slice it shows is decoded: until then there is
    // nothing on the screen to measure on.
    let shown = state.viewports.get(state.active);
    shown
        .is_none_or(|viewport| viewport.wl_pending)
        .then_some(NOT_SHOWN)
}

/// Why no region can be measured now, or `None` when one can: as [`why_not_measurable`], and
/// not on a picture in colour, which has no values to measure.
pub fn why_no_region(state: &AppState) -> Option<&'static str> {
    let colour = state
        .shown_info(state.active)
        .is_some_and(|info| info.colour);
    why_not_measurable(state).or(colour.then_some(IN_COLOUR))
}

/// The scale what is measured now is made at: the patient's while the scene's volume is ready,
/// and otherwise that of the slices the active viewport shows. `None` while nothing can be
/// measured.
pub fn scale_now(state: &AppState) -> Option<Scale> {
    if grid(state).is_some() {
        return Some(Scale::Patient);
    }
    if why_not_measurable(state).is_some() {
        return None;
    }
    placement_of(state.shown_info(state.active)?).map(|at| at.scale)
}

/// Where the slices of the scene's series lie, which is the series the measurements are of.
pub fn scene_placement(state: &AppState) -> Option<SlicePlacement> {
    let series = state.scene.series.as_ref()?;
    placement_of(&state.series(series)?.info)
}

/// What the list of measurements says of the scene's series being a plain stack, when it is
/// one that can be measured on in the patient's millimetres: its slices lie only roughly where
/// they are placed, the gaps between them being uneven or some sharing one position.
pub fn plain_stack_note(state: &AppState) -> Option<&'static str> {
    let series = state.scene.series.as_ref()?;
    let info = &state.series(series)?.info;
    let rough = info.problems.iter().any(|problem| {
        matches!(
            problem,
            SeriesProblem::UnevenSpacing | SeriesProblem::RepeatedPositions
        )
    });
    let placed = placement_of(info).is_some_and(|at| at.scale == Scale::Patient);
    (rough && placed).then_some(PLAIN_STACK)
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    (length > 0.0 && length.is_finite()).then(|| v.map(|part| part / length))
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

fn part_of([x, y, z]: [f64; 3], part: usize) -> f64 {
    match part {
        0 => x,
        1 => y,
        _ => z,
    }
}

#[cfg(test)]
mod tests;
