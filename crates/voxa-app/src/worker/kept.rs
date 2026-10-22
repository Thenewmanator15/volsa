//! What the workers keep: the result of a run, written under the local folder for its
//! series, found again, deleted and saved where the person chooses; and what the person chose
//! for labelling. Nothing here writes a path or a file's name anywhere, and a notice here has
//! none.
//!
//! A result is reported before anything is written. Its files are made after, and held and
//! kept once the state has taken the result in (see `labelling/taken.rs`): the person sees the
//! labels at once, and is told afterwards that they were kept, or that they could not be. The
//! files are made for every result, kept or not, so that the labels can be saved as a file of
//! their own with the structures' own numbers; a person who saves while they are still being
//! made waits for them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use voxa_core::SeriesId;
use voxa_labels::{LabelGrid, LabelSet, Table, read_table, write_labels, write_table};

use super::{Shared, Workers, lock};
use crate::action::{Action, Effect};
use crate::choices::{Choices, save_choices};
use crate::kept_labels::{forget, keep, kept, saved_as, table_for, write_whole};
use crate::labelling::LabellingAction;
use crate::state::{Notice, NoticeKind};
use crate::views::series_key;

/// The key of the notices about a kept result: the last takes the place of the one before.
const KEPT: &str = "kept labels";
/// The key of the notices about saving labels.
const SAVING: &str = "saving labels";
/// What is said when a result could not be kept.
const NOT_KEPT: &str = "These labels could not be kept on this computer. They are shown now and \
                        will be gone when Voxa closes.";
/// What is said when a result could not be deleted.
const NOT_DELETED: &str = "The labels kept for this scan could not be deleted from this computer.";
/// What is said when labels were saved.
const SAVED: &str = "The labels and their table of names were saved.";
/// What is said when labels could not be saved.
const NOT_SAVED: &str = "These labels could not be saved as a file. Only labels that Voxa made \
                         can be saved, and the folder must be one that can be written to.";
/// What is said of the day a kept result was made when its table does not say.
const NO_DAY: &str = "a day that was not written down";
/// The longest table of a kept result that is read, in bytes.
const MAX_TABLE_BYTES: u64 = 4 << 20;
/// The longest label file read from elsewhere that is held so that it can be saved again, in
/// bytes. A file Voxa wrote is a small fraction of this.
pub(super) const MAX_SAVED_BYTES: usize = 32 << 20;

/// The label file and the table of a set of labels, as they are written: the bytes of a
/// `.nii.gz` file and the text of its `.json` table.
pub(super) struct Saved {
    pub(super) labels: Vec<u8>,
    pub(super) table: String,
}

/// The files of a run's labels, as far as they have got.
pub(super) enum Files {
    /// Still being made, by the run.
    Making,
    /// Made, ready to be held and kept.
    Made(Arc<Saved>),
    /// Too big to be written as files: there is nothing to keep or to save.
    Unmade,
}

/// The files of a label grid and its table. The grid is let go as soon as its file is made,
/// before the table is written and before anything is kept.
pub(super) fn files_of(grid: LabelGrid, table: &Table) -> Files {
    let labels = write_labels(&grid);
    drop(grid);
    match labels.ok().zip(write_table(table).ok()) {
        Some((labels, table)) => Files::Made(Arc::new(Saved { labels, table })),
        None => Files::Unmade,
    }
}

/// What the workers keep besides the labels themselves.
#[derive(Default)]
pub(super) struct Keeping {
    /// The files of the labels held, with the set they are the files of. A set that has been
    /// replaced or given up has no files here: the set is held by a `Weak`, so these never
    /// keep one alive.
    saved: Mutex<Option<(Weak<LabelSet>, Arc<Saved>)>>,
    /// Held by a run from before it reports its labels until their files are made, and by
    /// anyone saving, so that a save waits for files that are on their way.
    making: Mutex<()>,
    /// How many times choices have been asked to be written.
    asked: AtomicU64,
    /// The newest of those that has been.
    written: Mutex<u64>,
}

impl Keeping {
    /// Held while the files of a result are being made.
    pub(super) fn making(&self) -> MutexGuard<'_, ()> {
        lock(&self.making)
    }

    /// The files of the labels just read, if they are Voxa's own, taking the place of the
    /// files of any labels before. Called with the labels locked.
    pub(super) fn read(&self, set: &Arc<LabelSet>, saved: Option<Saved>) {
        let held = saved.map(|saved| (Arc::downgrade(set), Arc::new(saved)));
        *lock(&self.saved) = held;
    }

    /// Let go of the files of labels that have been given up.
    pub(super) fn give_up(&self) {
        *lock(&self.saved) = None;
    }
}

impl Workers {
    /// Carry out an effect about what is kept.
    pub(super) fn kept(&self, effect: Effect) {
        match effect {
            Effect::FindKept { series } => self.find_kept(series),
            Effect::ForgetKept { series } => self.forget_kept(&series),
            Effect::SaveLabels { to } => self.save_labels(to),
            Effect::StoreChoices(choices) => self.store_choices(choices),
            _ => {}
        }
    }

    /// Look for the result kept for a series and report it, with the day it was made, if both
    /// its files are there and its table can be read. With nowhere to keep settings there is
    /// nothing to look in.
    fn find_kept(&self, series: SeriesId) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let Some((labels, table)) = kept(&folder, &series_key(&series)) else {
                return;
            };
            let Some(when) = day_of(&table) else {
                return;
            };
            let found = LabellingAction::KeptFound {
                series,
                when,
                labels,
                table,
            };
            shared.report(Action::Labelling(found));
        });
    }

    /// Delete the result kept for a series, and say so if that could not be done.
    fn forget_kept(&self, series: &SeriesId) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let (shared, key) = (Arc::clone(&self.shared), series_key(series));
        self.spawn(move || {
            if forget(&folder, &key).is_err() {
                shared.say(NoticeKind::Warning, NOT_DELETED, KEPT);
            }
        });
    }

    /// Write the labels that are open to a file, and their table beside it, and say how it
    /// went.
    fn save_labels(&self, to: PathBuf) {
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            if shared.save(&to) {
                shared.say(NoticeKind::Info, SAVED, SAVING);
            } else {
                shared.say(NoticeKind::Warning, NOT_SAVED, SAVING);
            }
        });
    }

    /// Write what is chosen to the file it is kept in. Of two asked for close together, the
    /// later is the one left in the file, whichever a worker gets to first. A failure goes to
    /// the log, in words that name no folder.
    fn store_choices(&self, choices: Choices) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let number = self.shared.keeping.asked.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let mut newest_written = lock(&shared.keeping.written);
            if number < *newest_written {
                return;
            }
            *newest_written = number;
            if save_choices(&folder, &choices).is_err() {
                tracing::warn!("the choices for labelling could not be written");
            }
        });
    }
}

impl Shared {
    /// Say something to the person.
    fn say(&self, kind: NoticeKind, text: &str, key: &'static str) {
        self.report(Action::Post(Notice {
            kind,
            text: text.to_owned(),
            details: Vec::new(),
            key: Some(key),
        }));
    }

    /// Keep the files of a result that the state has taken in for the series `keep_for` names,
    /// made on the day it gives, and say where they are found; or say that they could not be
    /// kept. Nothing when the result is not to be kept.
    pub(super) fn keep_files(&self, files: &Files, keep_for: Option<&(SeriesId, String)>) {
        let Some((series, when)) = keep_for else {
            return;
        };
        match files {
            Files::Made(saved) => self.keep_for(series, saved, when),
            Files::Unmade => self.say(NoticeKind::Warning, NOT_KEPT, KEPT),
            Files::Making => {}
        }
    }

    /// Hold the files of `set`, so that the labels can be saved, if it is still the labels
    /// held.
    pub(super) fn hold_files(&self, set: &Arc<LabelSet>, saved: &Arc<Saved>) {
        let labels = lock(&self.labels);
        if labels.as_ref().is_some_and(|now| Arc::ptr_eq(now, set)) {
            let held = (Arc::downgrade(set), Arc::clone(saved));
            *lock(&self.keeping.saved) = Some(held);
        }
    }

    /// Keep the files for a series, and say where they are found; or say that they could not
    /// be kept. With nowhere to keep settings, they cannot be.
    fn keep_for(&self, series: &SeriesId, saved: &Saved, when: &str) {
        let key = series_key(series);
        let folder = self.settings.as_deref();
        let written =
            folder.is_some_and(|folder| keep(folder, &key, &saved.labels, &saved.table).is_ok());
        let files = folder
            .filter(|_| written)
            .and_then(|folder| kept(folder, &key));
        match files {
            Some((labels, table)) => {
                let found = LabellingAction::KeptFound {
                    series: series.clone(),
                    when: when.to_owned(),
                    labels,
                    table,
                };
                self.report(Action::Labelling(found));
            }
            None => self.say(NoticeKind::Warning, NOT_KEPT, KEPT),
        }
    }

    /// The files of the labels held, if they are known.
    fn files_held(&self) -> Option<Arc<Saved>> {
        let now = lock(&self.labels).clone()?;
        let (of, saved) = lock(&self.keeping.saved).clone()?;
        Weak::ptr_eq(&of, &Arc::downgrade(&now)).then_some(saved)
    }

    /// Write the files of the labels held to `to`, under a name that ends in `.nii.gz`, and,
    /// beside it, to the name of their table. False when there are no files to write, or they
    /// could not be written.
    fn save(&self, to: &Path) -> bool {
        // Files that are still being made are waited for.
        let _made = self.keeping.making();
        let Some(files) = self.files_held() else {
            return false;
        };
        let to = saved_as(to);
        write_whole(&to, &files.labels).is_ok()
            && write_whole(&table_for(&to), files.table.as_bytes()).is_ok()
    }
}

/// The day a kept result was made, as its table says it, or that it does not say. `None` when
/// the table cannot be read, or is longer than any Voxa writes: that is no kept result.
fn day_of(table: &Path) -> Option<String> {
    let size = std::fs::metadata(table).ok()?.len();
    if size > MAX_TABLE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(table).ok()?;
    let table = read_table("table.json", &text).ok()?;
    let day = table
        .made
        .map(|made| made.when)
        .filter(|day| !day.is_empty());
    Some(day.unwrap_or_else(|| NO_DAY.to_owned()))
}
