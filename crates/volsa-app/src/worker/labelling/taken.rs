//! What a run made, from its report until the state takes it in. The labels held are the ones
//! drawn, named under the pointer and saved, and they change only when the state takes a run's
//! result in (`Effect::TakeLabels`): the window never draws a run's labels while its list
//! shows others. A result the state does not take, because the run was stopped or the scene
//! moved on before the report reached it, is let go when the run is stopped, when another run
//! begins and when the labels are given up. Nothing of it is kept for its series until it is
//! taken.

use std::sync::Arc;

use volsa_core::SeriesId;
use volsa_labels::{LabelSet, MAX_MADE_LINE, MAX_NOTE, MAX_NOTES, Made, Table, label_set};
use volsa_models::{Device, Model, Room, Rung};
use volsa_segment::Labelled;

use super::room::{PROCESSOR, card_name, fall};
use super::{Job, Kit};
use crate::action::Action;
use crate::labels::NamesFrom;
use crate::worker::kept::{Files, files_of};
use crate::worker::{Shared, Workers, lock};

/// The note on a result whose scores network ran on the processor, though the rest did not.
const SCORES_ON_THE_PROCESSOR: &str = "The model checked its own work on the processor: that \
                                       step did not fit on the graphics card.";

/// What a run made that the state has not taken in, or has taken in while its files were still
/// being made.
pub(super) struct Unclaimed {
    /// The run that made it.
    run: u64,
    set: Arc<LabelSet>,
    files: Files,
    /// The series it is to be kept for, if it is, and the day it was made.
    keep_for: Option<(SeriesId, String)>,
    /// Taken in by the state while its files were being made: the run holds and keeps them
    /// once it has made them.
    taken: bool,
}

impl Kit {
    /// Let go of what a run made that the state has not taken in.
    pub(in crate::worker) fn let_go(&self) {
        let mut made = lock(&self.made);
        if made.as_ref().is_some_and(|made| !made.taken) {
            *made = None;
        }
    }
}

impl Workers {
    /// The state has taken in the labels run `run` made: hold them in place of any before, and
    /// their files with them, and keep the files for their series if they are to be. Files
    /// still being made are held and kept by the run once they are. Labels let go before, or of
    /// another run, are not taken.
    pub(in crate::worker) fn take_labels(&self, run: u64) {
        let shared = &self.shared;
        let mut unclaimed = lock(&shared.labelling.made);
        let Some(made) = unclaimed
            .as_mut()
            .filter(|made| made.run == run && !made.taken)
        else {
            return;
        };
        *lock(&shared.labels) = Some(Arc::clone(&made.set));
        shared.keeping.give_up();
        if let Files::Made(saved) = &made.files {
            shared.hold_files(&made.set, saved);
        } else if let Files::Making = made.files {
            made.taken = true;
            return;
        }
        let Some(made) = unclaimed.take() else {
            return;
        };
        drop(unclaimed);
        if made.keep_for.is_some() {
            // Writing the files to be kept takes a moment: not on the window's thread.
            let shared = Arc::clone(shared);
            self.spawn(move || shared.keep_files(&made.files, made.keep_for.as_ref()));
        }
    }
}

impl Shared {
    /// A rung gave labels: say on them who made them, where, and anything the labels cannot
    /// show of how; then report them, unless the run is no longer the one waited for, and hold
    /// them aside until the state takes them in. Their files are made once they are reported,
    /// with the grid let go as soon as its file is.
    pub(super) fn done(
        &self,
        job: &Job,
        model: &Model,
        room: &Room,
        rung: Rung,
        labelled: Labelled,
        gave_way: Option<(Device, bool)>,
    ) {
        let Labelled {
            grid,
            table,
            mut notes,
        } = labelled;
        notes.extend(fall(gave_way, rung.device, room));
        if rung.device != Device::Processor && rung.scores_on == Device::Processor {
            notes.push(SCORES_ON_THE_PROCESSOR.to_owned());
        }
        let device = match rung.device {
            Device::Processor => PROCESSOR.to_owned(),
            Device::Card(card) => card_name(room, card),
        };
        let when = self.labelling.labeller.today();
        let made = made(&table.title, &model.sum, &device, &when, &notes);
        let day = made.when.clone();
        let table = Table {
            made: Some(made),
            ..table
        };
        let set = match label_set(&grid, Some(&table)) {
            Ok(set) => Arc::new(set),
            Err(why) => return self.fail(job, &why.to_string()),
        };
        // A save waits for the files of these labels from here until they are made.
        let _making = self.keeping.making();
        let mut unclaimed = lock(&self.labelling.made);
        if !self.current(job) {
            return;
        }
        *unclaimed = Some(Unclaimed {
            run: job.run,
            set: Arc::clone(&set),
            files: Files::Making,
            keep_for: job.keep.then(|| (job.series.clone(), day)),
            taken: false,
        });
        drop(unclaimed);
        self.report(Action::LabelsRead {
            title: set.title.clone(),
            parts: set.parts.clone(),
            verdict: set.verdict.clone(),
            corners: set.corners(),
            names_from: NamesFrom::Model,
            made: set.made.clone(),
            run: Some(job.run),
        });
        self.files_made(job.run, files_of(grid, &table));
    }

    /// A run's files are made. Before the state takes the run's labels in, they wait with
    /// them; once it has, they are held, if the labels are still the ones held, and kept for
    /// their series if they are to be. Labels that were let go are let go with them.
    fn files_made(&self, run: u64, files: Files) {
        let mut unclaimed = lock(&self.labelling.made);
        match unclaimed.take() {
            Some(mut made) if made.run == run && !made.taken => {
                made.files = files;
                *unclaimed = Some(made);
            }
            Some(made) if made.run == run => {
                drop(unclaimed);
                if let Files::Made(saved) = &files {
                    self.hold_files(&made.set, saved);
                }
                self.keep_files(&files, made.keep_for.as_ref());
            }
            other => *unclaimed = other,
        }
    }
}

/// Who made a label set, cut to the lengths a table keeps.
fn made(model: &str, sum: &str, device: &str, when: &str, notes: &[String]) -> Made {
    let line = |text: &str| text.chars().take(MAX_MADE_LINE).collect::<String>();
    let note = |text: &String| text.chars().take(MAX_NOTE).collect::<String>();
    Made {
        by: line(&format!("Volsa {}", env!("CARGO_PKG_VERSION"))),
        model: line(model),
        model_sum: sum.to_owned(),
        device: line(device),
        when: line(when),
        notes: notes.iter().take(MAX_NOTES).map(note).collect(),
    }
}
