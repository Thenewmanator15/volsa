//! The file the places are kept in between one run and the next: `places.json`, in the folder
//! Volsa keeps on this computer alone, beside the saved views. It holds a hash for each series
//! and each study, numbers, and a running count of what was written; no identifier, no name,
//! no path and no date. It is read and written as every kept file is (see the module
//! `kept_file`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{MAX_PLACES, Place};
use crate::kept_file::{self, Read};

/// The name of the file within its folder.
const FILE: &str = "places.json";

/// Everything the file holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// The number given to the entry written last. Each entry written takes the next number,
    /// so the numbers say which was written longest ago without saying when.
    written: u64,
    /// Where each series was left, under the series' key.
    series: BTreeMap<String, KeptPlace>,
    /// The series last shown in the first viewport, under each study's key.
    studies: BTreeMap<String, KeptFirst>,
}

/// Where one series was left, and the number it was written with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KeptPlace {
    number: u64,
    place: Place,
}

/// The key of the series to open first in a study, and the number it was written with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KeptFirst {
    number: u64,
    series: String,
}

impl Kept {
    /// Where the series with this key was left, as the file has it: numbers not yet checked.
    pub(crate) fn place(&self, key: &str) -> Option<Place> {
        self.series.get(key).map(|kept| kept.place)
    }

    /// The key of the series to open first: the one kept for whichever of these studies, by
    /// their keys, was written last.
    pub(crate) fn first(&self, studies: &[String]) -> Option<&str> {
        let kept = studies.iter().filter_map(|study| self.studies.get(study));
        let newest = kept.max_by_key(|first| first.number);
        newest.map(|first| first.series.as_str())
    }

    /// The next running number.
    fn next(&mut self) -> u64 {
        self.written = self.written.saturating_add(1);
        self.written
    }
}

/// The file as it is read. It says no version: whatever reads as places is understood.
fn read(folder: &Path) -> Read<Kept> {
    kept_file::read(&folder.join(FILE), |_: &Kept| true)
}

/// What is kept in `folder`. A file that is missing, damaged, cut short or not what this
/// version wrote is read as holding nothing.
pub(crate) fn load(folder: &Path) -> Kept {
    read(folder).held().unwrap_or_default()
}

/// Keep these places, each under its series' key, in place of any kept for those series
/// before, and `first` as a study's key and the key of the series to open first in it; what
/// is kept for other series and studies is left as it is, until there are more than
/// [`MAX_PLACES`], when those written longest ago are dropped. The folder is made if it is not
/// there. A file already there that cannot be read is first put aside, as every kept file is
/// (see the module `kept_file`).
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub(crate) fn save(
    folder: &Path,
    places: &[(String, Place)],
    first: Option<(&str, &str)>,
) -> std::io::Result<()> {
    let was = read(folder);
    let unread = was.is_unread();
    let mut all = was.held().unwrap_or_default();
    for (key, place) in places {
        let (number, place) = (all.next(), *place);
        all.series.insert(key.clone(), KeptPlace { number, place });
    }
    if let Some((study, series)) = first {
        let (number, series) = (all.next(), series.to_owned());
        all.studies
            .insert(study.to_owned(), KeptFirst { number, series });
    }
    bound(&mut all.series, |kept| kept.number);
    bound(&mut all.studies, |kept| kept.number);
    let text = serde_json::to_string_pretty(&all)?;
    kept_file::write(&folder.join(FILE), &text, unread)
}

/// Drop the entries written longest ago, by their numbers, until no more than [`MAX_PLACES`]
/// are left. Of two with one number, as only a file changed by hand has, the one whose key
/// comes first goes first.
fn bound<T>(kept: &mut BTreeMap<String, T>, number: impl Fn(&T) -> u64) {
    let over = kept.len().saturating_sub(MAX_PLACES);
    if over == 0 {
        return;
    }
    let mut oldest: Vec<(u64, String)> = kept
        .iter()
        .map(|(key, entry)| (number(entry), key.clone()))
        .collect();
    oldest.sort_unstable();
    for (_, key) in oldest.into_iter().take(over) {
        kept.remove(&key);
    }
}

#[cfg(test)]
mod tests;
