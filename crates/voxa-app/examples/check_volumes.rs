//! Checks that Voxa can show every series of an export in 3D, or says why it cannot: builds
//! each volume as the application does, draws it in each look, cuts it open, draws the slice
//! in the scene, and saves a picture and a turntable. Prints counts, sizes and times.
//!
//! ```text
//! cargo run --release -p voxa-app --example check_volumes -- some/export.zip some/folder
//! ```
//!
//! Nothing about the patient is printed: no name, no identifier, no date, no description.
//! Series are called by their place in the export. The pictures written to the folder given
//! are pictures of the scan: keep them as you would the scan.

use std::fmt::{Display, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use voxa_app::{Action, NoticeKind, SUGGESTED_NAME, SceneStatus, Session, gif, png};
use voxa_core::{Grid, SeriesId, WindowLevel};
use voxa_render::{CutPlane, HeadlessScene, Look, SceneParams, SlicePlane, turntable};

/// The size of the pictures drawn: a large viewport.
const PICTURE: [u32; 2] = [1280, 800];
/// The size of a turntable's pictures.
const SMALL: [u32; 2] = [512, 320];
/// How long the workers may take over one thing before the check gives up.
const PATIENCE: Duration = Duration::from_secs(300);
const LOOKS: [(&str, Look); 3] = [
    ("Surface", Look::Surface),
    ("VR", Look::Vr),
    ("MIP", Look::Mip),
];

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let (Some(path), Some(pictures)) = (args.next(), args.next()) else {
        eprintln!("Usage: check_volumes PATH FOLDER   (an export, and a folder for pictures)");
        return ExitCode::from(2);
    };
    match check(&path, &pictures) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn said(why: impl Display) -> String {
    why.to_string()
}

fn viewport() -> [f32; 2] {
    PICTURE.map(|side| side as f32)
}

/// A session with nothing kept between runs.
fn session() -> Session {
    let (sender, reports) = channel();
    Session::new(sender, reports, 1 << 30, None)
}

/// Whether indexing has ended: the best series has been opened, or the export could not be.
fn indexed(session: &Session) -> bool {
    let state = session.state();
    let opened = state
        .viewports
        .first()
        .is_some_and(|shown| shown.series.is_some());
    opened
        || state
            .notices
            .iter()
            .any(|notice| notice.kind == NoticeKind::Error)
}

fn ready(session: &Session) -> bool {
    session.state().scene.status == SceneStatus::Ready
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

fn select(session: &mut Session, id: &SeriesId) {
    let (viewport, series) = (0, id.clone());
    session.dispatch(Action::SelectSeries { viewport, series });
}

fn check(path: &Path, pictures: &Path) -> Result<(), String> {
    std::fs::create_dir_all(pictures).map_err(said)?;
    let mut session = session();
    let mut gpu = HeadlessScene::new().map_err(said)?;
    session.dispatch(Action::OpenPath(path.to_owned()));
    until(&mut session, indexed)?;
    let series: Vec<SeriesId> = match &session.state().index {
        Some(index) => index
            .series
            .iter()
            .map(|series| series.info.id.clone())
            .collect(),
        None => return Err("The export could not be opened.".to_owned()),
    };
    println!("{} series", series.len());
    let mut largest: Option<(u64, usize)> = None;
    for (place, id) in series.iter().enumerate() {
        let voxels = one_series(&mut session, &mut gpu, id, place + 1, pictures)?;
        if voxels.is_some_and(|voxels| largest.is_none_or(|(most, _)| voxels > most)) {
            largest = voxels.map(|voxels| (voxels, place));
        }
    }
    let Some((_, place)) = largest else {
        return Ok(());
    };
    let id = series.get(place).ok_or("No such series.")?;
    timed_from_opening(path, &mut gpu, id, place + 1)?;
    // Back to that series in this session, so that its scene is the one to cut and save.
    select(&mut session, id);
    until(&mut session, ready)?;
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not in the store.")?;
    gpu.upload(&volume).map_err(said)?;
    session.dispatch(Action::SceneResized {
        viewport: viewport(),
    });
    extras(&session, &mut gpu, id, pictures)
}

/// Show one series in 3D and print a line about it: how it was built and drawn, or why it
/// has no 3D view. Gives its number of voxels when it was built.
fn one_series(
    session: &mut Session,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    number: usize,
    pictures: &Path,
) -> Result<Option<u64>, String> {
    let started = Instant::now();
    select(session, id);
    until(session, |session| {
        !matches!(session.state().scene.status, SceneStatus::Building { .. })
    })?;
    let Some(info) = session.state().series(id).map(|listed| listed.info.clone()) else {
        return Ok(None);
    };
    let [across, down, between] = info.spacing_mm;
    let size = format!(
        "{} x {} x {}, {across:.2} x {down:.2} x {between:.2} mm",
        info.cols, info.rows, info.slices
    );
    match session.state().scene.status.clone() {
        SceneStatus::Unavailable(why) => {
            println!("Series {number}: {size}: no 3D view: {}", why.join(" "));
            Ok(None)
        }
        SceneStatus::Ready => {
            let built = started.elapsed().as_secs_f64();
            let drawn = draw(session, gpu, id, pictures, number)?;
            println!("Series {number}: {size}: built in {built:.2} s; {drawn}");
            Ok(Some(info.voxel_count()))
        }
        other => {
            println!("Series {number}: {size}: {other:?}");
            Ok(None)
        }
    }
}

/// What the scene of the series now shown is drawn from, in one look, as the window would.
fn scene_of(session: &Session, look: Look) -> SceneParams {
    let state = session.state();
    let shown = state.viewports.first();
    let window = shown.map_or(WindowLevel::new(0.5, 1.0), |shown| shown.wl);
    let (camera, cuts) = (state.scene.camera, state.scene.cuts.clone());
    SceneParams {
        camera,
        window,
        look,
        cuts,
        slice: None,
        quality: 1.0,
        skip_empty: true,
    }
}

/// Upload the volume of the series shown and draw it in each look; how long each took and how
/// much of the picture is drawn on.
fn draw(
    session: &mut Session,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    pictures: &Path,
    number: usize,
) -> Result<String, String> {
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not in the store.")?;
    // The window is chosen from the slice shown, which arrives a moment after the volume.
    let chosen = |session: &Session| {
        session
            .state()
            .viewports
            .first()
            .is_some_and(|shown| !shown.wl_pending)
    };
    until(session, chosen)?;
    session.dispatch(Action::SceneResized {
        viewport: viewport(),
    });
    let started = Instant::now();
    let halved = gpu.upload(&volume).map_err(said)?;
    let took = started.elapsed().as_secs_f64();
    let mut line = format!("on the device in {took:.2} s, halved {halved} times");
    for (name, look) in LOOKS {
        let started = Instant::now();
        let rgba = gpu
            .render(&scene_of(session, look), PICTURE)
            .map_err(said)?;
        let took = started.elapsed().as_secs_f64() * 1000.0;
        let lit = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 8)
            .count();
        let share = 100.0 * lit as f64 / f64::from(PICTURE[0] * PICTURE[1]);
        // Writing to a string cannot fail.
        let _ = write!(line, "; {name} {took:.0} ms, {share:.0}% drawn on");
        let path = pictures.join(format!("series-{number}-{}.png", name.to_lowercase()));
        std::fs::write(path, png(&rgba, PICTURE).map_err(said)?).map_err(said)?;
    }
    Ok(line)
}

/// How long it takes from opening the export to a full-quality picture of one series,
/// everything included: indexing, decoding, stacking, putting it on the graphics device and
/// drawing. Done in a session of its own, so that nothing is already decoded.
fn timed_from_opening(
    path: &Path,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    number: usize,
) -> Result<(), String> {
    let mut session = session();
    let started = Instant::now();
    session.dispatch(Action::OpenPath(path.to_owned()));
    until(&mut session, indexed)?;
    let chosen = Instant::now();
    select(&mut session, id);
    until(&mut session, ready)?;
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not in the store.")?;
    gpu.upload(&volume).map_err(said)?;
    gpu.render(&scene_of(&session, Look::Surface), PICTURE)
        .map_err(said)?;
    let (whole, after) = (
        started.elapsed().as_secs_f64(),
        chosen.elapsed().as_secs_f64(),
    );
    println!(
        "Largest is series {number}: from opening the export to a full picture of it \
         {whole:.2} s, of which {after:.2} s after choosing it"
    );
    Ok(())
}

/// On the series shown: a cut, the slice drawn in the scene, a saved picture and a saved
/// turntable, each read back.
fn extras(
    session: &Session,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    pictures: &Path,
) -> Result<(), String> {
    let listed = session
        .state()
        .series(id)
        .ok_or("No such series.")?
        .info
        .clone();
    let placement = listed
        .voxel_to_patient
        .ok_or("The series has no placement.")?;
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not in the store.")?;
    let plain = scene_of(session, Look::Surface);
    // Turned, and fitted again so that the whole of it is still in the picture.
    let camera = plain.camera.orbited([70.0, 35.0]);
    let camera = camera.refitted(&volume.corners(), viewport());
    let turned = SceneParams { camera, ..plain };
    let before = gpu.render(&turned, PICTURE).map_err(said)?;
    let cut = SceneParams {
        cuts: vec![CutPlane::facing(&turned.camera)],
        ..turned.clone()
    };
    let slice = SlicePlane::of_slice(&placement, [listed.cols, listed.rows], listed.slices / 2);
    let with_slice = SceneParams {
        slice,
        look: Look::Vr,
        ..turned.clone()
    };
    for (name, scene) in [("cut", &cut), ("slice", &with_slice)] {
        let rgba = gpu.render(scene, PICTURE).map_err(said)?;
        let file = png(&rgba, PICTURE).map_err(said)?;
        std::fs::write(pictures.join(format!("{SUGGESTED_NAME}-{name}.png")), &file)
            .map_err(said)?;
        let read = image::load_from_memory(&file).map_err(said)?;
        println!(
            "With a {name}: the picture changes: {}; saved and read back at {} x {}",
            rgba != before,
            read.width(),
            read.height()
        );
    }
    spin(gpu, &turned, pictures)
}

/// Draw a turntable of a scene, save it as a GIF and read it back.
fn spin(gpu: &mut HeadlessScene, scene: &SceneParams, pictures: &Path) -> Result<(), String> {
    // The same scene laid out in a picture two fifths the size.
    let smaller = scene.camera.zoomed_about(0.4, [0.0, 0.0]);
    let started = Instant::now();
    let mut frames = Vec::new();
    for camera in turntable(&smaller, 72) {
        let scene = SceneParams {
            camera,
            ..scene.clone()
        };
        frames.push(gpu.render(&scene, SMALL).map_err(said)?);
    }
    let drawn = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let file = gif(&frames, SMALL, 6.0).map_err(said)?;
    let joined = started.elapsed().as_secs_f64();
    std::fs::write(pictures.join(format!("{SUGGESTED_NAME}.gif")), &file).map_err(said)?;
    let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(file)).map_err(said)?;
    let read = image::AnimationDecoder::into_frames(decoder).count();
    println!(
        "Turntable: 72 pictures drawn in {drawn:.1} s, put together in {joined:.1} s, saved and \
         read back as {read} frames"
    );
    Ok(())
}
