//! Reading a file of labels on a worker: the label volume, the table that names its parts,
//! and the label set made of the two. Nothing here writes a path or a file's name anywhere.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use voxa_labels::{LabelSet, Table, label_set, read_labels, read_table};

use super::{Shared, Workers, lock, shielded};
use crate::action::Action;
use crate::labels::NamesFrom;
use crate::state::{Notice, NoticeKind};

/// The largest label file read, in bytes.
pub const MAX_LABEL_FILE_BYTES: u64 = 1 << 30;
/// The largest table of names read from a file, in bytes.
const MAX_TABLE_FILE_BYTES: u64 = 8 << 20;
/// What is said when the table of names found beside the labels cannot be read.
pub const NAMES_NOT_READ: &str =
    "The table of names for these labels could not be read, so the parts are shown by number.";
const NOT_READ: &str = "The label file could not be read.";
const TOO_LARGE: &str = "The label file is larger than Voxa will read.";
const STOPPED: &str = "Reading the labels stopped part-way.";
/// The notice about names takes the place of an earlier one about names.
const NAMES: &str = "label names";

/// The table of names that goes with a label file, by the names such files are given: for
/// `x.nii` or `x.nii.gz`, a file `x.json` or `x.ctbl` beside it; for `labels<rest>`, also
/// `regions<rest>.json`; then `regions.json` and `labels.ctbl`. `exists` says whether a file
/// is there. `None` when none of them is.
pub fn table_beside(labels: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let name = labels.file_name()?.to_string_lossy().into_owned();
    let lower = name.to_ascii_lowercase();
    let ending = [".nii.gz", ".nii"]
        .into_iter()
        .find(|ending| lower.ends_with(ending));
    let stem = name.get(..name.len() - ending.map_or(0, str::len))?;
    let mut names = vec![format!("{stem}.json"), format!("{stem}.ctbl")];
    if let Some(rest) = stem.strip_prefix("labels") {
        names.push(format!("regions{rest}.json"));
    }
    names.extend(["regions.json", "labels.ctbl"].map(str::to_owned));
    let mut tried: Vec<String> = Vec::new();
    for name in names {
        if tried.contains(&name) {
            continue;
        }
        let beside = labels.with_file_name(&name);
        if exists(&beside) {
            return Some(beside);
        }
        tried.push(name);
    }
    None
}

impl Workers {
    /// The label set kept, if labels have been read and not given up.
    pub fn labels(&self) -> Option<Arc<LabelSet>> {
        lock(&self.shared.labels).clone()
    }

    /// Read a label file on a worker, with the table named or the one found beside it, keep
    /// the set made of them and report it. A read overtaken by a newer one, or given up,
    /// keeps and reports nothing.
    pub(super) fn read_labels(&self, labels: PathBuf, table: Option<PathBuf>) {
        let number = self.shared.label_reads.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let mut outcome = Err(STOPPED.to_owned());
            shielded(|| outcome = shared.labels_from(&labels, table.as_deref()));
            let mut kept = lock(&shared.labels);
            if shared.label_reads.load(Ordering::SeqCst) != number {
                return;
            }
            let report = match outcome {
                Ok((set, names_from)) => {
                    let set = Arc::new(set);
                    *kept = Some(Arc::clone(&set));
                    Action::LabelsRead {
                        title: set.title.clone(),
                        parts: set.parts.clone(),
                        verdict: set.verdict.clone(),
                        corners: set.corners(),
                        names_from,
                    }
                }
                Err(why) => Action::LabelsFailed(why),
            };
            drop(kept);
            shared.report(report);
        });
    }

    /// Give up the labels kept, and any being read.
    pub(super) fn drop_labels(&self) {
        self.shared.label_reads.fetch_add(1, Ordering::SeqCst);
        *lock(&self.shared.labels) = None;
    }
}

impl Shared {
    /// Read a label file and its table into a label set, and say where its names came from.
    /// What goes wrong is a sentence for the person, with no path in it.
    fn labels_from(
        &self,
        labels: &Path,
        table: Option<&Path>,
    ) -> Result<(LabelSet, NamesFrom), String> {
        let bytes = whole_file(labels, MAX_LABEL_FILE_BYTES)?;
        let grid = read_labels(&bytes).map_err(|why| why.to_string())?;
        drop(bytes);
        let named = table.map(Path::to_path_buf);
        let found = named.or_else(|| table_beside(labels, Path::is_file));
        let table = found.map(|table| table_from(&table));
        if let Some(Err(())) = &table {
            self.report(Action::Post(Notice {
                kind: NoticeKind::Warning,
                text: NAMES_NOT_READ.to_owned(),
                details: Vec::new(),
                key: Some(NAMES),
            }));
        }
        let table = table.and_then(Result::ok);
        let names_from = if table.is_some() {
            NamesFrom::Table
        } else {
            NamesFrom::Nothing
        };
        let set = label_set(&grid, table.as_ref()).map_err(|why| why.to_string())?;
        Ok((set, names_from))
    }
}

/// Everything in a file no larger than `most` bytes.
fn whole_file(path: &Path, most: u64) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|_| NOT_READ.to_owned())?
        .len();
    if size > most {
        return Err(TOO_LARGE.to_owned());
    }
    std::fs::read(path).map_err(|_| NOT_READ.to_owned())
}

/// The table of names in a file, or nothing but the fact that it could not be read.
fn table_from(path: &Path) -> Result<Table, ()> {
    let bytes = whole_file(path, MAX_TABLE_FILE_BYTES).map_err(|_| ())?;
    let text = String::from_utf8(bytes).map_err(|_| ())?;
    let name = path.file_name().ok_or(())?.to_string_lossy();
    read_table(&name, &text).map_err(|_| ())
}
