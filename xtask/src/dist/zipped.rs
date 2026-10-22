//! The zip a release offers: the package's folder, compressed, each entry dated now.

use std::collections::BTreeSet;
use std::fs::File;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::failed;

/// Writes the zip: the folder `top`, its folders and then its files, each compressed and
/// dated now.
pub fn zipped(folder: &Path, top: &str, names: &[String], zip: &Path) -> Result<(), String> {
    let written =
        |error: &dyn std::fmt::Display| format!("could not write {}: {error}", zip.display());
    let file = File::create(zip).map_err(|error| written(&error))?;
    let mut writer = ZipWriter::new(file);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip_time(now.as_secs()));
    let mut folders = BTreeSet::from([format!("{top}/")]);
    for name in names {
        let parents = name
            .match_indices('/')
            .map(|(at, _)| format!("{top}/{}/", &name[..at]));
        folders.extend(parents);
    }
    for inner in folders {
        writer
            .add_directory(inner, options)
            .map_err(|error| written(&error))?;
    }
    for name in names {
        let from = folder.join(name);
        let mut read = File::open(&from).map_err(|error| failed("read", &from, &error))?;
        writer
            .start_file(format!("{top}/{name}"), options)
            .map_err(|error| written(&error))?;
        io::copy(&mut read, &mut writer).map_err(|error| written(&error))?;
    }
    writer.finish().map_err(|error| written(&error))?;
    Ok(())
}

/// A zip's date and time for `seconds` after the start of 1970, at Greenwich. A time a zip
/// cannot hold, before 1980 or after 2107, is given as the start of 1980.
pub fn zip_time(seconds: u64) -> DateTime {
    let (year, month, day) = civil(seconds / 86_400);
    let of_day = seconds % 86_400;
    let [hour, minute, second] = [of_day / 3600, of_day % 3600 / 60, of_day % 60]
        .map(|part| u8::try_from(part).unwrap_or(0));
    let year = u16::try_from(year).unwrap_or(0);
    DateTime::from_date_and_time(year, month, day, hour, minute, second).unwrap_or_default()
}

/// The year, month and day of the day `days` after the first of January 1970.
pub fn civil(days: u64) -> (u64, u8, u8) {
    // Days counted from the first of March of the year 0, in eras of 400 years.
    let counted = days + 719_468;
    let era = counted / 146_097;
    let of_era = counted % 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let from_march = (5 * of_year + 2) / 153;
    let day = of_year - (153 * from_march + 2) / 5 + 1;
    let month = if from_march < 10 {
        from_march + 3
    } else {
        from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    let small = |part: u64| u8::try_from(part).unwrap_or(0);
    (year, small(month), small(day))
}
