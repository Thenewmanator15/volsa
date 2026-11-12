//! Large made-up studies of the made-up body, for measuring Volsa on scans as large as CT and
//! whole-body MRI: a CT series of thin slices in Hounsfield units, and a whole-body MR study
//! taken in overlapping stations. Each slice is worked out and written on a thread of its own.

use std::path::Path;

use rayon::prelude::*;
use volsa_dicom::testkit::{SliceOptions, classic_slice};

use crate::body::{self, HEIGHT, Tissue};
use crate::study;

/// The CT's slices are 512 pixels a side, 0.8 mm apart: 410 mm across.
const CT_SIDE: u16 = 512;
const CT_PIXEL: f64 = 0.8;

/// A CT value in Hounsfield units: what each tissue reads, before the scan's noise.
fn hounsfield(tissue: Tissue) -> f64 {
    match tissue {
        Tissue::Air => -1000.0,
        Tissue::Fat => -100.0,
        Tissue::Soft => 45.0,
        Tissue::Lung => -850.0,
        Tissue::Bone => 1100.0,
        Tissue::Marrow => 250.0,
        Tissue::Brain | Tissue::Kidney => 35.0,
        Tissue::Blood => 50.0,
        Tissue::Liver => 60.0,
        Tissue::Table => 150.0,
    }
}

/// An MR value, as a fat-bright scan of the whole body reads each tissue.
fn magnetic(tissue: Tissue) -> f64 {
    match tissue {
        Tissue::Air | Tissue::Table => 5.0,
        Tissue::Fat => 950.0,
        Tissue::Soft => 280.0,
        Tissue::Lung => 25.0,
        Tissue::Bone => 40.0,
        Tissue::Marrow => 750.0,
        Tissue::Brain => 520.0,
        Tissue::Blood => 180.0,
        Tissue::Liver => 360.0,
        Tissue::Kidney => 420.0,
    }
}

/// Write a CT of the whole made-up body as `slices` slices into `folder`, stored as the
/// scanner stores CT: whole numbers 1024 above the Hounsfield value, with a rescale
/// intercept of -1024. The slices are as far apart as spreads them over the whole body.
pub fn ct(folder: &Path, slices: u32) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let apart = HEIGHT / f64::from(slices);
    let first = -CT_PIXEL * (f64::from(CT_SIDE) - 1.0) / 2.0;
    (0..slices).into_par_iter().try_for_each(|place| {
        let z = (f64::from(place) + 0.5) * apart;
        let stored = grid(CT_SIDE, CT_SIDE, CT_PIXEL, z)
            .map(|(col, row, point)| {
                let value = hounsfield(body::at(point)) + 12.0 * body::noise(col, row, place);
                stored_ct(value)
            })
            .collect();
        let opts = SliceOptions {
            modality: "CT".to_owned(),
            instance_number: i32::try_from(place).unwrap_or(i32::MAX) + 1,
            rows: CT_SIDE,
            cols: CT_SIDE,
            position: Some([first, first, z]),
            pixel_spacing: Some([CT_PIXEL, CT_PIXEL]),
            slice_thickness: Some(apart),
            stored,
            intercept: -1024.0,
            ..study::image(CT_UID, 1, "made-up CT, whole body")
        };
        std::fs::write(
            folder.join(format!("I{place:05}.dcm")),
            classic_slice(&opts),
        )
    })
}

const CT_UID: &str = "1.2.826.0.1.3680043.9.9999.8.1";

/// The whole-body MR: slices 512 pixels across and 384 from front to back, 0.9375 mm apart,
/// 1.5 mm between slices; each station 192 slices, overlapping the next by 32.
const MR_COLS: u16 = 512;
const MR_ROWS: u16 = 384;
const MR_PIXEL: f64 = 0.9375;
const MR_APART: f64 = 1.5;
const STATION: u32 = 192;
const OVERLAP: u32 = 32;

/// Write a whole-body MR of the made-up body into `folder`: `stations` series that each cover
/// a part of the body and overlap the next, as a scanner takes them, with the brightness of
/// each falling off towards its ends as a coil's does; and one series of them joined, as a
/// scanner also offers, with no fall-off. Each series is in a folder of its own.
pub fn whole_body(folder: &Path, stations: u32) -> std::io::Result<()> {
    let step = STATION - OVERLAP;
    for station in 0..stations {
        let first = station * step;
        let series = MrSeries {
            number: i32::try_from(station).unwrap_or(0) + 1,
            description: format!("made-up body, station {} of {stations}", station + 1),
            first,
            slices: STATION,
            falls_off: true,
        };
        series.write(&folder.join(format!("station-{}", station + 1)))?;
    }
    let joined = MrSeries {
        number: 100,
        description: "made-up body, stations joined".to_owned(),
        first: 0,
        slices: stations * step + OVERLAP,
        falls_off: false,
    };
    joined.write(&folder.join("joined"))
}

struct MrSeries {
    number: i32,
    description: String,
    /// The first slice's place among all the body's slices, `MR_APART` apart.
    first: u32,
    slices: u32,
    /// Whether brightness falls off towards the series' ends.
    falls_off: bool,
}

impl MrSeries {
    fn write(&self, folder: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(folder)?;
        let uid = format!("1.2.826.0.1.3680043.9.9999.8.2.{}", self.number);
        let left = -MR_PIXEL * (f64::from(MR_COLS) - 1.0) / 2.0;
        let front = -MR_PIXEL * (f64::from(MR_ROWS) - 1.0) / 2.0;
        (0..self.slices).into_par_iter().try_for_each(|place| {
            let overall = self.first + place;
            let z = (f64::from(overall) + 0.5) * MR_APART;
            let from_middle = (f64::from(place) + 0.5) / f64::from(self.slices) * 2.0 - 1.0;
            let coil = if self.falls_off {
                1.0 - 0.3 * from_middle * from_middle
            } else {
                1.0
            };
            let stored = mr_grid(z)
                .map(|(col, row, point)| {
                    let clean = magnetic(body::at(point)) * coil;
                    study::stored(clean + 15.0 * body::noise(col, row, overall).abs())
                })
                .collect();
            let opts = SliceOptions {
                instance_number: i32::try_from(place).unwrap_or(i32::MAX) + 1,
                rows: MR_ROWS,
                cols: MR_COLS,
                position: Some([left, front, z]),
                pixel_spacing: Some([MR_PIXEL, MR_PIXEL]),
                slice_thickness: Some(MR_APART),
                stored,
                ..study::image(&uid, self.number, &self.description)
            };
            std::fs::write(
                folder.join(format!("I{place:05}.dcm")),
                classic_slice(&opts),
            )
        })
    }
}

/// Every pixel of a slice `cols` by `rows`, `pixel` mm apart and centred on the body, at
/// height `z`: its column, its row and where it lies in the patient.
fn grid(cols: u16, rows: u16, pixel: f64, z: f64) -> impl Iterator<Item = (u32, u32, [f64; 3])> {
    let middle =
        move |count: u16, at: u16| (f64::from(at) - (f64::from(count) - 1.0) / 2.0) * pixel;
    (0..rows).flat_map(move |row| {
        (0..cols).map(move |col| {
            let point = [middle(cols, col), middle(rows, row), z];
            (u32::from(col), u32::from(row), point)
        })
    })
}

fn mr_grid(z: f64) -> impl Iterator<Item = (u32, u32, [f64; 3])> {
    grid(MR_COLS, MR_ROWS, MR_PIXEL, z)
}

/// A Hounsfield value as the CT stores it: 1024 above, between 0 and 4095.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held between 0 and 4095 first"
)]
fn stored_ct(value: f64) -> u16 {
    (value + 1024.0).round().clamp(0.0, 4095.0) as u16
}

/// Put every file under `folder` into a zip at `out`, compressed unless `stored`, each under
/// its name relative to the folder.
pub fn zip(folder: &Path, out: &Path, stored: bool) -> std::io::Result<()> {
    let method = if stored {
        zip::CompressionMethod::Stored
    } else {
        zip::CompressionMethod::Deflated
    };
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .large_file(false);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(out)?);
    let mut files = Vec::new();
    gather(folder, folder, &mut files)?;
    files.sort();
    for name in files {
        writer.start_file(name.clone(), options)?;
        let bytes = std::fs::read(folder.join(&name))?;
        std::io::Write::write_all(&mut writer, &bytes)?;
    }
    writer.finish()?;
    Ok(())
}

/// The names, relative to `root` and with `/` between folders, of every file under `dir`.
fn gather(root: &Path, dir: &Path, into: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            gather(root, &path, into)?;
        } else if let Ok(name) = path.strip_prefix(root) {
            into.push(name.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}
