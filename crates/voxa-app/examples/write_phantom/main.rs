//! Writes a small made-up study to a folder, for trying Voxa without a real scan.
//!
//! ```text
//! cargo run -p voxa-app --example write_phantom -- some/folder
//! cargo run -p voxa-app -- some/folder
//! ```
//!
//! The study holds two series of a ball that is brighter towards the patient's left, with a
//! small bright bead near the front, so the orientation of the picture can be told at a
//! glance. Beside it, in `labels`, are made-up labels of the ball's parts, as another
//! program might have written them for the finer series:
//!
//! ```text
//! cargo run -p voxa-app -- some/folder --labels some/folder/labels/labels.nii.gz
//! ```

use std::path::{Path, PathBuf};

use voxa_dicom::testkit::{SliceOptions, classic_slice};

mod labels;

/// What distinguishes one series of the phantom from another.
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

fn main() -> std::io::Result<()> {
    let folder = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("phantom"), PathBuf::from);
    for series in &SERIES {
        write(&folder, series)?;
    }
    // The labels are of the finer series, the one Voxa opens first.
    let [_, fine] = &SERIES;
    let beside = folder.join("labels");
    std::fs::create_dir_all(&beside)?;
    let volume = labels::volume(96.0 / f64::from(fine.side), fine.apart);
    std::fs::write(beside.join("labels.nii.gz"), volume)?;
    std::fs::write(beside.join("regions.json"), labels::TABLE)?;
    println!("A made-up study was written to {}", folder.display());
    Ok(())
}

fn write(folder: &Path, series: &Series) -> std::io::Result<()> {
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
                brightness(across, down, height)
            })
            .collect();
        let opts = SliceOptions {
            study_uid: "1.2.826.0.1.3680043.9.9999.7".to_owned(),
            study_description: "Made-up phantom".to_owned(),
            series_uid: series.uid.to_owned(),
            series_number: series.number,
            series_description: series.description.to_owned(),
            instance_number: i32::from(place) + 1,
            rows: series.side,
            cols: series.side,
            position: Some([-48.0, -48.0, f64::from(place) * series.apart]),
            pixel_spacing: Some([pixel, pixel]),
            slice_thickness: Some(series.apart),
            stored,
            ..SliceOptions::default()
        };
        std::fs::write(into.join(format!("I{place:04}.dcm")), classic_slice(&opts))?;
    }
    Ok(())
}

/// The stored value at a point of the phantom, each coordinate running from -1 to 1: across
/// towards the patient's left, down towards the back, height towards the head.
fn brightness(across: f64, down: f64, height: f64) -> u16 {
    let from_centre = (across * across + down * down + height * height).sqrt();
    let value = match labels::part(across, down, height) {
        labels::BEAD => 1000.0,
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
    };
    stored(value)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held between 0 and 4095 first"
)]
fn stored(value: f64) -> u16 {
    value.clamp(0.0, 4095.0) as u16
}
