//! Labelling a scan on a worker: looking for the model and the runtime (the module `look`),
//! and running the networks on the volume kept for a series, from one place to try to the
//! next, to a label set that is kept and reported. A run says what it says with its number.
//! One that is stopped or overtaken, or whose export has been closed, keeps and says nothing
//! more. One whose volume the store has given up says once that the scan changed, so that no
//! one waits for it. One run has its networks open at a time: the next waits until a stopped
//! one is over.
//!
//! The runtime's own words for a failure go to the log, with every path taken out. Nothing
//! else of a run is logged but what memory it went by, and never a path or a file's name.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use voxa_core::{SeriesId, Volume};
use voxa_labels::{MAX_MADE_LINE, MAX_NOTE, MAX_NOTES, Made, Table, label_set};
use voxa_models::{
    Device, Model, Network, Role, Room, RunError, Rung, RunsOn, Stop, Wanted, rungs,
};
use voxa_segment::{LabelError, Labelled, Networks, Stage, label, sizes};

use super::{Shared, Workers, lock, shielded};
use crate::action::{Action, Effect};
use crate::labelling::LabellingAction;
use crate::labels::NamesFrom;

mod labeller;
mod look;
mod room;

pub use labeller::Labeller;
use labeller::ThisComputer;
use room::{PROCESSOR, card_name, does_not_fit, logged_words, place, room_of};
#[cfg(test)]
use room::{card_named, free_bytes, without_paths};

#[cfg(test)]
#[path = "labelling_lab.rs"]
mod lab;
#[cfg(test)]
#[path = "labelling_look_tests.rs"]
mod look_tests;
#[cfg(test)]
#[path = "labelling_stand_ins.rs"]
mod stand_ins;
#[cfg(test)]
#[path = "labelling_tests.rs"]
mod tests;

/// The notes on a result that ran elsewhere than where it was first tried.
const NO_ROOM: &str = "Run on the processor: the graphics card did not have room.";
const LOST: &str = "Run on the processor: the graphics card stopped answering.";
const SCORES_ON_THE_PROCESSOR: &str = "The model checked its own work on the processor: that \
                                       step did not fit on the graphics card.";
/// What is said when a run stops part-way.
const PART_WAY: &str = "Labelling stopped part-way: something went wrong inside it.";
/// What is said when there is no volume, or no model, to run on.
const NO_VOLUME: &str = "The 3D view of this scan is no longer there to label.";
const NO_MODEL: &str = "Voxa has not yet checked whether this computer can label a scan.";
/// What is said when the volume of a run is no longer the one kept for its series.
const CHANGED: &str = "Labelling stopped because the scan in view changed.";

/// What labelling a scan keeps from one effect to the next.
pub(super) struct Kit {
    /// What runs are done with.
    labeller: Arc<dyn Labeller>,
    /// The number of the newest look asked for.
    looks: AtomicU64,
    /// The model the newest look found, while it found this computer able.
    model: Mutex<Option<Arc<Model>>>,
    /// The switch that stops the run on, if one is.
    run: Mutex<Option<Stop>>,
    /// Held by a run from its start to its end: only one run has networks open at a time,
    /// and the next waits for a stopped one to be over.
    turn: Mutex<()>,
}

impl Kit {
    /// What labelling keeps, with runs done with `labeller`, or with the runtime and this
    /// computer when there is none.
    pub(super) fn new(labeller: Option<Arc<dyn Labeller>>) -> Self {
        let this_computer = || Arc::new(ThisComputer::default()) as Arc<dyn Labeller>;
        Self {
            labeller: labeller.unwrap_or_else(this_computer),
            looks: AtomicU64::new(0),
            model: Mutex::new(None),
            run: Mutex::new(None),
            turn: Mutex::new(()),
        }
    }

    /// The model the newest look found, if it found this computer able.
    fn model(&self) -> Option<Arc<Model>> {
        lock(&self.model).clone()
    }

    /// Note the stop of a run that begins. The run before, if one is on, is stopped.
    fn begin(&self, stop: Stop) {
        if let Some(before) = lock(&self.run).replace(stop) {
            before.stop();
        }
    }

    /// Stop the run on, if one is.
    fn stop(&self) {
        if let Some(on) = lock(&self.run).take() {
            on.stop();
        }
    }
}

/// One run, as a worker carries it out.
struct Job {
    /// The number the state gave it.
    run: u64,
    /// Its place among the reads of labels: one asked for after it overtakes it.
    number: u64,
    /// Which export was open when it was asked for.
    opened: u64,
    wanted: Wanted,
    runs_on: RunsOn,
    /// What Voxa holds of the card it draws on, and that card's name.
    held: u64,
    drawing_on: Option<String>,
    stop: Stop,
    /// The series it labels.
    series: SeriesId,
    /// The volume kept for the series when the run was asked for. It is let go when the run
    /// has said all it will say. The run is over, and says so once, when the store no longer
    /// keeps this very volume for the series.
    volume: Option<Arc<Volume>>,
    /// The model the newest look found.
    model: Option<Arc<Model>>,
}

impl Workers {
    /// Carry out an effect about labelling a scan.
    pub(super) fn labelling(&self, effect: Effect) {
        match effect {
            Effect::LookForLabelling {
                look,
                models,
                runtime,
            } => self.look(look, models, runtime),
            scan @ Effect::LabelScan { .. } => self.label_scan(scan),
            Effect::StopLabelling => self.shared.labelling.stop(),
            _ => {}
        }
    }

    /// Label the volume kept for a series on a worker, in place of any run on. The volume and
    /// the model are taken now, so that what the run is of cannot change under it.
    fn label_scan(&self, scan: Effect) {
        let Effect::LabelScan {
            run,
            series,
            wanted,
            runs_on,
            held,
            keep: _,
            drawing_on,
        } = scan
        else {
            return;
        };
        let shared = &self.shared;
        let stop = Stop::new();
        shared.labelling.begin(stop.clone());
        let job = Job {
            run,
            number: shared.label_reads.fetch_add(1, Ordering::SeqCst) + 1,
            opened: shared.opened.load(Ordering::SeqCst),
            wanted,
            runs_on,
            held,
            drawing_on,
            stop,
            volume: lock(&shared.volumes).get(&series),
            series,
            model: shared.labelling.model(),
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            if !shielded(|| shared.carry_out(&job)) {
                shared.fail(&job, PART_WAY);
            }
        });
    }
}

impl Shared {
    /// Carry out a run: the sizes of the scan, where it fits, and each place in turn.
    fn carry_out(&self, job: &Job) {
        // A new run waits here for a stopped one to be over, and says nothing until it has its
        // turn: a network that was running on a card finishes what it was handed first.
        let _turn = lock(&self.labelling.turn);
        let (Some(volume), Some(model)) = (&job.volume, &job.model) else {
            let why = if job.volume.is_none() {
                NO_VOLUME
            } else {
                NO_MODEL
            };
            return self.fail(job, why);
        };
        let sizes = match sizes(volume) {
            Ok(sizes) => sizes,
            Err(why) => return self.fail(job, &why.to_string()),
        };
        let labeller = self.labelling.labeller.as_ref();
        let room = room_of(labeller, job.drawing_on.as_deref(), job.held);
        let rungs = rungs(sizes.whole, sizes.head, job.wanted, &room, job.runs_on);
        if rungs.is_empty() {
            return self.fail(job, &does_not_fit(sizes, job.wanted, &room, job.runs_on));
        }
        self.climb(job, volume, model, &room, rungs);
    }

    /// Try each rung in turn. A card without room, or that stops answering, gives way to the
    /// next rung, and a card that stopped answering is not tried again. Anything else ends
    /// the run, and so does the last rung's failure.
    fn climb(&self, job: &Job, volume: &Volume, model: &Model, room: &Room, rungs: Vec<Rung>) {
        let opener = self.labelling.labeller.as_ref();
        let (mut gave_way, mut cards_gone, mut why_last) = (None, Vec::new(), RunError::NoRoom);
        for rung in rungs {
            if !self.current(job) {
                return;
            }
            if cards_gone.contains(&rung.device) {
                continue;
            }
            let mut networks = OnRung {
                labeller: opener,
                model,
                rung,
                stop: &job.stop,
            };
            let mut said = |stage| self.stage(job, stage);
            let (cut, lists) = (rung.cut, &model.lists);
            let outcome = label(volume, cut, lists, job.wanted, &mut networks, &mut said);
            let why = match outcome {
                Ok(result) => return self.done(job, model, room, rung, result, gave_way),
                Err(LabelError::Run(why @ (RunError::NoRoom | RunError::DeviceLost))) => why,
                Err(LabelError::Run(RunError::Stopped)) => return,
                Err(LabelError::Run(RunError::Failed(words))) => {
                    let words = logged_words(&words, model);
                    tracing::warn!("a network of the labelling model could not be run: {words}");
                    let device = place(rung.device, room);
                    return self.fail(job, &format!("The model could not be run on {device}."));
                }
                Err(other) => return self.fail(job, &other.to_string()),
            };
            if let Device::Card(_) = rung.device {
                let lost = why == RunError::DeviceLost;
                cards_gone.extend(lost.then_some(rung.device));
                gave_way = Some(if lost { LOST } else { NO_ROOM });
            }
            why_last = why;
        }
        self.fail(job, &why_last.to_string());
    }

    /// A rung gave labels: say on them who made them, where, and anything the labels cannot
    /// show of how; then keep the label set and report it, unless the run is no longer the one
    /// waited for.
    fn done(
        &self,
        job: &Job,
        model: &Model,
        room: &Room,
        rung: Rung,
        labelled: Labelled,
        gave_way: Option<&str>,
    ) {
        let Labelled {
            grid,
            table,
            mut notes,
        } = labelled;
        let on_the_processor = rung.device == Device::Processor;
        notes.extend(gave_way.filter(|_| on_the_processor).map(str::to_owned));
        if !on_the_processor && rung.scores_on == Device::Processor {
            notes.push(SCORES_ON_THE_PROCESSOR.to_owned());
        }
        let device = match rung.device {
            Device::Processor => PROCESSOR.to_owned(),
            Device::Card(card) => card_name(room, card),
        };
        let when = self.labelling.labeller.today();
        let made = made(&table.title, &model.sum, &device, &when, &notes);
        let table = Table {
            made: Some(made),
            ..table
        };
        let set = match label_set(&grid, Some(&table)) {
            Ok(set) => Arc::new(set),
            Err(why) => return self.fail(job, &why.to_string()),
        };
        drop(grid);
        let mut kept = lock(&self.labels);
        if !self.current(job) {
            return;
        }
        *kept = Some(Arc::clone(&set));
        drop(kept);
        self.report(Action::LabelsRead {
            title: set.title.clone(),
            parts: set.parts.clone(),
            verdict: set.verdict.clone(),
            corners: set.corners(),
            names_from: NamesFrom::Table,
            made: set.made.clone(),
            run: Some(job.run),
        });
    }

    /// Whether a run is still the one waited for: not stopped, not overtaken by another run
    /// or read of labels, in the export that was open when it was asked for, and with the
    /// volume it began with still the one the store keeps for its series. A run whose volume
    /// has gone is stopped, and is over at its next step. If it was still the one waited for,
    /// it says once that the scan changed: a state that waits for it is not left waiting.
    fn current(&self, job: &Job) -> bool {
        let waited_for = self.label_reads.load(Ordering::SeqCst) == job.number
            && self.opened.load(Ordering::SeqCst) == job.opened;
        if !job.stop.is_set() && self.volume_gone(job) {
            job.stop.stop();
            if waited_for {
                self.failed(job, CHANGED);
            }
        }
        !job.stop.is_set() && waited_for
    }

    /// Whether the store has given up the very volume a run began with, for its series: it
    /// keeps none now, or another in its place.
    fn volume_gone(&self, job: &Job) -> bool {
        let Some(own) = &job.volume else {
            return false;
        };
        let kept = lock(&self.volumes).get(&job.series);
        !kept.is_some_and(|kept| Arc::ptr_eq(&kept, own))
    }

    /// Say that a run has reached a stage, if it is still the one waited for.
    fn stage(&self, job: &Job, stage: Stage) {
        if self.current(job) {
            let run = job.run;
            let reached = LabellingAction::LabellingStage { run, stage };
            self.report(Action::Labelling(reached));
        }
    }

    /// Say that a run failed, and why, if it is still the one waited for.
    fn fail(&self, job: &Job, why: &str) {
        if self.current(job) {
            self.failed(job, why);
        }
    }

    /// Say that a run failed, and why.
    fn failed(&self, job: &Job, why: &str) {
        let (run, why) = (job.run, why.to_owned());
        let failed = LabellingAction::LabellingFailed { run, why };
        self.report(Action::Labelling(failed));
    }
}

/// The networks of one rung: the scores network on the rung's `scores_on`, every other on its
/// device.
struct OnRung<'a> {
    labeller: &'a dyn Labeller,
    model: &'a Model,
    rung: Rung,
    stop: &'a Stop,
}

impl Networks for OnRung<'_> {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        let device = match role {
            Role::Scores => self.rung.scores_on,
            _ => self.rung.device,
        };
        self.labeller
            .open(self.model, role, device, sizes, self.stop)
    }
}

/// Who made a label set, cut to the lengths a table keeps.
fn made(model: &str, sum: &str, device: &str, when: &str, notes: &[String]) -> Made {
    let line = |text: &str| text.chars().take(MAX_MADE_LINE).collect::<String>();
    let note = |text: &String| text.chars().take(MAX_NOTE).collect::<String>();
    Made {
        by: line(&format!("Voxa {}", env!("CARGO_PKG_VERSION"))),
        model: line(model),
        model_sum: sum.to_owned(),
        device: line(device),
        when: line(when),
        notes: notes.iter().take(MAX_NOTES).map(note).collect(),
    }
}
