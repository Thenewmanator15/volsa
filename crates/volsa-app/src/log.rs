//! A log of what Volsa did, for finding out afterwards why something went wrong.
//!
//! It holds when Volsa started and on what, how much an export held, each thing Volsa had to
//! tell the person, and where a crash happened. It never holds a patient's name, identifier
//! or date, a value from a file, or the name of any of the person's files: a notice whose words
//! may name a file is written as its kind alone, and nothing other people's libraries log is
//! taken. A crash is placed by its file and line in Volsa's source or a library's.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::span;
use volsa_dicom::Index;

use crate::state::{AppState, Notice, NoticeKind};

/// The name of the log within its folder.
pub const FILE: &str = "volsa.log";
/// The size past which the log is cut back when Volsa starts.
pub const MOST_BYTES: u64 = 1 << 20;
/// The keys of the notices whose words are Volsa's own from first to last. A notice under
/// any other key may quote a file's name or a reason from outside Volsa, and is written by
/// its kind and key alone.
const OWN_WORDS: [&str; 11] = [
    "annotations",
    "graphics",
    "label names",
    "labels",
    "measurements",
    "measuring",
    "nothing to label",
    "refused",
    "skipped",
    "smoothing",
    "views",
];

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// The folder the system gives Volsa for what it keeps on this machine. `None` when the
/// system has no such place.
pub fn folder() -> Option<PathBuf> {
    let places = directories::ProjectDirs::from("", "", "Volsa")?;
    Some(places.data_local_dir().to_owned())
}

/// Where the log of this run is, once [`start`] has opened one.
pub fn path() -> Option<&'static Path> {
    PATH.get().map(PathBuf::as_path)
}

/// Writes what Volsa's own code logs, a line each, to the end of a file.
pub struct ToFile {
    file: Mutex<File>,
}

/// The message of an event, taken out of its fields.
#[derive(Default)]
struct Said(String);

impl Visit for Said {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // Writing to a string does not fail.
            let _ = write!(self.0, "{value:?}");
        }
    }
}

impl tracing::Subscriber for ToFile {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with("volsa") && *metadata.level() <= tracing::Level::INFO
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut said = Said::default();
        event.record(&mut said);
        let level = event.metadata().level();
        let line = format!("{} {level:<5} {}\n", stamp(SystemTime::now()), said.0);
        if let Ok(mut file) = self.file.lock() {
            // A log that cannot be written is not a reason to stop.
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

/// The end of `text`, in whole lines, that fits in `most` bytes.
pub fn cut_back(text: &str, most: usize) -> &str {
    if text.len() <= most {
        return text;
    }
    let from = text.len() - most;
    let line_start = text
        .get(from..)
        .and_then(|tail| tail.find('\n'))
        .map_or(text.len(), |at| from + at + 1);
    text.get(line_start..).unwrap_or("")
}

/// Open the log in `folder`, making the folder if need be and cutting the log back to half
/// its limit when it has passed the limit. `None` when that cannot be done.
pub fn to_file(folder: &Path) -> Option<ToFile> {
    std::fs::create_dir_all(folder).ok()?;
    let path = folder.join(FILE);
    let grown = std::fs::metadata(&path).is_ok_and(|file| file.len() > MOST_BYTES);
    if grown {
        let all = std::fs::read(&path).ok()?;
        let all = String::from_utf8_lossy(&all);
        let half = usize::try_from(MOST_BYTES / 2).unwrap_or(usize::MAX);
        std::fs::write(&path, cut_back(&all, half)).ok()?;
    }
    let file = File::options().create(true).append(true).open(&path);
    Some(ToFile {
        file: Mutex::new(file.ok()?),
    })
}

/// Begin the log of this run in `folder`: from now on what Volsa logs goes there, and so does
/// the place of a crash. The file's path, or `None` when there is nowhere to keep a log or
/// one has been begun already.
pub fn start(folder: Option<&Path>) -> Option<PathBuf> {
    let folder = folder?;
    let logger = to_file(folder)?;
    tracing::subscriber::set_global_default(logger).ok()?;
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("{}", crash_line(info.location()));
        before(info);
    }));
    let (version, system) = (env!("CARGO_PKG_VERSION"), std::env::consts::OS);
    tracing::info!(
        "Volsa {version} started on {system} ({})",
        std::env::consts::ARCH
    );
    let path = folder.join(FILE);
    Some(PATH.get_or_init(|| path).clone())
}

/// What is logged of a crash: where in the source it happened, Volsa's or a library's, and
/// nothing of what the failing code said, which may quote what it was working on.
pub fn crash_line(place: Option<&std::panic::Location<'_>>) -> String {
    match place {
        Some(place) => format!(
            "Volsa stopped at {}:{}:{}",
            place.file(),
            place.line(),
            place.column()
        ),
        None => "Volsa stopped at a place that is not known".to_owned(),
    }
}

/// The year, month and day of a day counted from the first of January 1970.
fn civil(days: u64) -> (u64, u64, u64) {
    // Howard Hinnant's algorithm, for days that are not before 1970.
    let shifted = days + 719_468;
    let (era, of_era) = (shifted / 146_097, shifted % 146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_based = (5 * of_year + 2) / 153;
    let day = of_year - (153 * march_based + 2) / 5 + 1;
    let month = if march_based < 10 {
        march_based + 3
    } else {
        march_based - 9
    };
    (year_of_era + era * 400 + u64::from(month <= 2), month, day)
}

/// A moment as a date and a time of day in UTC: `2024-02-29 01:01:01Z`.
fn stamp(moment: SystemTime) -> String {
    let seconds = moment
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (year, month, day) = civil(seconds / 86_400);
    let of_day = seconds % 86_400;
    let (hour, minute, second) = (of_day / 3600, of_day % 3600 / 60, of_day % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}Z")
}

/// How a notice is written in the log: its kind and key, and its words when they are Volsa's
/// own. Never its details, which are the names of files.
pub fn line_for(notice: &Notice) -> String {
    let kind = match notice.kind {
        NoticeKind::Info => "notice",
        NoticeKind::Warning => "warning",
        NoticeKind::Error => "error",
    };
    let key = notice.key.unwrap_or("no key");
    if OWN_WORDS.contains(&key) {
        format!("{kind} [{key}]: {}", notice.text)
    } else {
        format!("{kind} [{key}] (said in the window; its words may name a file)")
    }
}

/// Log a notice.
pub fn said(notice: &Notice) {
    let line = line_for(notice);
    match notice.kind {
        NoticeKind::Info => tracing::info!("{line}"),
        NoticeKind::Warning => tracing::warn!("{line}"),
        NoticeKind::Error => tracing::error!("{line}"),
    }
}

/// Log that an export was opened, by how much it held and nothing else.
pub fn opened(index: &Index) {
    let counted = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    tracing::info!(
        "an export was opened: {}, {}, {} skipped",
        counted(index.studies.len(), "study", "studies"),
        counted(index.series.len(), "series", "series"),
        counted(index.skipped.len(), "file", "files"),
    );
}

/// Log what an action changed that is worth a line: each notice that was not there `before`,
/// and, when the action was the end of indexing, what the export held.
pub fn what_changed(before: &[Notice], state: &AppState, indexed: bool) {
    for notice in state.notices.iter().filter(|now| !before.contains(now)) {
        said(notice);
    }
    if let Some(index) = state.index.as_ref().filter(|_| indexed) {
        opened(index);
    }
}

/// Log which graphics device the window draws with.
pub fn device(render: Option<&egui_wgpu::RenderState>) {
    if let Some(found) = render.map(|render| render.adapter.get_info()) {
        tracing::info!("graphics device: {} ({:?})", found.name, found.backend);
    } else {
        tracing::warn!("no graphics device");
    }
}

#[cfg(test)]
mod tests;
