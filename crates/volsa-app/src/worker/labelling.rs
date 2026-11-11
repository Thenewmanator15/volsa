//! Labelling a scan: looking for the model and the runtime (the module `look`), and running
//! the networks on the volume kept for a series, from one place to try to the next, to a label
//! set that is reported, and held and kept once the state takes it in (the module `taken`). A
//! run says what it says with its number. One that is stopped or overtaken, or whose export
//! has been closed, keeps and says nothing more. One whose volume the store has given up says
//! once that the scan changed, so that no one waits for it. Runs and looks go on threads of
//! their own, and one run has its networks open at a time: the next waits until a stopped one
//! is over, and gives up at once if it is stopped while it waits (the module `turn`).
//!
//! The runtime's own words for a failure go to the log, with every path taken out. Nothing
//! else of a run is logged but what memory it went by, and never a path or a file's name.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use volsa_core::{SeriesId, Volume};
use volsa_models::{Device, Model, Room, RunError, Rung, RunsOn, Stop, Wanted, rungs};
use volsa_segment::{LabelError, Sizes, Stage, label, sizes};

use super::{Shared, Workers, lock, shielded};
use crate::action::{Action, Effect};
use crate::labelling::LabellingAction;

mod labeller;
mod look;
mod room;
mod taken;
mod turn;

pub use labeller::Labeller;
use labeller::{OnRung, ThisComputer};
pub(in crate::worker) use room::room_of;
#[cfg(test)]
use room::{card_named, free_bytes, without_paths};
use room::{does_not_fit, logged_words, passed_over, place};
use taken::Unclaimed;
pub(in crate::worker) use turn::on_its_own;

#[cfg(test)]
#[path = "labelling_full_card_tests.rs"]
mod full_card_tests;
#[cfg(test)]
#[path = "kept_saved_tests.rs"]
mod kept_saved_tests;
#[cfg(test)]
#[path = "kept_tests.rs"]
mod kept_tests;
#[cfg(test)]
#[path = "labelling_lab.rs"]
mod lab;
#[cfg(test)]
#[path = "labelling_look_tests.rs"]
mod look_tests;
#[cfg(test)]
#[path = "plugins_adding_tests.rs"]
mod plugins_adding_tests;
#[cfg(test)]
#[path = "plugins_run_tests.rs"]
mod plugins_run_tests;
#[cfg(test)]
#[path = "labelling_stand_ins.rs"]
mod stand_ins;
#[cfg(test)]
#[path = "labelling_taken_tests.rs"]
mod taken_tests;
#[cfg(test)]
#[path = "labelling_tests.rs"]
mod tests;

/// What is said when a run stops part-way.
const PART_WAY: &str = "Labelling stopped part-way: something went wrong inside it.";
/// What is said when the computer would not give a run a thread to go on.
const NO_THREAD: &str = "Labelling could not start: the computer would not give it a thread to \
                         run on.";
/// What is said when there is no volume, or no model, to run on.
const NO_VOLUME: &str = "The 3D view of this scan is no longer there to label.";
const NO_MODEL: &str = "Volsa has not yet checked whether this computer can label a scan.";
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
    /// Whether a run has its turn: only one run has networks open at a time. Held by a run
    /// from its start to its end (see `turn`).
    busy: Mutex<bool>,
    /// Told when the turn is given back, and when a run is stopped.
    freed: Condvar,
    /// What a run made that the state has not yet taken in (see `taken`).
    made: Mutex<Option<Unclaimed>>,
}

impl Kit {
    /// What runs are done with: the runtime and this computer, or stand-ins.
    pub(in crate::worker) fn labeller(&self) -> &dyn Labeller {
        self.labeller.as_ref()
    }

    /// What labelling keeps, with runs done with `labeller`, or with the runtime and this
    /// computer when there is none.
    pub(super) fn new(labeller: Option<Arc<dyn Labeller>>) -> Self {
        let this_computer = || Arc::new(ThisComputer::default()) as Arc<dyn Labeller>;
        Self {
            labeller: labeller.unwrap_or_else(this_computer),
            looks: AtomicU64::new(0),
            model: Mutex::new(None),
            run: Mutex::new(None),
            busy: Mutex::new(false),
            freed: Condvar::new(),
            made: Mutex::new(None),
        }
    }

    /// The model the newest look found, if it found this computer able.
    fn model(&self) -> Option<Arc<Model>> {
        lock(&self.model).clone()
    }

    /// Note the stop of a run that begins. The run before, if one is on, is stopped, and what
    /// it made that was not taken in is let go.
    fn begin(&self, stop: Stop) {
        if let Some(before) = lock(&self.run).replace(stop) {
            before.stop();
        }
        self.let_go();
        self.wake();
    }

    /// Stop the run on, if one is, and let go of what a run made that was not taken in.
    fn stop(&self) {
        if let Some(on) = lock(&self.run).take() {
            on.stop();
        }
        self.let_go();
        self.wake();
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
    /// What Volsa holds of the card it draws on, and that card's name.
    held: u64,
    drawing_on: Option<String>,
    stop: Stop,
    /// Whether the result is kept for the series.
    keep: bool,
    /// The series it labels.
    series: SeriesId,
    /// The volume kept for the series when the run was asked for. It is let go when the run
    /// has said all it will say, or gives up waiting for its turn. The run is over, and says so
    /// once, when the store no longer keeps this very volume for the series.
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
            Effect::TakeLabels { run } => self.take_labels(run),
            _ => {}
        }
    }

    /// Label the volume kept for a series on a thread of its own, in place of any run on. The
    /// volume and the model are taken now, so that what the run is of cannot change under it.
    fn label_scan(&self, scan: Effect) {
        let Effect::LabelScan {
            run,
            series,
            wanted,
            runs_on,
            held,
            keep,
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
            keep,
            volume: lock(&shared.volumes).get(&series),
            series,
            model: shared.labelling.model(),
        };
        let shared = Arc::clone(&self.shared);
        let started = on_its_own(move || {
            if !shielded(|| shared.carry_out(&job)) {
                shared.fail(&job, PART_WAY);
            }
        });
        if !started {
            let failed = LabellingAction::LabellingFailed {
                run,
                why: NO_THREAD.to_owned(),
            };
            self.shared.report(Action::Labelling(failed));
        }
    }
}

impl Shared {
    /// Carry out a run once it has its turn: the sizes of the scan, where it fits, and each
    /// place in turn. A run stopped before it has its turn gives up, and says nothing.
    fn carry_out(&self, job: &Job) {
        // A network that was running on a card finishes what it was handed before the turn is
        // given back.
        let Some(_turn) = self.labelling.turn(&job.stop) else {
            return;
        };
        if !self.current(job) {
            return;
        }
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
        self.climb(job, volume, sizes, model, &room, rungs);
    }

    /// Try each rung in turn. A card without room, or that stops answering, gives way to the
    /// next rung, and a card that stopped answering is not tried again. Anything else ends
    /// the run, and so does the last rung's failure. A result that no card gave way to says
    /// so if a card was passed over as the run began because programs held most of it.
    fn climb(
        &self,
        job: &Job,
        volume: &Volume,
        sizes: Sizes,
        model: &Model,
        room: &Room,
        rungs: Vec<Rung>,
    ) {
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
                Ok(mut result) => {
                    if gave_way.is_none() {
                        let full = passed_over(job.runs_on, rung, sizes, job.wanted, room);
                        result.notes.extend(full);
                    }
                    return self.done(job, model, room, rung, result, gave_way);
                }
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
                gave_way = Some((rung.device, lost));
            }
            why_last = why;
        }
        self.fail(job, &why_last.to_string());
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
