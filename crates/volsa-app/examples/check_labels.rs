//! Checks a file of labels against an export, with no window. On every series with a 3D
//! view that the labels reach it opens them as the window would, draws the three planes
//! through a labelled point with and without the labels over them, and draws the labelled
//! parts as solids. It checks that the labels change each plane's picture, that the solids
//! draw something and that hiding every part leaves nothing, and, once, that for every name
//! with a left and a right the left part lies towards the patient's left of the right.
//!
//! ```text
//! cargo run --release -p volsa-app --example check_labels -- some/export some/labels.nii.gz [some/folder]
//! ```
//!
//! Nothing from the files is printed: no name of a part, a group or a series, and no
//! identifier, date, description or size. Series are called by their place in the export.
//! With a folder given, the pictures are written there: they are pictures of the scan, to
//! be kept as the scan is.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use volsa_app::{
    Action, LabelsStatus, NOWHERE_NEAR, NoticeKind, SceneStatus, Session, png, table_beside,
};
use volsa_core::{Grid, Patient, Point, SeriesId, Volume, WindowLevel};
use volsa_labels::{LabelSet, pairs};
use volsa_render::{HeadlessScene, Look, Overlay, PartLook, PlaneView, SceneParams};
use volsa_volume::{PlaneKind, plane_of};

/// The side of the pictures drawn, in pixels.
const SIDE: u32 = 513;
/// How many times a picture is drawn to time it; the middle time is given.
const TIMES: usize = 5;
/// How long the workers may take over one thing before the check gives up.
const PATIENCE: Duration = Duration::from_secs(300);
/// About how many voxels of the labels are looked at to find where they lie; of a larger
/// volume, every few.
const LOOKED_AT: usize = 4_000_000;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let (Some(export), Some(labels)) = (args.next(), args.next()) else {
        eprintln!("Usage: check_labels EXPORT LABELS [FOLDER]   (a folder for pictures)");
        return ExitCode::from(2);
    };
    match check(&export, &labels, args.next().as_deref()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn said(why: impl std::fmt::Display) -> String {
    why.to_string()
}

fn yes(held: bool) -> &'static str {
    if held { "yes" } else { "NO" }
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

/// What became of one series.
enum Outcome {
    /// It has no 3D view, so labels cannot be put on it.
    NoScene,
    /// The labels lie nowhere near it.
    NotReached,
    /// The labels are on it: whether every check held, and the labels as read.
    Checked(bool, Arc<LabelSet>),
}

/// Check the labels on every series of the export. Whether every check held.
fn check(export: &Path, labels: &Path, pictures: Option<&Path>) -> Result<bool, String> {
    if let Some(folder) = pictures {
        std::fs::create_dir_all(folder).map_err(said)?;
    }
    let (sender, reports) = channel();
    // A session with nothing kept between runs.
    let mut session = Session::new(sender, reports, 1 << 30, None);
    let mut gpu = HeadlessScene::new().map_err(said)?;
    session.dispatch(Action::OpenPath(export.to_owned()));
    until(&mut session, indexed)?;
    let Some(index) = &session.state().index else {
        return Err("The export could not be opened.".to_owned());
    };
    let listed = index.series.iter();
    let series: Vec<SeriesId> = listed.map(|series| series.info.id.clone()).collect();
    let (mut on, mut held, mut read) = (0, 0, None);
    for (place, id) in series.iter().enumerate() {
        let files = (labels, pictures);
        match one_series(&mut session, &mut gpu, id, place + 1, files)? {
            Outcome::Checked(passed, set) => {
                on += 1;
                held += usize::from(passed);
                read = Some(set);
            }
            Outcome::NoScene | Outcome::NotReached => {}
        }
    }
    let sided = read.as_deref().is_none_or(sides);
    let all = series.len();
    println!("{all} series, {on} with the labels on them, {held} of those passing every check");
    Ok(on > 0 && held == on && sided)
}

/// Say how many parts the labels have, and check that each left part lies towards the
/// patient's left of the right part of the same name. Whether that held for all of them.
fn sides(set: &LabelSet) -> bool {
    let middles = middles(set);
    let middle = |part: Option<usize>| part.and_then(|part| middles.get(part).copied().flatten());
    let both = pairs(&set.parts);
    let sided: Vec<(Point<Patient>, Point<Patient>)> = both
        .iter()
        .filter_map(|pair| middle(pair.left).zip(middle(pair.right)))
        .collect();
    // Towards the patient's left is the way x rises.
    let held = sided
        .iter()
        .filter(|(left, right)| left.x > right.x)
        .count();
    let mut groups: Vec<&str> = set.parts.iter().map(|part| part.group.as_str()).collect();
    groups.sort_unstable();
    groups.dedup();
    println!(
        "Parts: {}; groups: {}; names with a left and a right: {}; the left lies towards the \
         patient's left of the right in {held} of them: {}",
        set.parts.len(),
        groups.len(),
        sided.len(),
        yes(held == sided.len()),
    );
    held == sided.len()
}

/// Every so many voxels of the labels, each with its place in the patient and its part.
fn sampled(set: &LabelSet) -> impl Iterator<Item = (Point<Patient>, usize)> + '_ {
    let [cols, rows, _] = set.dims.map(|count| count.max(1) as usize);
    // An odd step, so that it does not fall into line with the columns.
    let step = (set.numbers.len() / LOOKED_AT).max(1) | 1;
    let labelled = set.numbers.iter().enumerate().step_by(step);
    labelled.filter_map(move |(index, number)| {
        let part = usize::from(*number).checked_sub(1)?;
        let (col, row, slice) = (index % cols, index / cols % rows, index / (cols * rows));
        let voxel = Point::new(col as f64, row as f64, slice as f64);
        Some((set.placement.apply(voxel), part))
    })
}

/// The middle of each part in the patient: the mean of its voxels looked at.
fn middles(set: &LabelSet) -> Vec<Option<Point<Patient>>> {
    let mut sums = vec![([0.0_f64; 3], 0.0_f64); set.parts.len()];
    for (at, part) in sampled(set) {
        if let Some((sum, count)) = sums.get_mut(part) {
            *sum = [sum[0] + at.x, sum[1] + at.y, sum[2] + at.z];
            *count += 1.0;
        }
    }
    let mean = |(sum, count): ([f64; 3], f64)| {
        let [x, y, z] = sum.map(|part| part / count);
        (count > 0.0).then(|| Point::new(x, y, z))
    };
    sums.into_iter().map(mean).collect()
}

/// Whether a point of the patient is inside a volume's box.
fn inside(volume: &Volume, at: Point<Patient>) -> bool {
    let Some(to_voxel) = volume.voxel_to_patient().inverse() else {
        return false;
    };
    let voxel = to_voxel.apply(at);
    let within = |place: f64, count: u32| place >= -0.5 && place <= f64::from(count) - 0.5;
    let [cols, rows, slices] = volume.dims();
    within(voxel.x, cols) && within(voxel.y, rows) && within(voxel.z, slices)
}

/// The share of what is labelled that lies inside a volume, and a labelled point in it: the
/// one nearest the middle of those that do. `None` when nothing labelled lies inside.
fn reach(set: &LabelSet, volume: &Volume) -> Option<(f64, Point<Patient>)> {
    let (mut all, mut within) = (0.0, Vec::new());
    for (at, _) in sampled(set) {
        all += 1.0;
        if inside(volume, at) {
            within.push(at);
        }
    }
    let count = within.len() as f64;
    let sum = within.iter().fold([0.0; 3], |sum, at| {
        [sum[0] + at.x, sum[1] + at.y, sum[2] + at.z]
    });
    let [x, y, z] = sum.map(|part| part / count);
    let away = |at: &Point<Patient>| (at.x - x).powi(2) + (at.y - y).powi(2) + (at.z - z).powi(2);
    let nearest = within
        .iter()
        .min_by(|a, b| away(a).total_cmp(&away(b)))
        .copied()?;
    Some((count / all, nearest))
}

/// Open the labels on one series, check them there and print a line about it.
fn one_series(
    session: &mut Session,
    gpu: &mut HeadlessScene,
    id: &SeriesId,
    number: usize,
    (labels, pictures): (&Path, Option<&Path>),
) -> Result<Outcome, String> {
    let (viewport, series) = (0, id.clone());
    session.dispatch(Action::CloseLabels);
    session.dispatch(Action::SelectSeries { viewport, series });
    let built = |session: &Session| {
        let shown = session.state().viewports.first();
        let building = matches!(session.state().scene.status, SceneStatus::Building { .. });
        !building && shown.is_some_and(|shown| !shown.wl_pending)
    };
    until(session, built)?;
    if session.state().scene.status != SceneStatus::Ready {
        println!("Series {number}: no 3D view, so no labels on it");
        return Ok(Outcome::NoScene);
    }
    let table = table_beside(labels, Path::is_file);
    let labels = labels.to_owned();
    session.dispatch(Action::OpenLabels { labels, table });
    until(session, |session| {
        session.state().labels.status != LabelsStatus::Reading
    })?;
    match &session.state().labels.status {
        LabelsStatus::Ready => {}
        LabelsStatus::Failed(why) if why == NOWHERE_NEAR => {
            println!("Series {number}: the labels lie nowhere near it");
            return Ok(Outcome::NotReached);
        }
        LabelsStatus::Failed(why) => return Err(why.clone()),
        LabelsStatus::None | LabelsStatus::Reading => {
            return Err("The labels were not opened.".to_owned());
        }
    }
    let volume = session
        .volume(id)
        .and_then(|held| held.to_volume(0..held.dims()[2]))
        .ok_or("The volume is not kept.")?;
    let set = session.labels().ok_or("The labels are not kept.")?;
    let Some((share, through)) = reach(&set, &volume) else {
        println!("Series {number}: nothing labelled lies inside it");
        return Ok(Outcome::NotReached);
    };
    gpu.upload(&volume).map_err(said)?;
    let drawn = draw(session, gpu, (&volume, &set), through)?;
    if let Some(folder) = pictures {
        save(folder, number, &drawn)?;
    }
    println!(
        "Series {number}: {:.0}% of what is labelled lies inside it; the labels change each \
         plane's picture: {}, a plane with them in {:.1} ms; the solids draw something: {}, in \
         {:.1} ms; hiding every part leaves nothing: {}",
        share * 100.0,
        yes(drawn.planes_change),
        drawn.plane_ms,
        yes(drawn.solids_show),
        drawn.solids_ms,
        yes(drawn.hidden_is_black),
    );
    let held = drawn.planes_change && drawn.solids_show && drawn.hidden_is_black;
    Ok(Outcome::Checked(held, set))
}

/// What was drawn of the labels on one series.
struct Drawn {
    planes_change: bool,
    plane_ms: f64,
    solids_show: bool,
    solids_ms: f64,
    hidden_is_black: bool,
    /// The pictures, each with what it is called.
    pictures: Vec<(String, Vec<u8>)>,
}

/// The middle of the times `draw` takes, in milliseconds, and its last picture.
fn timed<E: std::fmt::Display>(
    mut draw: impl FnMut() -> Result<Vec<u8>, E>,
) -> Result<(f64, Vec<u8>), String> {
    // Once to settle, then timed.
    let mut rgba = draw().map_err(said)?;
    let mut times = Vec::new();
    for _ in 0..TIMES {
        let started = Instant::now();
        rgba = draw().map_err(said)?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    Ok((times.get(TIMES / 2).copied().unwrap_or_default(), rgba))
}

/// Draw the planes through a labelled point with and without the labels, and the solids
/// with every part shown and with every part hidden.
fn draw(
    session: &Session,
    gpu: &mut HeadlessScene,
    (volume, set): (&Volume, &LabelSet),
    through: Point<Patient>,
) -> Result<Drawn, String> {
    let state = session.state();
    let shown = state.viewports.first();
    let window = shown.map_or(WindowLevel::new(0.5, 1.0), |shown| shown.wl);
    if !gpu.label(Some(set)) {
        return Err("The graphics device has no room for the labels.".to_owned());
    }
    let looks = state.labels.looks();
    gpu.set_looks(&looks, None);
    let placement = volume.voxel_to_patient();
    let (mut planes_change, mut plane_ms, mut pictures) = (true, 0.0_f64, Vec::new());
    for kind in PlaneKind::ALL {
        let plane = plane_of(&placement, volume.dims(), kind, through, true);
        let plane = plane.ok_or("The volume's placement cannot be turned round.")?;
        let (middle, _) = plane.of(through);
        let view = PlaneView::showing(plane, &volume.corners(), [SIDE as f32; 2], 1.0, middle);
        gpu.overlay(Overlay::Off);
        let plain = gpu.render_plane(&view, window, [SIDE; 2]).map_err(said)?;
        gpu.overlay(Overlay::Filled);
        let (ms, labelled) = timed(|| gpu.render_plane(&view, window, [SIDE; 2]))?;
        planes_change &= plain != labelled;
        plane_ms = plane_ms.max(ms);
        pictures.push((kind.name().to_lowercase(), labelled));
    }
    let scene = SceneParams {
        camera: state.scene.camera,
        window,
        look: Look::Parts,
        cuts: Vec::new(),
        slice: None,
        quality: 1.0,
        skip_empty: true,
    };
    let (solids_ms, solids) = timed(|| gpu.render_parts(&scene, [SIDE; 2]))?;
    let lit = |rgba: &[u8]| {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[..3] != [0; 3])
    };
    let solids_show = lit(&solids);
    let hidden: Vec<PartLook> = looks
        .iter()
        .map(|look| PartLook {
            opacity: 0.0,
            ..*look
        })
        .collect();
    gpu.set_looks(&hidden, None);
    let hidden_is_black = !lit(&gpu.render_parts(&scene, [SIDE; 2]).map_err(said)?);
    pictures.push(("solids".to_owned(), solids));
    Ok(Drawn {
        planes_change,
        plane_ms,
        solids_show,
        solids_ms,
        hidden_is_black,
        pictures,
    })
}

/// Write the pictures of a series into a folder.
fn save(folder: &Path, number: usize, drawn: &Drawn) -> Result<(), String> {
    for (name, rgba) in &drawn.pictures {
        let path = folder.join(format!("series-{number}-labels-{name}.png"));
        let file = png(rgba, [SIDE; 2]).map_err(said)?;
        std::fs::write(path, file).map_err(said)?;
    }
    Ok(())
}
