//! Labels the largest series of an export that has a 3D view, with no window, on every
//! place Volsa would try, and prints a line for each: how many voxels the networks were given,
//! where, how many seconds the labelling took, how many parts it made and whether the model's
//! own check of its work passed. A place that fails prints its reason, and the next is tried.
//!
//! ```text
//! VOLSA_MODELS=some/model-folder VOLSA_RUNTIME=some/runtime-folder \
//!     cargo run --release -p volsa-app --example check_labelling -- some/export [standard|robust] [no-areas]
//! ```
//!
//! `VOLSA_MODELS` names a model folder and `VOLSA_RUNTIME` the folder that holds the runtime.
//! Left to itself the model is the one Volsa picks for the series, with the cortex divided into
//! its areas, as the button in the Tools tab does. The places are those that
//! `volsa_models::rungs` offers this computer when Volsa is left to choose: the whole scan on
//! each discrete card, then on the processor, then the scan cut to the head on each, then on
//! the processor. A card the system lists twice is tried once. The seconds are counted from
//! the call that labels to the label set made, which is what a worker does between the button
//! and its report.
//!
//! Nothing else is printed, and nothing that says what the scan is: no name, identifier, date
//! or description.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use volsa_app::{Action, NoticeKind, SceneStatus, Session};
use volsa_core::{Grid, SeriesId, Volume};
use volsa_labels::label_set;
use volsa_models::{
    Cut, Device, FreeMemory, Model, Network, Role, Room, RunError, Rung, RunsOn, Runtime, Stop,
    Wanted, budget, cards, free_memory, read_model, rungs, unheld, why_not,
};
use volsa_segment::{ModelChoice, Networks, choose, label, sizes};

/// How long the workers may take over one thing before the check gives up.
const PATIENCE: Duration = Duration::from_secs(300);

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let Some(export) = args.next() else {
        return usage();
    };
    let (mut choice, mut cortex) = (ModelChoice::Automatic, true);
    for word in args {
        match word.to_str() {
            Some("standard") => choice = ModelChoice::Standard,
            Some("robust") => choice = ModelChoice::Robust,
            Some("no-areas") => cortex = false,
            _ => return usage(),
        }
    }
    match check(&export, choice, cortex) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "Usage: check_labelling EXPORT [standard|robust] [no-areas]   \
         (VOLSA_MODELS names a model folder, VOLSA_RUNTIME the runtime's folder)"
    );
    ExitCode::from(2)
}

fn said(why: impl std::fmt::Display) -> String {
    why.to_string()
}

/// The folder a variable names.
fn folder(variable: &str, what: &str) -> Result<PathBuf, String> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{variable} must name {what}."))
}

/// Take in what the workers report until `done` holds.
fn until(session: &mut Session, done: impl Fn(&Session) -> bool) -> Result<(), String> {
    let started = Instant::now();
    while !done(session) {
        if started.elapsed() > PATIENCE {
            return Err("Gave up waiting for the workers.".to_owned());
        }
        session.wait(Duration::from_millis(50));
    }
    Ok(())
}

/// Whether indexing has ended: the best series has been opened, or the export could not be.
fn indexed(session: &Session) -> bool {
    let state = session.state();
    let opened = state.viewports.first();
    let failed = |notice: &volsa_app::Notice| notice.kind == NoticeKind::Error;
    opened.is_some_and(|shown| shown.series.is_some()) || state.notices.iter().any(failed)
}

/// Show a series in the scene and wait until its 3D view is built or has been refused.
fn show(session: &mut Session, id: &SeriesId) -> Result<(), String> {
    let (viewport, series) = (0, id.clone());
    session.dispatch(Action::SelectSeries { viewport, series });
    until(session, |session| {
        !matches!(session.state().scene.status, SceneStatus::Building { .. })
    })
}

/// The scan to label: a series' volume, and how far apart its voxels were as stored.
struct Scan {
    volume: Arc<Volume>,
    spacing_mm: [f64; 3],
}

/// The largest series that has a 3D view, as a scan to label.
fn largest(session: &mut Session, export: &Path) -> Result<Scan, String> {
    session.dispatch(Action::OpenPath(export.to_owned()));
    until(session, indexed)?;
    let Some(index) = &session.state().index else {
        return Err("The export could not be opened.".to_owned());
    };
    let series: Vec<SeriesId> = index.series.iter().map(|one| one.info.id.clone()).collect();
    let mut most: Option<(u64, SeriesId)> = None;
    for id in series {
        show(session, &id)?;
        let voxels = session
            .state()
            .series(&id)
            .map(|one| one.info.voxel_count());
        let ready = session.state().scene.status == SceneStatus::Ready;
        let larger = |voxels: u64| most.as_ref().is_none_or(|(held, _)| voxels > *held);
        if let Some(voxels) = voxels.filter(|&voxels| ready && larger(voxels)) {
            most = Some((voxels, id));
        }
    }
    let (_, id) = most.ok_or("No series of the export has a 3D view.")?;
    show(session, &id)?;
    let volume = session
        .volume(&id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .map(Arc::new)
        .ok_or("The volume is not kept.")?;
    let listed = session.state().series(&id);
    let spacing_mm = listed
        .map(|one| one.info.spacing_mm)
        .ok_or("The series is gone.")?;
    Ok(Scan { volume, spacing_mm })
}

/// What the computer has now, as a run starts.
fn room() -> Room {
    let cards = cards();
    let budgets = cards.iter().map(|card| budget(card.id)).collect();
    let unheld = cards.iter().map(|card| unheld(card.id)).collect();
    let free_memory = match free_memory() {
        FreeMemory::Promised(bytes) | FreeMemory::Physical(bytes) => bytes,
        FreeMemory::Unknown => 0,
    };
    Room {
        cards,
        budgets,
        unheld,
        drawing_on: None,
        held: 0,
        free_memory,
    }
}

/// The networks of one place, each opened where the place puts it.
struct OnRung<'a> {
    runtime: &'a Runtime,
    model: &'a Model,
    rung: Rung,
    stop: Stop,
}

impl Networks for OnRung<'_> {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        let device = match role {
            Role::Scores => self.rung.scores_on,
            _ => self.rung.device,
        };
        self.runtime
            .open(self.model.file(role), device, sizes, &self.stop)
    }
}

/// A place in words: the cut, and where the networks go, and where the scores network does
/// when it is not there too.
fn place(rung: Rung, room: &Room) -> String {
    let cut = match rung.cut {
        Cut::Whole => "the whole scan",
        Cut::Head => "cut to the head",
    };
    let device = |device| match device {
        Device::Processor => "the processor".to_owned(),
        Device::Card(id) => room
            .cards
            .iter()
            .find(|card| card.id == id)
            .map_or_else(|| "a card".to_owned(), |card| card.name.clone()),
    };
    let scores = if rung.scores_on == rung.device {
        String::new()
    } else {
        format!(", the scores on {}", device(rung.scores_on))
    };
    format!("{cut}, on {}{scores}", device(rung.device))
}

/// What labelling on one place gave, said in a line: the seconds, the parts and the check, or
/// why it failed.
fn on_one_place(
    scan: &Scan,
    (runtime, model): (&Runtime, &Model),
    wanted: Wanted,
    rung: Rung,
) -> (bool, String) {
    let mut networks = OnRung {
        runtime,
        model,
        rung,
        stop: Stop::new(),
    };
    let started = Instant::now();
    let outcome = label(
        &scan.volume,
        rung.cut,
        &model.lists,
        wanted,
        &mut networks,
        &mut |_| {},
    );
    let made = outcome.map_err(said).and_then(|made| {
        let set = label_set(&made.grid, Some(&made.table)).map_err(said)?;
        Ok((made.table.verdict, set.parts.len()))
    });
    let seconds = started.elapsed().as_secs_f64();
    match made {
        Ok((verdict, parts)) => {
            let check = match verdict {
                Some(verdict) if verdict.passed => "passed",
                Some(_) => "FAILED",
                None => "none",
            };
            (
                true,
                format!("{seconds:.1} s, {parts} parts, check {check}"),
            )
        }
        Err(why) => (false, format!("{seconds:.1} s, failed: {why}")),
    }
}

/// Label the scan on every place offered. Whether any gave labels.
fn check(export: &Path, choice: ModelChoice, cortex: bool) -> Result<bool, String> {
    let models = folder("VOLSA_MODELS", "a model folder")?;
    let runtime = folder("VOLSA_RUNTIME", "the folder that holds the runtime")?;
    let model = read_model(&models).map_err(said)?;
    let runtime = Runtime::load(&runtime).map_err(said)?;
    // A session with nothing kept between runs.
    let (sender, reports) = channel();
    let mut session = Session::new(sender, reports, 1 << 30, None);
    let scan = largest(&mut session, export)?;
    let wanted = Wanted {
        robust: choose(scan.spacing_mm, choice),
        cortex,
    };
    let counted = sizes(&scan.volume).map_err(said)?;
    let room = room();
    let places = rungs(
        counted.whole,
        counted.head,
        wanted,
        &room,
        RunsOn::Automatic,
    );
    if places.is_empty() {
        return Err(why_not(counted.whole, counted.head, wanted, &room));
    }
    // The system's list can hold one card twice; a place is tried once, by what it is called.
    let mut tried: Vec<String> = Vec::new();
    let mut any = false;
    for rung in places {
        let words = place(rung, &room);
        if tried.contains(&words) {
            continue;
        }
        let voxels = match rung.cut {
            Cut::Whole => counted.whole,
            Cut::Head => counted.head,
        };
        let (gave, result) = on_one_place(&scan, (&runtime, &model), wanted, rung);
        any |= gave;
        println!("{voxels} voxels, {words}: {result}");
        tried.push(words);
    }
    Ok(any)
}
