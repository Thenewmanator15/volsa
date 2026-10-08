//! Small synthetic exports shared by this crate's tests.

use voxa_core::SeriesId;
use voxa_dicom::testkit::{MemorySource, SliceOptions, classic_slice};
use voxa_dicom::{Index, index};

/// Thirty 4 × 4 slices, series number 1.
pub const THICK_SCAN: &str = "1.2.3.1";
/// Sixty-four 8 × 8 slices, series number 2: the most detailed.
pub const FINE_SCAN: &str = "1.2.3.2";
/// Three 4 × 4 slices, series number 3.
pub const SCOUT: &str = "1.2.3.3";

pub fn id(text: &str) -> SeriesId {
    SeriesId(text.to_owned())
}

/// The options of slice `place` (counting from 0) of a series, 5 mm above the one before.
pub fn slice_options(uid: &str, number: i32, place: i32, side: u16) -> SliceOptions {
    SliceOptions {
        series_uid: uid.to_owned(),
        series_number: number,
        instance_number: place + 1,
        rows: side,
        cols: side,
        position: Some([0.0, 0.0, f64::from(place) * 5.0]),
        ..SliceOptions::default()
    }
}

/// The files of one series, named so that name order is slice order.
pub fn series_files(uid: &str, number: i32, count: i32, side: u16) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|place| {
            let bytes = classic_slice(&slice_options(uid, number, place, side));
            (format!("S{number}/I{place:04}.dcm"), bytes)
        })
        .collect()
}

pub fn three_series_files() -> Vec<(String, Vec<u8>)> {
    [
        series_files(THICK_SCAN, 1, 30, 4),
        series_files(FINE_SCAN, 2, 64, 8),
        series_files(SCOUT, 3, 3, 4),
    ]
    .concat()
}

pub fn indexed(files: Vec<(String, Vec<u8>)>) -> Index {
    index(&mut MemorySource::new(files), &mut |_| {}).expect("indexes")
}

pub fn three_series() -> Index {
    indexed(three_series_files())
}
