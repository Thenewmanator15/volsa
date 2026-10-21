//! The kinds of series an export holds besides volumes, made up for the phantom: one series
//! holding two volumes of the ball at two echo times, three locating images in three planes,
//! and a picture in colour. Every image is 64 pixels of 1.5 mm a side, 96 mm across like the
//! ball.

use std::path::Path;

use voxa_dicom::testkit::{Colour, SliceOptions, classic_slice};

use crate::study::{self, BEAD, stored, value_at};

/// Pixels along a side of every image here.
const SIDE: u16 = 64;
/// Millimetres between neighbouring pixels.
const PIXEL: f64 = 1.5;

/// Write them into `folder`, each series in a folder of its own.
pub fn write(folder: &Path) -> std::io::Result<()> {
    echoes(&folder.join("series-3"))?;
    locating(&folder.join("series-4"))?;
    colour(&folder.join("series-5"))
}

/// Where the middle of pixel `index` along a side lies, in millimetres from the middle of
/// the image.
fn along(index: u16) -> f64 {
    PIXEL * (f64::from(index) + 0.5) - 48.0
}

/// The row and column of every pixel of an image, a row at a time.
fn pixels() -> impl Iterator<Item = (u16, u16)> {
    (0..SIDE).flat_map(|row| (0..SIDE).map(move |col| (row, col)))
}

/// The options of an image of this size in `series`, the first pixel's middle at `first`.
fn sized(series: SliceOptions, first: [f64; 3]) -> SliceOptions {
    SliceOptions {
        rows: SIDE,
        cols: SIDE,
        position: Some(first),
        pixel_spacing: Some([PIXEL, PIXEL]),
        ..series
    }
}

/// One series holding two volumes of the ball, as a scanner stores two echoes of one
/// examination together: the later echo has faded to half, all but the bead.
fn echoes(into: &Path) -> std::io::Result<()> {
    const SLICES: u16 = 16;
    let apart = 96.0 / f64::from(SLICES);
    std::fs::create_dir_all(into)?;
    for place in 0..SLICES {
        let height = apart * (f64::from(place) + 0.5);
        for (which, echo_ms, kept) in [(1, 20.0, 1.0), (2, 80.0, 0.5)] {
            let faded = |value: f64| if value >= BEAD { value } else { value * kept };
            let mut options = sized(
                study::image("1.2.826.0.1.3680043.9.9999.7.3", 3, "ball, two echoes"),
                [along(0), along(0), height],
            );
            options.instance_number = i32::from(place) * 2 + which;
            options.slice_thickness = Some(apart);
            options.scan.echo_ms = Some(echo_ms);
            options.stored = pixels()
                .map(|(row, col)| stored(faded(value_at([along(col), along(row), height]))))
                .collect();
            let name = format!("I{place:04}-{which}.dcm");
            std::fs::write(into.join(name), classic_slice(&options))?;
        }
    }
    Ok(())
}

/// Where a pixel of a locating image lies in the patient, from how far it is across the
/// image and down it.
type Placed = fn(f64, f64) -> [f64; 3];

/// Three locating images through the middle of the ball, one in each plane, as a scanner
/// takes them to plan an examination: seen from the feet, from the front, and from the side.
fn locating(into: &Path) -> std::io::Result<()> {
    let planes: [([f64; 6], Placed); 3] = [
        ([1.0, 0.0, 0.0, 0.0, 1.0, 0.0], |across, down| {
            [across, down, 48.0]
        }),
        ([1.0, 0.0, 0.0, 0.0, 0.0, -1.0], |across, down| {
            [across, 0.0, 48.0 - down]
        }),
        ([0.0, 1.0, 0.0, 0.0, 0.0, -1.0], |across, down| {
            [0.0, across, 48.0 - down]
        }),
    ];
    std::fs::create_dir_all(into)?;
    for (place, (orientation, placed)) in (1..).zip(planes) {
        let mut options = sized(
            study::image("1.2.826.0.1.3680043.9.9999.7.4", 4, "locating"),
            placed(along(0), along(0)),
        );
        options.instance_number = place;
        options.orientation = Some(orientation);
        options.slice_thickness = Some(8.0);
        options.stored = pixels()
            .map(|(row, col)| stored(value_at(placed(along(col), along(row)))))
            .collect();
        std::fs::write(
            into.join(format!("I{place:04}.dcm")),
            classic_slice(&options),
        )?;
    }
    Ok(())
}

/// A picture in colour of the ball's middle, as a program might save a map beside the scan:
/// what is dim in blue, what is bright in red, the bead in white and the air in black.
fn colour(into: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(into)?;
    let mut options = sized(
        study::image("1.2.826.0.1.3680043.9.9999.7.5", 5, "a picture in colour"),
        [along(0), along(0), 48.0],
    );
    options.instance_number = 1;
    // A picture saved by a program is no slice of a scan, and has no thickness.
    options.slice_thickness = None;
    let shades = pixels().map(|(row, col)| shade(value_at([along(col), along(row), 48.0])));
    options.colour = Some(Colour::of(shades.collect()));
    std::fs::write(into.join("I0001.dcm"), classic_slice(&options))
}

/// The colour a value of the ball is painted in.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn shade(value: f64) -> [u8; 3] {
    if value >= BEAD {
        return [255, 255, 255];
    }
    if value < 100.0 {
        return [0, 0, 0];
    }
    let warm = ((value - 150.0) / 500.0).clamp(0.0, 1.0);
    let part = |share: f64| (share * 255.0).round().clamp(0.0, 255.0) as u8;
    [part(warm), 40, part(1.0 - warm)]
}
