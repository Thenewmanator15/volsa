//! Checks that Volsa can read the whole of an export: indexes it, decodes every slice of every
//! series, and prints counts, sizes and times.
//!
//! ```text
//! cargo run --release -p volsa-app --example check_export -- some/export.zip
//! ```
//!
//! Nothing about the patient is printed: no name, no identifier, no date, no description.
//! What is printed is safe to paste into a bug report.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use volsa_dicom::{
    IndexedSeries, READING_STACK_BYTES, SeriesSource, decode_slice, index, list_tags, open_source,
};

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: check_export PATH   (a folder, a zip file or a DICOMDIR)");
        return ExitCode::from(2);
    };
    // Read as Volsa reads, on a thread with the stack its readers have, not on this one.
    let reading = std::thread::Builder::new()
        .stack_size(READING_STACK_BYTES)
        .spawn(move || check_export(&path));
    let Ok(Ok(code)) = reading.map(std::thread::JoinHandle::join) else {
        eprintln!("The export could not be read through.");
        return ExitCode::FAILURE;
    };
    code
}

/// Index the export at `path`, decode every slice, and say how that went.
fn check_export(path: &std::path::Path) -> ExitCode {
    let started = Instant::now();
    let mut source = match open_source(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let found = match index(source.as_mut(), &mut |_| {}) {
        Ok(found) => found,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Indexed in {:.2} s: {} studies, {} series, {} files skipped, {} refused unread",
        started.elapsed().as_secs_f64(),
        found.studies.len(),
        found.series.len(),
        found.skipped.len(),
        found.refused.len()
    );
    let mut failures = 0;
    for series in &found.series {
        failures += check(source.as_mut(), series);
    }
    println!("Slices that could not be decoded: {failures}");
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Decode every slice of a series and print one line about it; how many slices failed.
fn check(source: &mut dyn SeriesSource, series: &IndexedSeries) -> usize {
    let info = &series.info;
    let started = Instant::now();
    let mut failed = 0;
    let mut first_reason = None;
    for slice in &series.slices {
        if let Err(error) = decode_slice(source, slice) {
            failed += 1;
            first_reason.get_or_insert_with(|| error.to_string());
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    let per_slice = 1000.0 * seconds / series.slices.len().max(1) as f64;
    // The transfer syntax says how the pixels are stored, and so which decoder was used.
    let stored_as = series
        .slices
        .first()
        .and_then(|first| list_tags(source, first).ok());
    let stored_as = stored_as
        .and_then(|rows| rows.into_iter().find(|row| row.tag == "(0002,0010)"))
        .map_or_else(|| "unknown".to_owned(), |row| row.value);
    let [between_cols, between_rows, between_slices] = info.spacing_mm;
    println!(
        "series {:>4} {:>3}: {:>4} slices of {:>4} x {:<4}  {:.3} x {:.3} x {:.3} mm  geometry {}  \
         problems {}  stored as {}  decoded {} of {} in {:.2} s ({:.1} ms each){}",
        info.number,
        info.modality,
        info.slices,
        info.cols,
        info.rows,
        between_cols,
        between_rows,
        between_slices,
        if info.has_geometry() { "yes" } else { "no" },
        info.problems.len(),
        stored_as,
        series.slices.len() - failed,
        series.slices.len(),
        seconds,
        per_slice,
        first_reason.map_or_else(String::new, |reason| format!("  first failure: {reason}")),
    );
    for problem in &info.problems {
        println!("             {problem}");
    }
    failed
}
