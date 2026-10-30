//! A lab that follows the worker that labels a scan, with the stand-ins of
//! `labelling_stand_ins.rs`: a made-up volume kept for a series, made-up cards and a made-up
//! model folder. Nothing waits on the clock: a test waits for what the worker says, or for a
//! run to let its volume go, which it does once it has said all it will say. (Only a file that
//! is to appear or to go, which the worker does not announce, is waited for by looking.)

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use voxa_core::HeldVolume;
use voxa_models::RunsOn;
use voxa_segment::Stage;

use super::Labeller;
use super::stand_ins::{PATIENCE, ROBUST, StandIn, head, held, long_head, models};
use crate::action::{Action, Effect};
use crate::cache::FrameCache;
use crate::labelling::{LabellingAction, Ready};
use crate::testing::{FINE_SCAN, THICK_SCAN, held_of, id};
use crate::volume_store::VolumeStore;
use crate::worker::Workers;

/// Workers with stand-ins, the made-up head kept as the 30-slice series' volume, and what the
/// workers say.
pub(super) struct Lab {
    pub(super) workers: Workers,
    reports: Receiver<Action>,
    pub(super) stand_in: Arc<StandIn>,
    volume: Arc<HeldVolume>,
    volumes: Arc<Mutex<VolumeStore>>,
    pub(super) folder: TempDir,
    /// The folder settings are kept in, when the lab made one.
    settings: Option<TempDir>,
}

impl Lab {
    /// The lab, before anything has been looked at.
    pub(super) fn unlooked(stand_in: StandIn) -> Self {
        Self::in_settings(stand_in, None)
    }

    /// The lab with this folder to keep settings in, before anything has been looked at.
    fn in_settings(stand_in: StandIn, settings: Option<PathBuf>) -> Self {
        let (sender, reports) = channel();
        let cache = Arc::new(Mutex::new(FrameCache::new(1 << 20)));
        let volumes = Arc::new(Mutex::new(VolumeStore::default()));
        let stand_in = Arc::new(stand_in);
        let labeller: Arc<dyn Labeller> = Arc::clone(&stand_in) as Arc<dyn Labeller>;
        let kept = Arc::clone(&volumes);
        let workers = Workers::new(sender, cache, kept, settings, Some(labeller));
        let volume = Arc::new(held_of(&head()));
        held(&volumes).put(id(THICK_SCAN), Arc::clone(&volume));
        Self {
            workers,
            reports,
            stand_in,
            volume,
            volumes,
            folder: models(),
            settings: None,
        }
    }

    /// The lab, with a look at the made-up model folder that found this computer able.
    pub(super) fn new(stand_in: StandIn) -> Self {
        let lab = Self::unlooked(stand_in);
        assert_eq!(lab.look(1, Some(lab.folder.path())), Ready::Yes);
        lab
    }

    /// [`new`](Self::new), with the long made-up head (`long_head`) as the 30-slice series'
    /// volume: one whose image is smaller cut to the head than whole.
    pub(super) fn with_a_long_head(stand_in: StandIn) -> Self {
        let mut lab = Self::new(stand_in);
        let volume = Arc::new(held_of(&long_head()));
        held(&lab.volumes).put(id(THICK_SCAN), Arc::clone(&volume));
        lab.volume = volume;
        lab
    }

    /// The lab, looked at, with a folder of its own to keep settings in.
    pub(super) fn keeping(stand_in: StandIn) -> Self {
        let settings = tempfile::tempdir().expect("a temporary folder");
        let mut lab = Self::in_settings(stand_in, Some(settings.path().to_owned()));
        lab.settings = Some(settings);
        assert_eq!(lab.look(1, Some(lab.folder.path())), Ready::Yes);
        lab
    }

    /// The lab, looked at, with this folder to keep settings in.
    pub(super) fn keeping_in(stand_in: StandIn, settings: PathBuf) -> Self {
        let lab = Self::in_settings(stand_in, Some(settings));
        assert_eq!(lab.look(1, Some(lab.folder.path())), Ready::Yes);
        lab
    }

    /// The folder of a lab that made one to keep settings in.
    pub(super) fn settings(&self) -> &Path {
        self.settings.as_ref().expect("the lab has one").path()
    }

    /// Ask for a look in this model folder, and what it found.
    pub(super) fn look(&self, look: u64, models: Option<&Path>) -> Ready {
        let models = models.map(Path::to_path_buf);
        let asked = Effect::LookForLabelling {
            look,
            models,
            runtime: None,
        };
        self.workers.run(asked, None);
        match self.next() {
            Action::Labelling(LabellingAction::LabellingLooked {
                look: seen,
                ready,
                cards,
            }) => {
                assert_eq!((seen, cards), (look, self.stand_in.cards.clone()));
                ready
            }
            other => panic!("not what a look says: {other:?}"),
        }
    }

    /// Ask for a run of the robust networks on the 30-slice series, not to be kept.
    pub(super) fn label(&self, run: u64, runs_on: RunsOn) {
        self.label_keeping(run, runs_on, false);
    }

    /// Ask for a run of the robust networks on the 30-slice series, kept or not.
    pub(super) fn label_keeping(&self, run: u64, runs_on: RunsOn, keep: bool) {
        let asked = Effect::LabelScan {
            run,
            series: id(THICK_SCAN),
            wanted: ROBUST,
            runs_on,
            held: 0,
            keep,
            drawing_on: None,
        };
        self.workers.run(asked, None);
    }

    pub(super) fn next(&self) -> Action {
        let said = self.reports.recv_timeout(PATIENCE);
        said.expect("the worker says something")
    }

    /// Everything said up to the end of a run: its labels, or why it failed.
    pub(super) fn until_the_end(&self) -> Vec<Action> {
        let mut said = vec![self.next()];
        while !said.last().is_some_and(is_an_end) {
            said.push(self.next());
        }
        said
    }

    /// [`until_the_end`](Self::until_the_end), and the labels a run brought back taken in, as
    /// the state asks once it has taken them.
    pub(super) fn until_taken(&self) -> Vec<Action> {
        let said = self.until_the_end();
        if let Some(Action::LabelsRead { run: Some(run), .. }) = said.last() {
            self.workers.run(Effect::TakeLabels { run: *run }, None);
        }
        said
    }

    /// How many hold the made-up head: the lab, the store while it keeps this very volume, and
    /// each run that has not let it go.
    pub(super) fn holders(&self) -> usize {
        Arc::strong_count(&self.volume)
    }

    /// Wait until no run holds the head: every run has said all it will say. The lab holds it
    /// once, and the store once more while it still keeps this very volume.
    pub(super) fn settled(&self) {
        let started = Instant::now();
        loop {
            let kept = held(&self.volumes).get(&id(THICK_SCAN));
            let kept = kept.is_some_and(|kept| Arc::ptr_eq(&kept, &self.volume));
            if Arc::strong_count(&self.volume) <= 1 + usize::from(kept) {
                return;
            }
            assert!(started.elapsed() < PATIENCE, "a run never ended");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Wait until something the worker does without announcing it, such as writing a file, is
    /// done, by looking.
    pub(super) fn eventually(what: &str, done: impl Fn() -> bool) {
        let started = Instant::now();
        while !done() {
            assert!(started.elapsed() < PATIENCE, "still waiting for {what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Another series' volume takes the place of the one kept, as when the person chooses
    /// another series for the scene.
    pub(super) fn another_series(&self) {
        held(&self.volumes).put(id(FINE_SCAN), Arc::new(held_of(&head())));
    }

    /// The volume of the same series is made again, and takes the place of the one kept.
    pub(super) fn the_volume_made_again(&self) {
        held(&self.volumes).put(id(THICK_SCAN), Arc::new(held_of(&head())));
    }

    /// What has been said and not yet read.
    pub(super) fn said(&self) -> Vec<Action> {
        self.reports.try_iter().collect()
    }
}

fn is_an_end(said: &Action) -> bool {
    matches!(
        said,
        Action::LabelsRead { .. } | Action::Labelling(LabellingAction::LabellingFailed { .. })
    )
}

/// The stages a run said, of those said.
pub(super) fn stages(said: &[Action], of: u64) -> Vec<Stage> {
    let stage = |said: &Action| match said {
        Action::Labelling(LabellingAction::LabellingStage { run, stage }) if *run == of => {
            Some(*stage)
        }
        _ => None,
    };
    said.iter().filter_map(stage).collect()
}

/// Who made the labels a run brought back, and the run's number, from the last thing said.
pub(super) fn made_by_run(said: &[Action]) -> (voxa_labels::Made, Option<u64>) {
    match said.last() {
        Some(Action::LabelsRead { made, run, .. }) => (made.clone().expect("Voxa made them"), *run),
        other => panic!("no labels: {other:?}"),
    }
}

/// Why a run failed, and the run's number, from the last thing said.
pub(super) fn failed_run(said: &[Action]) -> (u64, String) {
    match said.last() {
        Some(Action::Labelling(LabellingAction::LabellingFailed { run, why })) => {
            (*run, why.clone())
        }
        other => panic!("no failure: {other:?}"),
    }
}
