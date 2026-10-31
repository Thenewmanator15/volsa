//! Measures how Voxa opens one large series, the way the window does but with no window:
//! from opening the export to the index, to the first slice, to the volume built, to the
//! volume on the graphics device, and then how long a picture of it takes to draw. Prints
//! one line for each step, with the time since the program started, so that a measurement
//! of the program's memory taken from outside can be laid against the steps.
//!
//! ```text
//! cargo run --release -p voxa-app --example measure_big -- some/export [nodraw]
//! ```
//!
//! The largest series of the export is the one measured. Nothing about the patient is
//! printed; use it with made-up studies from `write_phantom`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use voxa_app::{Action, NoticeKind, SceneStatus, Session};
use voxa_core::{Grid, SeriesId, WindowLevel};
use voxa_render::{CardRoom, HeadlessScene, Look, SceneParams};

/// How long the workers may take over one step before the measurement gives up.
const PATIENCE: Duration = Duration::from_secs(600);
/// The memory budget, as the window's is by default; `VOXA_BUDGET_MIB` sets another.
const BUDGET_BYTES: usize = 2 << 30;
/// How many pictures are timed in each look and size, after one that is not.
const TIMES: usize = 5;
const SIZES: [[u32; 2]; 2] = [[1280, 800], [3840, 2160]];

fn main() -> ExitCode {
    let started = Instant::now();
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let (path, draw) = match args.as_slice() {
        [path] => (path, true),
        [path, word] if word.as_os_str() == "nodraw" => (path, false),
        _ => {
            eprintln!("Usage: measure_big PATH [nodraw]");
            return ExitCode::from(2);
        }
    };
    match measure(path, draw, started) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// Print one step: its name, the time since the program started, and what else is said.
fn step(started: Instant, name: &str, more: &str) {
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    println!("{ms:>9.0} ms  {name:<12} {more}");
}

fn until(session: &mut Session, done: impl Fn(&Session) -> bool) -> Result<(), String> {
    let began = Instant::now();
    while !done(session) {
        if began.elapsed() > PATIENCE {
            return Err("Gave up waiting for the workers.".to_owned());
        }
        session.wait(Duration::from_millis(2));
    }
    Ok(())
}

/// Whether indexing has ended: a series has been opened, or the export could not be.
fn indexed(session: &Session) -> bool {
    let state = session.state();
    let opened = state.viewports.first().is_some_and(|v| v.series.is_some());
    opened || state.notices.iter().any(|n| n.kind == NoticeKind::Error)
}

/// The series with the most voxels.
fn largest(session: &Session) -> Option<SeriesId> {
    let index = session.state().index.as_ref()?;
    let most = index.series.iter().max_by_key(|s| s.info.voxel_count())?;
    Some(most.info.id.clone())
}

fn measure(path: &Path, draw: bool, started: Instant) -> Result<(), String> {
    let mut gpu = HeadlessScene::new().map_err(|why| why.to_string())?;
    let (device, queue) = gpu.device();
    let side = device.limits().max_texture_dimension_3d;
    step(
        started,
        "device",
        &format!("{} (3D side {side})", gpu.device_name()),
    );
    let (sender, reports) = channel();
    let asked = std::env::var("VOXA_BUDGET_MIB").ok();
    let budget = asked
        .and_then(|mib| mib.trim().parse::<usize>().ok())
        .map_or(BUDGET_BYTES, |mib| mib << 20);
    // A folder of its own, as the window has, for slabs past the budget.
    let local = std::env::temp_dir().join("voxa-measure-big");
    let mut session = Session::new(sender, reports, budget, Some(local));
    // As the window does: told what the device can hold; never given the device itself.
    let workers = session.workers();
    workers.set_card_room(CardRoom::of(&device, None));
    workers.leave_to_the_device(gpu.makes_curves());
    drop(queue);
    let id = open(&mut session, path, started)?;
    let (card, halved) = session.for_card(&id).ok_or("Nothing is kept.")?;
    let dims = card.dims();
    let placing = Instant::now();
    gpu.upload_held(&card).map_err(|why| why.to_string())?;
    gpu.settle();
    drop(card);
    // Windows counts what a texture holds in the program's own memory, even on a card with
    // memory of its own; said here so that it can be told apart from what Voxa holds.
    let card_mib = (dims.iter().map(|&side| u64::from(side)).product::<u64>() * 2) >> 20;
    let step_ms = gpu.longest_upload_step().as_secs_f64() * 1000.0;
    let said = format!(
        "{dims:?} on the device, halved {halved} times; {:.0} ms to hand over, longest step \
         {step_ms:.1} ms; card {card_mib} MiB",
        ms(placing)
    );
    step(started, "on device", &said);
    let held = session.volume(&id).map_or(0, |volume| volume.bytes_held()) >> 20;
    let over = session.workers().over_budget() >> 20;
    let budgeted = format!("{held} MiB of the volume in memory; {over} MiB past the budget");
    step(started, "budget", &budgeted);
    if draw {
        draw_all(&session, &mut gpu, started)?;
    }
    step(started, "done", "");
    Ok(())
}

/// Open the export, show its largest series and build its volume, printing each step.
/// Gives the series.
fn open(session: &mut Session, path: &Path, started: Instant) -> Result<SeriesId, String> {
    let opened = Instant::now();
    session.dispatch(Action::OpenPath(path.to_owned()));
    until(session, indexed)?;
    let id = largest(session).ok_or("The export holds no series.")?;
    let info = session.state().series(&id).map(|s| s.info.clone());
    let info = info.ok_or("The series is not listed.")?;
    let shape = format!("{} x {} x {}", info.cols, info.rows, info.slices);
    step(started, "indexed", &format!("largest series {shape}"));
    let shown = session
        .state()
        .viewports
        .first()
        .and_then(|v| v.series.clone());
    if shown.as_ref() != Some(&id) {
        session.dispatch(Action::SelectSeries {
            viewport: 0,
            series: id.clone(),
        });
    }
    let on_show = |s: &Session| {
        s.state()
            .viewports
            .first()
            .is_some_and(|v| v.series.as_ref() == Some(&id) && !v.wl_pending)
    };
    until(session, on_show)?;
    step(
        started,
        "first slice",
        &format!("{:.0} ms after opening", ms(opened)),
    );
    // A long series is shown as a preview first, while the rest of it is decoded.
    let previewed_or_built = |s: &Session| {
        let scene = &s.state().scene;
        scene.preview || !matches!(scene.status, SceneStatus::Building { .. })
    };
    until(session, previewed_or_built)?;
    if session.state().scene.preview {
        let said = format!("{:.0} ms after opening", ms(opened));
        step(started, "preview", &said);
    }
    let built = |s: &Session| !matches!(s.state().scene.status, SceneStatus::Building { .. });
    until(session, built)?;
    let status = session.state().scene.status.clone();
    if status != SceneStatus::Ready {
        return Err(format!("No 3D view: {status:?}"));
    }
    step(
        started,
        "volume",
        &format!("{:.0} ms after opening", ms(opened)),
    );
    Ok(id)
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// Draw the volume in VR and MIP at each size, and print the middle time of each.
fn draw_all(session: &Session, gpu: &mut HeadlessScene, started: Instant) -> Result<(), String> {
    let state = session.state();
    let window = state
        .viewports
        .first()
        .map_or(WindowLevel::new(0.5, 1.0), |v| v.wl);
    for look in [Look::Vr, Look::Mip] {
        for size in SIZES {
            let params = SceneParams {
                camera: state.scene.camera,
                window,
                look,
                cuts: state.scene.cuts.clone(),
                slice: None,
                quality: 1.0,
                skip_empty: true,
            };
            gpu.render(&params, size).map_err(|why| why.to_string())?;
            let mut times: Vec<f64> = (0..TIMES)
                .map(|_| {
                    let began = Instant::now();
                    gpu.render(&params, size).map(|_| ms(began))
                })
                .collect::<Result<_, _>>()
                .map_err(|why| why.to_string())?;
            times.sort_by(f64::total_cmp);
            let middle = times.get(TIMES / 2).copied().unwrap_or_default();
            let said = format!("{look:?} {}x{}: {middle:.1} ms a picture", size[0], size[1]);
            step(started, "drawn", &said);
        }
    }
    Ok(())
}
