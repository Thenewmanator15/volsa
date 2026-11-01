//! Moving what Volsa keeps out of the roaming settings folder. 0.5.0 and earlier kept the
//! saved views, the annotations, the choices for labelling and the kept labelling results in
//! the folder the system gives for settings, which on Windows is in the roaming profile that a
//! managed network copies to its servers. They are kept now in the folder on this computer
//! alone, beside the log, and on the first start of a newer Volsa whatever is in the old place
//! is moved there, once: each file whole, never two copies left, and a file already in the new
//! place never written over. Only `preferences.json` stays where it was.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The files moved, by the start of their names, which also takes in what was being written
/// beside them or set aside when Volsa stopped: `views.json.new`, `annotations.json.unread`.
const MOVED: [&str; 3] = ["views.json", "annotations.json", "choices.json"];
/// The folder of kept labelling results, each two files named by its series' key.
const RESULTS: &str = "labels";
/// What is added to a file's name while it is copied to another drive.
const MOVING: &str = ".moving";

/// What a move did, for the log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moved {
    /// The files moved, by name, in order.
    pub files: Vec<String>,
    /// How many files of kept labelling results were moved.
    pub results: usize,
    /// The files left where they were, by name, in order.
    pub left: Vec<String>,
    /// How many files of kept labelling results were left where they were.
    pub left_results: usize,
}

/// How the move of one file went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// It is in the new place, and no longer in the old one.
    Moved,
    /// It is where it was, and nothing of it is in the new place.
    Left,
}

impl Moved {
    /// What the log says of the move, in a sentence or two that name only Volsa's own files and
    /// count the results, whose names are hashes. `None` when there was nothing to move.
    pub fn sentence(&self) -> Option<String> {
        let mut said = Vec::new();
        if let Some(moved) = listed(&self.files, self.results) {
            said.push(format!(
                "Moved Volsa's kept files out of the roaming settings folder into the local one: \
                 {moved}."
            ));
        }
        if let Some(left) = listed(&self.left, self.left_results) {
            said.push(format!(
                "Left {left} in the roaming settings folder: the local one has a different file \
                 of the same name, or the move failed."
            ));
        }
        (!said.is_empty()).then(|| said.join(" "))
    }
}

/// Names and a count of results as a list in words: `a, b and 2 files of kept labelling
/// results`. `None` for nothing.
fn listed(names: &[String], results: usize) -> Option<String> {
    let mut items = names.to_vec();
    if results > 0 {
        let files = if results == 1 { "file" } else { "files" };
        items.push(format!("{results} {files} of kept labelling results"));
    }
    let (last, rest) = items.split_last()?;
    Some(if rest.is_empty() {
        last.clone()
    } else {
        format!("{} and {last}", rest.join(", "))
    })
}

/// Move what Volsa keeps from `from`, the roaming settings folder, to `to`, the folder on this
/// computer alone, making it if need be, and say what was moved and what was left. Nothing is
/// moved when the two are one folder, or when `from` holds nothing to move.
pub fn move_kept(from: &Path, to: &Path) -> Moved {
    let mut moved = Moved::default();
    if one_folder(from, to) {
        return moved;
    }
    for name in names_in(from, |name| MOVED.iter().any(|kept| name.starts_with(kept))) {
        let outcome = move_one(&from.join(&name), &to.join(&name));
        match outcome {
            Outcome::Moved => moved.files.push(name),
            Outcome::Left => moved.left.push(name),
        }
    }
    let (old, new) = (from.join(RESULTS), to.join(RESULTS));
    for names in results_in(&old).into_values() {
        match move_result(&old, &new, &names) {
            Outcome::Moved => moved.results += names.len(),
            Outcome::Left => moved.left_results += names.len(),
        }
    }
    // The old folder of results goes once it is empty; while anything is left in it, it stays.
    let _ = std::fs::remove_dir(&old);
    moved
}

/// Whether two paths name one folder, however they are written.
fn one_folder(one: &Path, other: &Path) -> bool {
    one == other
        || matches!(
            (one.canonicalize(), other.canonicalize()),
            (Ok(one), Ok(other)) if one == other
        )
}

/// The names of the files in a folder that `wanted` takes, in order. None when there is no
/// such folder.
fn names_in(folder: &Path, wanted: impl Fn(&str) -> bool) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| wanted(name))
        .collect();
    names.sort();
    names
}

/// The kept results in a folder, each the names of its files under its series' key: the part
/// of a name before its first dot.
fn results_in(folder: &Path) -> BTreeMap<String, Vec<String>> {
    let mut results: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in names_in(folder, |_| true) {
        let key = name.split('.').next().unwrap_or_default().to_owned();
        results.entry(key).or_default().push(name);
    }
    results
}

/// Move the files of one kept result, which belong together: none of them is moved when a
/// different file of the same name is already in the new place, so that a label file is never
/// paired with another result's table.
fn move_result(old: &Path, new: &Path, names: &[String]) -> Outcome {
    let in_the_way = |name: &String| {
        let there = new.join(name);
        there.exists() && !same_bytes(&old.join(name), &there)
    };
    if names.iter().any(in_the_way) {
        return Outcome::Left;
    }
    let outcomes: Vec<Outcome> = names
        .iter()
        .map(|name| move_one(&old.join(name), &new.join(name)))
        .collect();
    if outcomes.contains(&Outcome::Left) {
        Outcome::Left
    } else {
        Outcome::Moved
    }
}

/// Whether two files hold the same bytes.
fn same_bytes(one: &Path, other: &Path) -> bool {
    matches!((std::fs::read(one), std::fs::read(other)), (Ok(one), Ok(other)) if one == other)
}

/// Move one file, making the folder it goes to. A different file of the same name in the new
/// place is never written over: the old file is left. The same bytes there are what a move cut
/// short leaves behind, and the old copy is taken away.
fn move_one(old: &Path, new: &Path) -> Outcome {
    if new.exists() {
        let finished = same_bytes(old, new) && std::fs::remove_file(old).is_ok();
        return if finished {
            Outcome::Moved
        } else {
            Outcome::Left
        };
    }
    let made = new.parent().map_or(Ok(()), std::fs::create_dir_all);
    if made.is_err() {
        return Outcome::Left;
    }
    if std::fs::rename(old, new).is_ok() {
        return Outcome::Moved;
    }
    // A folder on another drive cannot be renamed into; the file is copied there instead.
    copy_across(old, new)
}

/// Copy a file to another drive and take the old one away. It is copied beside its new place
/// first and then renamed into it, so the new place never holds half a file; and when the old
/// one cannot be taken away, neither can the new one stay, so there are never two.
fn copy_across(old: &Path, new: &Path) -> Outcome {
    let mut beside = new.as_os_str().to_owned();
    beside.push(MOVING);
    let beside = PathBuf::from(beside);
    let copied = std::fs::copy(old, &beside).and_then(|_| std::fs::rename(&beside, new));
    if copied.is_err() {
        let _ = std::fs::remove_file(&beside);
        return Outcome::Left;
    }
    match std::fs::remove_file(old) {
        Ok(()) => Outcome::Moved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Outcome::Moved,
        Err(_) => {
            let _ = std::fs::remove_file(new);
            Outcome::Left
        }
    }
}

/// Move what Volsa keeps out of the roaming settings folder `settings` into the folder on this
/// computer alone, `local`, and say in the log what was moved. Nothing is done when either is
/// not there, as when the run is to take a picture of the window.
pub fn into_local(settings: Option<&Path>, local: Option<&Path>) {
    let (Some(settings), Some(local)) = (settings, local) else {
        return;
    };
    if let Some(said) = move_kept(settings, local).sentence() {
        tracing::info!("{said}");
    }
}

#[cfg(test)]
mod tests;
