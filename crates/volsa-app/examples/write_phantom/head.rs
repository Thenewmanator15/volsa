//! The converter's made-up head, written as a study: a brain's shape that is nobody's, to label.
//!
//! The model folder's `check` folder holds the head (`volume.f32`, with where it lies in
//! `case.json`). Here each of its slices becomes a DICOM file, so Voxa opens it as it opens
//! any export. The values are rounded to the nearest sixty-fourth and stored as 16-bit
//! numbers, and the rescale slope is one sixty-fourth, so what Voxa reads back is exactly what
//! was stored.
//!
//! With `whole` the head is put in the middle of a grid of air as large as a common 1 mm scan
//! of a whole head, 176 x 256 x 256 voxels, which pads to 192 x 256 x 256 for the networks.

use std::io::{Error, ErrorKind, Result};
use std::path::Path;

use serde_json::Value;
use voxa_dicom::testkit::{SliceOptions, classic_slice};

/// Stored steps to one unit of the head's values.
const STEPS: f64 = 64.0;
/// The grid of a whole-head scan, along the head's three axes, in voxels.
const WHOLE: [usize; 3] = [176, 256, 256];
/// What the study is called, and its series.
const STUDY: &str = "1.2.826.0.1.3680043.9.9999.8";
const SERIES: &str = "1.2.826.0.1.3680043.9.9999.8.1";

/// The head as the `check` folder has it.
struct Head {
    /// Voxels along the columns, the rows and the slices.
    dims: [usize; 3],
    /// The values, columns fastest.
    values: Vec<f32>,
    /// The patient's millimetres between neighbouring voxels along each of the three axes,
    /// towards the patient's left, back and head.
    steps: [[f64; 3]; 3],
    /// Where the first voxel lies.
    origin: [f64; 3],
}

fn wrong(why: &str) -> Error {
    Error::new(ErrorKind::InvalidData, why.to_owned())
}

/// A number of the `check` folder's `case.json`, or why not.
fn number(case: &Value, path: &[&str], place: [usize; 2]) -> Result<f64> {
    let mut at = case;
    for name in path {
        at = at
            .get(*name)
            .ok_or_else(|| wrong("case.json lacks the volume"))?;
    }
    at.get(place[0])
        .and_then(|row| row.get(place[1]))
        .and_then(Value::as_f64)
        .ok_or_else(|| wrong("case.json does not place the volume"))
}

/// Read the head of a `check` folder.
fn read(check: &Path) -> Result<Head> {
    let case: Value = serde_json::from_slice(&std::fs::read(check.join("case.json"))?)?;
    let side = |place: usize| {
        let side = case.get("volume")?.get("dims")?.get(place)?.as_u64()?;
        usize::try_from(side).ok()
    };
    let dims = [0, 1, 2].map(|place| side(place).unwrap_or(0));
    let bytes = std::fs::read(check.join("volume.f32"))?;
    let values: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&four| f32::from_le_bytes(four))
        .collect();
    if dims.contains(&0) || values.len() != dims.iter().product::<usize>() {
        return Err(wrong("volume.f32 is not the size case.json gives"));
    }
    // case.json places the voxels as NIfTI does, to the patient's right and front; the
    // study is placed as DICOM does, to the patient's left and back.
    let from = |row: usize, column: usize| number(&case, &["volume", "affine"], [row, column]);
    let mut steps = [[0.0; 3]; 3];
    for (axis, step) in steps.iter_mut().enumerate() {
        *step = [-from(0, axis)?, -from(1, axis)?, from(2, axis)?];
    }
    let origin = [-from(0, 3)?, -from(1, 3)?, from(2, 3)?];
    Ok(Head {
        dims,
        values,
        steps,
        origin,
    })
}

fn length(step: [f64; 3]) -> f64 {
    step.iter().map(|part| part * part).sum::<f64>().sqrt()
}

/// A step as a direction.
fn towards(step: [f64; 3]) -> [f64; 3] {
    step.map(|part| part / length(step))
}

/// A value as the images store it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held between 0 and 65535 first"
)]
fn stored(value: f32) -> u16 {
    (f64::from(value) * STEPS).round().clamp(0.0, 65535.0) as u16
}

/// Where the head lies in a grid of `grid` voxels: how many voxels of air come before it
/// along each axis. A grid smaller than the head along an axis leaves no air there.
fn air_before(dims: [usize; 3], grid: [usize; 3]) -> [usize; 3] {
    [0, 1, 2].map(|axis| {
        let (inside, whole) = (dims.get(axis).copied(), grid.get(axis).copied());
        whole
            .zip(inside)
            .map_or(0, |(whole, inside)| whole.saturating_sub(inside) / 2)
    })
}

/// The values of slice `slice` of a grid, row by row; air where the head is not.
fn slice_of(head: &Head, grid: [usize; 3], before: [usize; 3], slice: usize) -> Vec<u16> {
    let [cols, rows, _] = head.dims;
    let mut values = vec![0; grid[0] * grid[1]];
    let inside = slice.checked_sub(before[2]);
    let Some(inside) = inside.filter(|inside| *inside < head.dims[2]) else {
        return values;
    };
    for row in 0..rows {
        for col in 0..cols {
            let from = col + cols * (row + rows * inside);
            let to = (col + before[0]) + grid[0] * (row + before[1]);
            if let (Some(value), Some(into)) = (head.values.get(from), values.get_mut(to)) {
                *into = stored(*value);
            }
        }
    }
    values
}

/// The head in a grid of air, or on its own grid: how the study's slices are laid out.
struct Grid<'a> {
    head: &'a Head,
    /// Voxels along the columns, the rows and the slices of the study.
    sides: [usize; 3],
    /// Voxels of air before the head along each of them.
    before: [usize; 3],
}

impl Grid<'_> {
    /// Where the first voxel of slice `slice` lies: the head's own first voxel, less the air
    /// before it along the columns and the rows, and the slices from there.
    fn first(&self, slice: usize) -> [f64; 3] {
        let [columns, rows, slices] = self.before.map(|count| count as f64);
        let counts = [-columns, -rows, slice as f64 - slices];
        [0, 1, 2].map(|part| {
            let moved = self.head.steps.iter().zip(counts);
            self.head.origin[part] + moved.map(|(step, count)| step[part] * count).sum::<f64>()
        })
    }

    /// What the file of slice `slice` says, but for the description of the series.
    fn options(&self, slice: usize, description: &str) -> Result<SliceOptions> {
        let [along_columns, along_rows, between] = self.head.steps;
        let (row_direction, column_direction) = (towards(along_columns), towards(along_rows));
        let side = |axis: usize| u16::try_from(self.sides[axis]).map_err(|_| wrong("too large"));
        Ok(SliceOptions {
            study_uid: STUDY.to_owned(),
            study_date: Some("20990102".to_owned()),
            study_description: "Made-up head".to_owned(),
            series_uid: SERIES.to_owned(),
            series_number: 1,
            series_description: description.to_owned(),
            instance_number: i32::try_from(slice).map_err(|_| wrong("too many slices"))? + 1,
            rows: side(1)?,
            cols: side(0)?,
            position: Some(self.first(slice)),
            orientation: Some([
                row_direction[0],
                row_direction[1],
                row_direction[2],
                column_direction[0],
                column_direction[1],
                column_direction[2],
            ]),
            pixel_spacing: Some([length(along_rows), length(along_columns)]),
            slice_thickness: Some(length(between)),
            stored: slice_of(self.head, self.sides, self.before, slice),
            slope: 1.0 / STEPS,
            ..SliceOptions::default()
        })
    }
}

/// Write the head of `check` into `out/series-1`, one file for each slice. With `whole`, in
/// a grid of air as large as a whole-head scan.
pub fn write(check: &Path, out: &Path, whole: bool) -> Result<()> {
    let head = read(check)?;
    let sides = if whole {
        [0, 1, 2].map(|axis| WHOLE[axis].max(head.dims[axis]))
    } else {
        head.dims
    };
    let before = air_before(head.dims, sides);
    let grid = Grid {
        head: &head,
        sides,
        before,
    };
    let description = if whole {
        "head in a whole-head grid"
    } else {
        "head"
    };
    let into = out.join("series-1");
    std::fs::create_dir_all(&into)?;
    for slice in 0..sides[2] {
        let file = classic_slice(&grid.options(slice, description)?);
        std::fs::write(into.join(format!("I{slice:04}.dcm")), file)?;
    }
    Ok(())
}
