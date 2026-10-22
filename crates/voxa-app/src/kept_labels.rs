//! A labelling result kept on this computer for its series, to be shown again: a label file
//! and the table that names its parts, in a `labels` folder in the folder on this computer
//! alone, beside the log. They are named by the series' key (see
//! [`series_key`](crate::series_key)), a hash that says nothing of the scan, and hold nothing
//! that does.
//!
//! Each file is written beside itself under a `.part` name and then renamed, the table last,
//! so a result counts as kept only when both are there and whole.

use std::io;
use std::path::{Path, PathBuf};

/// The folder within Voxa's own folder that holds the results.
const FOLDER: &str = "labels";
/// How the label file and its table end.
const LABELS_END: &str = "nii.gz";
const TABLE_END: &str = "json";
/// What is added to a file's name while it is being written.
const PART: &str = ".part";

/// What is said when a key could not be the name of a file.
const BAD_KEY: &str = "That is not a key a result is kept under.";

/// The two files a result is kept in, for the series with this key: the labels, then their
/// table. A key is letters and digits, so that it cannot name a place outside the folder.
fn files(folder: &Path, key: &str) -> io::Result<(PathBuf, PathBuf)> {
    if key.is_empty() || !key.chars().all(|one| one.is_ascii_alphanumeric()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, BAD_KEY));
    }
    let inside = folder.join(FOLDER);
    Ok((
        inside.join(format!("{key}.{LABELS_END}")),
        inside.join(format!("{key}.{TABLE_END}")),
    ))
}

/// The name a file is written under before it is moved into place: the same with `.part` added.
fn beside(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(PART);
    PathBuf::from(name)
}

/// Write a file whole: beside itself first, then moved into place, so that a failure half way
/// leaves what was there before (or nothing) and never half a file.
pub(crate) fn write_whole(file: &Path, bytes: &[u8]) -> io::Result<()> {
    let part = beside(file);
    let written = std::fs::write(&part, bytes).and_then(|()| std::fs::rename(&part, file));
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written
}

/// Take a file away; one that is not there is as good as gone.
fn remove(file: &Path) -> io::Result<()> {
    match std::fs::remove_file(file) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Keep a result for the series with this key, in place of any kept for it before: the bytes
/// of the label file and the text of its table. The folder is made if it is not there. The
/// older table goes before the new labels come, so that no table is ever beside labels that
/// are not its own, and the new table comes last: the result is whole only then.
///
/// # Errors
///
/// When the key is not a name, the folder cannot be made or a file cannot be written. What was
/// partly written is taken away again.
pub fn keep(folder: &Path, key: &str, labels: &[u8], table: &str) -> io::Result<()> {
    let (labels_file, table_file) = files(folder, key)?;
    std::fs::create_dir_all(folder.join(FOLDER))?;
    let (labels_part, table_part) = (beside(&labels_file), beside(&table_file));
    let moved = std::fs::write(&labels_part, labels)
        .and_then(|()| std::fs::write(&table_part, table))
        .and_then(|()| remove(&table_file))
        .and_then(|()| std::fs::rename(&labels_part, &labels_file))
        .and_then(|()| std::fs::rename(&table_part, &table_file));
    if moved.is_err() {
        let _ = remove(&labels_part);
        let _ = remove(&table_part);
    }
    moved
}

/// Whether a file is there and holds something.
fn whole(file: &Path) -> bool {
    std::fs::metadata(file).is_ok_and(|found| found.is_file() && found.len() > 0)
}

/// The label file and its table kept for the series with this key, when both are there. A
/// file that is still under its `.part` name is not one.
pub fn kept(folder: &Path, key: &str) -> Option<(PathBuf, PathBuf)> {
    let (labels, table) = files(folder, key).ok()?;
    (whole(&labels) && whole(&table)).then_some((labels, table))
}

/// Delete the result kept for the series with this key, with anything left of one being
/// written. A result that is not there is as good as deleted.
///
/// # Errors
///
/// When a file is there and cannot be deleted. The others are deleted all the same.
pub fn forget(folder: &Path, key: &str) -> io::Result<()> {
    let (labels, table) = files(folder, key)?;
    let all = [beside(&labels), beside(&table), labels, table];
    all.iter()
        .map(|file| remove(file))
        .fold(Ok(()), Result::and)
}

/// Where the table of labels saved as `labels` goes: beside them, under the same name with
/// `.json` for `.nii.gz` or `.nii`, which is where Voxa looks for it when they are opened.
pub(crate) fn table_for(labels: &Path) -> PathBuf {
    let name = labels.file_name().map(|name| name.to_string_lossy());
    let name = name.unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let cut = [".nii.gz", ".nii"]
        .into_iter()
        .find(|ending| lower.ends_with(ending))
        .map_or(0, str::len);
    let stem = name.get(..name.len() - cut).unwrap_or_default();
    labels.with_file_name(format!("{stem}.{TABLE_END}"))
}

/// The name labels asked to be saved as `to` are written under: one that ends in `.nii.gz`,
/// since what is written is a gzipped NIfTI file, and Voxa and other programs know it as one
/// by that ending. A name that ends in `.nii` gains `.gz`, one that ends in `.gz` gains `.nii`
/// before it, and any other gains `.nii.gz`.
pub(crate) fn saved_as(to: &Path) -> PathBuf {
    let name = to.file_name().map(|name| name.to_string_lossy());
    let name = name.unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let ending = [".nii.gz", ".nii", ".gz"]
        .into_iter()
        .find(|ending| lower.ends_with(ending));
    let named = match ending {
        Some(".nii.gz") => name.into_owned(),
        Some(".nii") => format!("{name}.gz"),
        Some(gz) => {
            let stem = name.get(..name.len() - gz.len()).unwrap_or_default();
            format!("{stem}.nii.gz")
        }
        None => format!("{name}.nii.gz"),
    };
    to.with_file_name(named)
}

#[cfg(test)]
mod tests;
