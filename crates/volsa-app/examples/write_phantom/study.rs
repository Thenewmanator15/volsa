//! The made-up study: two series of a ball, the other kinds of series an export holds, and
//! made-up labels of the ball's parts beside them.

use std::path::Path;

use volsa_dicom::testkit::{SliceOptions, classic_slice};

use crate::{labels, more};

/// What distinguishes one series of the ball from another.
struct Series {
    uid: &'static str,
    number: i32,
    description: &'static str,
    /// Rows and columns of each slice.
    side: u16,
    slices: u16,
    /// Millimetres between slices.
    apart: f64,
}

const SERIES: [Series; 2] = [
    Series {
        uid: "1.2.826.0.1.3680043.9.9999.7.1",
        number: 1,
        description: "ball, coarse",
        side: 96,
        slices: 24,
        apart: 4.0,
    },
    Series {
        uid: "1.2.826.0.1.3680043.9.9999.7.2",
        number: 2,
        description: "ball, fine",
        side: 160,
        slices: 40,
        apart: 2.4,
    },
];

/// Write the whole study into `folder`, each series in a folder of its own and the labels in
/// `labels`.
pub fn write(folder: &Path) -> std::io::Result<()> {
    for series in &SERIES {
        write_series(folder, series)?;
    }
    more::write(folder)?;
    // The labels are of the finer series, the one Volsa opens first.
    let [_, fine] = &SERIES;
    let beside = folder.join("labels");
    std::fs::create_dir_all(&beside)?;
    let volume = labels::volume(96.0 / f64::from(fine.side), fine.apart);
    std::fs::write(beside.join("labels.nii.gz"), volume)?;
    std::fs::write(beside.join("regions.json"), labels::TABLE)
}

/// The options every image of the study starts from: the study, and the series it is in.
pub fn image(series_uid: &str, series_number: i32, series_description: &str) -> SliceOptions {
    SliceOptions {
        study_uid: "1.2.826.0.1.3680043.9.9999.7".to_owned(),
        study_description: "Made-up phantom".to_owned(),
        series_uid: series_uid.to_owned(),
        series_number,
        series_description: series_description.to_owned(),
        ..SliceOptions::default()
    }
}

fn write_series(folder: &Path, series: &Series) -> std::io::Result<()> {
    let into = folder.join(format!("series-{}", series.number));
    std::fs::create_dir_all(&into)?;
    let side = f64::from(series.side);
    // The slices span the same 96 mm whatever their number, like the picture across them.
    let pixel = 96.0 / side;
    for place in 0..series.slices {
        let height = (f64::from(place) + 0.5) / f64::from(series.slices) * 2.0 - 1.0;
        let stored = (0..series.side)
            .flat_map(|row| (0..series.side).map(move |col| (row, col)))
            .map(|(row, col)| {
                let across = (f64::from(col) + 0.5) / side * 2.0 - 1.0;
                let down = (f64::from(row) + 0.5) / side * 2.0 - 1.0;
                stored(value(across, down, height))
            })
            .collect();
        let opts = SliceOptions {
            instance_number: i32::from(place) + 1,
            rows: series.side,
            cols: series.side,
            position: Some([-48.0, -48.0, f64::from(place) * series.apart]),
            pixel_spacing: Some([pixel, pixel]),
            slice_thickness: Some(series.apart),
            stored,
            ..image(series.uid, series.number, series.description)
        };
        std::fs::write(into.join(format!("I{place:04}.dcm")), classic_slice(&opts))?;
    }
    Ok(())
}

/// The value of the ball at a point in the patient, in millimetres: its middle is 48 mm
/// above the lowest slice, and it is 96 mm across.
pub fn value_at([x, y, z]: [f64; 3]) -> f64 {
    value(x / 48.0, y / 48.0, (z - 48.0) / 48.0)
}

/// The value at a point of the phantom, each coordinate running from -1 to 1: across towards
/// the patient's left, down towards the back, height towards the head.
fn value(across: f64, down: f64, height: f64) -> f64 {
    let from_centre = (across * across + down * down + height * height).sqrt();
    match labels::part(across, down, height) {
        labels::BEAD => BEAD,
        labels::NOTHING => 20.0,
        _ => {
            // Brighter towards the patient's left, with a faint ring every fifth of the
            // radius.
            let ring = if (from_centre * 5.0).fract() < 0.08 {
                60.0
            } else {
                0.0
            };
            400.0 + 250.0 * across + ring
        }
    }
}

/// The value of the bead, the brightest thing in the study.
pub const BEAD: f64 = 1000.0;

/// A value as the images store it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held between 0 and 4095 first"
)]
pub fn stored(value: f64) -> u16 {
    value.clamp(0.0, 4095.0) as u16
}
