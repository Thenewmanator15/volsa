//! Checks the plane views on every series of an export that has a 3D view: draws the three
//! planes through the middle with no window, plainly and smooth; checks that each picture
//! shows something, that the smooth pictures are not the plain ones over again, and that the
//! axial and the coronal picture agree on how wide what they show is along the line they
//! share; and times a picture of each. Prints counts and times.
//!
//! ```text
//! cargo run --release -p volsa-app --example check_planes -- some/export.zip [some/folder]
//! ```
//!
//! Nothing from the files is printed: no name, identifier, date, description or size. Series
//! are called by their place in the export. With a folder given, the pictures are written
//! there: they are pictures of the scan, to be kept as the scan is.

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use volsa_app::{Action, NoticeKind, SceneStatus, Session, png};
use volsa_core::{Grid, SeriesId, Volume, WindowLevel};
use volsa_render::{HeadlessScene, MAX_VOLUME_BYTES, PlaneView, Smoothed};
use volsa_volume::{PlaneKind, central_voxel, pitch_along, plane_of};

/// The side of the pictures drawn, in pixels. It is odd, so that the middle row and column
/// pass through the very point the planes are drawn through.
const SIDE: u32 = 513;
/// A pixel brighter than this is part of what is drawn; the greys of a picture must differ
/// by at least this much for it to show something.
const DRAWN: u8 = 51;
const SPREAD: u8 = 16;
/// How many times a picture is drawn to time it; the middle time is given.
const TIMES: usize = 5;
/// How long the workers may take over one thing before the check gives up.
const PATIENCE: Duration = Duration::from_secs(300);

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let Some(path) = args.next() else {
        eprintln!("Usage: check_planes PATH [FOLDER]   (an export; a folder for pictures)");
        return ExitCode::from(2);
    };
    match check(&path, args.next().as_deref()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn said(why: impl Display) -> String {
    why.to_string()
}

/// Whether indexing has ended: the best series has been opened, or the export could not be.
fn indexed(session: &Session) -> bool {
    let state = session.state();
    let opened = state.viewports.first();
    let failed = |notice: &volsa_app::Notice| notice.kind == NoticeKind::Error;
    opened.is_some_and(|shown| shown.series.is_some()) || state.notices.iter().any(failed)
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

/// Check every series of the export. Whether every check on every series held.
fn check(path: &Path, pictures: Option<&Path>) -> Result<bool, String> {
    if let Some(folder) = pictures {
        std::fs::create_dir_all(folder).map_err(said)?;
    }
    let (sender, reports) = channel();
    // A session with nothing kept between runs.
    let mut session = Session::new(sender, reports, 1 << 30, None);
    let mut gpu = HeadlessScene::new().map_err(said)?;
    session.dispatch(Action::OpenPath(path.to_owned()));
    until(&mut session, indexed)?;
    let Some(index) = &session.state().index else {
        return Err("The export could not be opened.".to_owned());
    };
    let listed = index.series.iter();
    let series: Vec<SeriesId> = listed.map(|series| series.info.id.clone()).collect();
    let (mut checked, mut held) = (0, 0);
    for (place, id) in series.iter().enumerate() {
        let outcome = one_series(&mut session, &mut gpu, id, place + 1, pictures)?;
        checked += usize::from(outcome.is_some());
        held += usize::from(outcome == Some(true));
    }
    let all = series.len();
    println!("{all} series, {checked} with plane views, {held} of those passing every check");
    Ok(held == checked)
}

/// Check the plane views of one series and print a line about it. `None` when it has no 3D
/// view, and so no plane views; otherwise whether every check held.
fn one_series(
    session: &mut Session,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    number: usize,
    pictures: Option<&Path>,
) -> Result<Option<bool>, String> {
    let (viewport, series) = (0, id.clone());
    session.dispatch(Action::SelectSeries { viewport, series });
    until(session, |session| {
        !matches!(session.state().scene.status, SceneStatus::Building { .. })
    })?;
    if session.state().scene.status != SceneStatus::Ready {
        println!("Series {number}: no 3D view, so no plane views");
        return Ok(None);
    }
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not kept.")?;
    // The window is chosen from the slice shown, which arrives a moment after the volume.
    let chosen = |session: &Session| {
        let shown = session.state().viewports.first();
        shown.is_some_and(|shown| !shown.wl_pending)
    };
    until(session, chosen)?;
    let shown = session.state().viewports.first();
    let window = shown.map_or(WindowLevel::new(0.5, 1.0), |shown| shown.wl);
    gpu.upload(&volume).map_err(said)?;
    let plain = planes(gpu, &volume, window)?;
    let started = Instant::now();
    let curve = Smoothed::of(&volume, u32::MAX, MAX_VOLUME_BYTES);
    let worked_out = started.elapsed().as_secs_f64();
    let on_device = gpu.smooth_planes(Some(&curve));
    let smooth = planes(gpu, &volume, window)?;
    gpu.smooth_planes(None);
    if let Some(folder) = pictures {
        save(folder, number, "plain", &plain)?;
        save(folder, number, "smooth", &smooth)?;
    }
    let shown = plain.iter().chain(&smooth).all(shows_something);
    let differs = plain
        .iter()
        .zip(&smooth)
        .all(|(plain, smooth)| plain.rgba != smooth.rgba);
    let (agree, apart) = ruler(&volume, &plain);
    let (plain_ms, smooth_ms) = (slowest(&plain), slowest(&smooth));
    let yes = |held: bool| if held { "yes" } else { "NO" };
    println!(
        "Series {number}: every picture shows something: {}; a plane plain in {plain_ms:.1} ms, \
         smooth in {smooth_ms:.1} ms; the curve worked out in {worked_out:.2} s and drawn from: \
         {}; the axial and coronal widths are {apart:.2} voxels apart: {}",
        yes(shown),
        yes(on_device && differs),
        yes(agree),
    );
    Ok(Some(shown && on_device && differs && agree))
}

/// One plane's picture, and what is needed to measure on it.
struct Drawn {
    kind: PlaneKind,
    rgba: Vec<u8>,
    view: PlaneView,
    /// How long drawing it took, in milliseconds: the middle of several goes.
    ms: f64,
}

/// Draw the three planes through the voxel in the middle of the volume on the device, each
/// with that voxel in the middle of its picture.
fn planes(
    gpu: &mut HeadlessScene,
    volume: &Volume,
    window: WindowLevel,
) -> Result<Vec<Drawn>, String> {
    let placement = volume.voxel_to_patient();
    let through = central_voxel(&placement, volume.dims());
    let mut drawn = Vec::new();
    for kind in PlaneKind::ALL {
        let plane = plane_of(&placement, volume.dims(), kind, through, true);
        let plane = plane.ok_or("The volume's placement cannot be turned round.")?;
        let (middle, _) = plane.of(through);
        let panel = [SIDE as f32; 2];
        let view = PlaneView::showing(plane, &volume.corners(), panel, 1.0, middle);
        // Once to settle, then timed.
        let mut rgba = gpu.render_plane(&view, window, [SIDE; 2]).map_err(said)?;
        let mut times = Vec::new();
        for _ in 0..TIMES {
            let started = Instant::now();
            rgba = gpu.render_plane(&view, window, [SIDE; 2]).map_err(said)?;
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        let ms = times.get(TIMES / 2).copied().unwrap_or_default();
        drawn.push(Drawn {
            kind,
            rgba,
            view,
            ms,
        });
    }
    Ok(drawn)
}

/// The greys of a picture, one for each pixel, row by row.
fn greys(drawn: &Drawn) -> impl Iterator<Item = u8> + '_ {
    drawn.rgba.as_chunks::<4>().0.iter().map(|pixel| pixel[0])
}

/// Whether a picture is more than one flat grey.
fn shows_something(drawn: &Drawn) -> bool {
    let darkest = greys(drawn).min().unwrap_or(0);
    let brightest = greys(drawn).max().unwrap_or(0);
    brightest - darkest >= SPREAD
}

/// The longest any of the pictures took to draw, in milliseconds.
fn slowest(drawn: &[Drawn]) -> f64 {
    drawn.iter().map(|drawn| drawn.ms).fold(0.0, f64::max)
}

/// How wide what is drawn is along the middle row of a picture, in millimetres: from the
/// first pixel that is part of it to the last.
fn width_mm(drawn: &Drawn) -> f64 {
    let row = greys(drawn)
        .skip((SIDE * (SIDE / 2)) as usize)
        .take(SIDE as usize);
    let lit: Vec<usize> = row
        .enumerate()
        .filter(|(_, grey)| *grey > DRAWN)
        .map(|(column, _)| column)
        .collect();
    match (lit.first(), lit.last()) {
        (Some(first), Some(last)) => (last - first + 1) as f64 * drawn.view.mm_per_px,
        _ => 0.0,
    }
}

/// Lay a ruler across what the axial and the coronal picture show along the line the two
/// planes share, which is the middle row of each. Whether the two agree within one voxel and
/// a pixel of each picture, and how many voxels apart they are.
fn ruler(volume: &Volume, plain: &[Drawn]) -> (bool, f64) {
    let of = |kind| plain.iter().find(|drawn| drawn.kind == kind);
    let (Some(axial), Some(coronal)) = (of(PlaneKind::Axial), of(PlaneKind::Coronal)) else {
        return (false, f64::INFINITY);
    };
    let shared = axial.view.plane.across;
    let voxel = pitch_along(&volume.voxel_to_patient(), shared).unwrap_or(1.0);
    let apart = (width_mm(axial) - width_mm(coronal)).abs();
    let allowed = voxel + axial.view.mm_per_px + coronal.view.mm_per_px;
    (apart <= allowed, apart / voxel)
}

/// Write the three pictures of a series into a folder.
fn save(folder: &Path, number: usize, how: &str, drawn: &[Drawn]) -> Result<(), String> {
    for drawn in drawn {
        let name = drawn.kind.name().to_lowercase();
        let path = folder.join(format!("series-{number}-{name}-{how}.png"));
        let file = png(&drawn.rgba, [SIDE; 2]).map_err(said)?;
        std::fs::write(path, file).map_err(said)?;
    }
    Ok(())
}
