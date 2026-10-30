//! Work done off the interface's thread: indexing a source, decoding its slices, and
//! building the volume of a series.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use voxa_core::SeriesId;
use voxa_dicom::{Index, IndexedSeries, SeriesSource, index, open_source};
use voxa_labels::LabelSet;

use crate::action::{Action, Effect};
use crate::cache::FrameCache;
use crate::volume_store::VolumeStore;

mod annotations;
mod frames;
mod held;
mod kept;
pub(crate) mod labelling;
mod labels;
mod opened;
mod post;
mod shown;
mod tags;
mod views;
mod volume;

pub use labels::{MAX_LABEL_FILE_BYTES, NAMES_NOT_READ, table_beside};

/// The longest side of a volume taken to fit a graphics device until the window says what
/// its own allows: what every device made for the desktop can hold.
const DEFAULT_VOLUME_SIDE: u32 = 2048;

/// Runs the work [`reduce`](crate::reduce) asks for on background threads and reports back
/// with further [`Action`]s. What they write to the log never holds what a file contains.
pub struct Workers {
    /// The threads the work runs on; the shared pool of the process when none could be made.
    pool: Option<rayon::ThreadPool>,
    shared: Arc<Shared>,
}

/// What every background task can reach.
struct Shared {
    reports: Sender<Action>,
    cache: Arc<Mutex<FrameCache>>,
    /// The source that is open, once it has been indexed.
    source: Mutex<Option<Box<dyn SeriesSource>>>,
    /// How many sources have been opened. Work begun for an earlier one stops.
    opened: AtomicU64,
    /// The file of many frames opened last.
    kept: opened::Opened,
    /// The newest request for each series. Work begun for an earlier request stops.
    newest: Mutex<HashMap<SeriesId, u64>>,
    requests: AtomicU64,
    /// The volume the 3D scene shows.
    volumes: Arc<Mutex<VolumeStore>>,
    /// How many volumes have been asked for. One being built for an earlier request stops.
    builds: AtomicU64,
    /// The longest side, in voxels, of a volume the window's graphics device can hold.
    volume_side: AtomicU32,
    /// What the window's graphics device does itself, and so is not done here first.
    device_does: volume::DeviceDoes,
    /// The series whose volume is being built. Its slices are decoded for the volume.
    wanted_volume: Mutex<Option<SeriesId>>,
    /// The folder views, annotations and labelling are kept in, if there is one.
    settings: Option<PathBuf>,
    /// How many times the saved views have been asked to be written, and the newest of
    /// those requests that has been.
    stores: AtomicU64,
    stored: Mutex<u64>,
    /// The same two counts for the kept annotations.
    notes: AtomicU64,
    noted: Mutex<u64>,
    /// The labels kept, and how many reads (and runs) of labels have been asked for or given up.
    labels: Mutex<Option<Arc<LabelSet>>>,
    label_reads: AtomicU64,
    labelling: labelling::Kit,
    /// The files of the labels held, and the choices written.
    keeping: kept::Keeping,
}

/// Some slices to decode from one file: (place in the series, frame within the file).
struct Batch {
    entry: String,
    slices: Vec<(usize, Option<u32>)>,
}

/// One request for the frames of a series.
struct Request {
    series: SeriesId,
    /// The slice being shown, which frames are kept near.
    around: usize,
    /// Which source was open when the request was made.
    opened: u64,
    /// Which request for this series it is.
    number: u64,
}

impl Workers {
    /// Workers that report through `reports`, keep slices in `cache`, volumes in `volumes` and
    /// views in the folder `settings` if one is given, and label with `labeller` or the runtime.
    pub fn new(
        reports: Sender<Action>,
        cache: Arc<Mutex<FrameCache>>,
        volumes: Arc<Mutex<VolumeStore>>,
        settings: Option<PathBuf>,
        labeller: Option<Arc<dyn labelling::Labeller>>,
    ) -> Self {
        let named = rayon::ThreadPoolBuilder::new().thread_name(|n| format!("voxa-worker-{n}"));
        let shared = Shared {
            reports,
            cache,
            source: Mutex::new(None),
            opened: AtomicU64::new(0),
            kept: opened::Opened::default(),
            newest: Mutex::new(HashMap::new()),
            requests: AtomicU64::new(0),
            volumes,
            builds: AtomicU64::new(0),
            volume_side: AtomicU32::new(DEFAULT_VOLUME_SIDE),
            device_does: volume::DeviceDoes::default(),
            wanted_volume: Mutex::new(None),
            settings,
            stores: AtomicU64::new(0),
            stored: Mutex::new(0),
            notes: AtomicU64::new(0),
            noted: Mutex::new(0),
            labels: Mutex::new(None),
            label_reads: AtomicU64::new(0),
            labelling: labelling::Kit::new(labeller),
            keeping: kept::Keeping::default(),
        };
        Self {
            pool: named.build().ok(),
            shared: Arc::new(shared),
        }
    }

    /// Do what an effect asks. `index` is the index the state holds, which says where the
    /// slices of a series are to be found.
    pub fn run(&self, effect: Effect, index: Option<&Index>) {
        match effect {
            Effect::StartIndex(path) => self.start_index(path),
            Effect::RequestFrames { series, around } => {
                let listed =
                    index.and_then(|index| index.series.iter().find(|s| s.info.id == series));
                if let Some(listed) = listed {
                    self.request_frames(listed, around, usize::MAX);
                }
            }
            Effect::RequestThumbnails => {
                // One small picture does for all the volumes of a series that holds several.
                let series = index.map_or(&[][..], |index| index.series.as_slice());
                for listed in series.iter().filter(|listed| listed.info.leads_its_set()) {
                    self.request_frames(listed, listed.slices.len() / 2, 1);
                }
            }
            Effect::BuildVolume { series } => {
                let listed =
                    index.and_then(|index| index.series.iter().find(|s| s.info.id == series));
                if let Some(listed) = listed {
                    self.build_volume(listed);
                }
            }
            Effect::BuildSmooth { series } => self.build_smooth(series),
            Effect::DropSmooth => lock(&self.shared.volumes).want_smoothed(false),
            Effect::LoadViews { series } => self.load_views(series),
            Effect::StoreViews { series, views } => self.store_views(&series, views),
            Effect::LoadAnnotations { series } => self.load_annotations(series),
            Effect::StoreAnnotations { series, items } => self.store_annotations(&series, items),
            Effect::ReadLabels { labels, table } => self.read_labels(labels, table),
            Effect::DropLabels => self.drop_labels(),
            labelling @ (Effect::LookForLabelling { .. }
            | Effect::LabelScan { .. }
            | Effect::StopLabelling
            | Effect::TakeLabels { .. }) => self.labelling(labelling),
            kept @ (Effect::FindKept { .. }
            | Effect::ForgetKept { .. }
            | Effect::SaveLabels { .. }
            | Effect::StoreChoices(_)) => self.kept(kept),
            Effect::None => {}
        }
    }

    /// Open another source. Everything belonging to the one before is dropped at once; work
    /// still running for it finds itself out of date and stops.
    fn start_index(&self, path: PathBuf) {
        let opened = self.shared.opened.fetch_add(1, Ordering::SeqCst) + 1;
        *lock(&self.shared.source) = None;
        self.shared.kept.clear();
        lock(&self.shared.newest).clear();
        lock(&self.shared.cache).clear();
        lock(&self.shared.volumes).clear();
        *lock(&self.shared.wanted_volume) = None;
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let finished = shielded(|| shared.index_source(&path, opened));
            if !finished && shared.opened.load(Ordering::SeqCst) == opened {
                // Without this the window would wait for ever for the index.
                let why = "This scan could not be read: the reader stopped on one of its files";
                shared.report(Action::IndexFailed(why.to_owned()));
            }
        });
    }

    /// Decode the slices of a series that are not yet in the cache, those nearest `around`
    /// first, and no more of them than `at_most` or than the cache can hold. None of a series
    /// held as a volume, and one at most of a series whose volume is being built.
    fn request_frames(&self, listed: &IndexedSeries, around: usize, at_most: usize) {
        let shared = &self.shared;
        let series = listed.info.id.clone();
        // A held series' slices and windows come from its volume. One whose volume is being
        // built is decoded there once; here only the slice shown.
        if self.say_windows(&series, around) {
            return;
        }
        let building = lock(&shared.wanted_volume).as_ref() == Some(&series);
        let at_most = if building { at_most.min(1) } else { at_most };
        let number = shared.requests.fetch_add(1, Ordering::SeqCst) + 1;
        lock(&shared.newest).insert(series.clone(), number);
        let request = Request {
            series,
            around,
            opened: shared.opened.load(Ordering::SeqCst),
            number,
        };

        let frame_bytes =
            usize::try_from(listed.info.voxel_count() / listed.slices.len().max(1) as u64)
                .unwrap_or(usize::MAX)
                .saturating_mul(size_of::<f32>());
        let room = (lock(&shared.cache).budget() / frame_bytes.max(1)).max(1);
        let mut nearest: Vec<usize> = (0..listed.slices.len()).collect();
        nearest.sort_by_key(|slice| slice.abs_diff(around));
        nearest.truncate(room.min(at_most));
        let wanted = {
            let cache = lock(&shared.cache);
            nearest
                .into_iter()
                .filter(|slice| !cache.contains(&request.series, *slice))
                .collect()
        };
        let mut batches = batches(listed, wanted).into_iter();
        let Some(first) = batches.next() else {
            return;
        };
        // The file holding the slice being shown is decoded before the others are begun, so
        // that slice is always the first to arrive.
        let (shared, request) = (Arc::clone(&self.shared), Arc::new(request));
        self.spawn(move || {
            shared.decode(&first, &request);
            for batch in batches {
                let (shared, request) = (Arc::clone(&shared), Arc::clone(&request));
                rayon::spawn_fifo(move || {
                    shielded(|| shared.decode(&batch, &request));
                });
            }
        });
    }

    fn spawn(&self, work: impl FnOnce() + Send + 'static) {
        let work = move || {
            shielded(work);
        };
        match &self.pool {
            Some(pool) => pool.spawn(work),
            None => rayon::spawn(work),
        }
    }
}

impl Shared {
    /// Index the source at `path`, reporting progress, and keep the source for decoding.
    fn index_source(&self, path: &Path, opened: u64) {
        let current = || self.opened.load(Ordering::SeqCst) == opened;
        let outcome = open_source(path).and_then(|mut source| {
            let mut report = |so_far: &Index| {
                if current() {
                    self.report(Action::IndexProgress(so_far.clone()));
                }
            };
            let found = index(source.as_mut(), &mut report)?;
            Ok((source, found))
        });
        // Checked with the source locked, so another source opened meanwhile is not overwritten.
        let mut kept = lock(&self.source);
        if !current() {
            return;
        }
        match outcome {
            Ok((source, found)) if !found.series.is_empty() => {
                *kept = Some(source);
                self.report(Action::IndexDone);
            }
            Ok((_, found)) => self.report(Action::IndexFailed(post::nothing_found(path, &found))),
            Err(error) => self.could_not_open(&error),
        }
    }

    fn report(&self, action: Action) {
        // If the application has gone there is no one left to tell.
        let _ = self.reports.send(action);
    }
}

/// Run a piece of background work so that a panic inside it ends that piece of work and
/// nothing else; false when it panicked. Left alone, a panic on a worker thread would stop
/// the whole program, and the readers and decoders are other people's code fed whatever a
/// file holds.
fn shielded(work: impl FnOnce()) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).is_ok()
}

/// Group slices by the file that holds them, keeping the order they were asked for in: a
/// file is read and parsed once however many of its frames are wanted.
fn batches(listed: &IndexedSeries, wanted: Vec<usize>) -> Vec<Batch> {
    let mut batches: Vec<Batch> = Vec::new();
    for slice in wanted {
        let Some(at) = listed.slices.get(slice) else {
            continue;
        };
        match batches.iter_mut().find(|batch| batch.entry == at.entry) {
            Some(batch) => batch.slices.push((slice, at.frame)),
            None => batches.push(Batch {
                entry: at.entry.clone(),
                slices: vec![(slice, at.frame)],
            }),
        }
    }
    batches
}

/// Lock a mutex, carrying on with what it holds even if a thread panicked while holding it.
fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod device_tests;
#[cfg(test)]
mod labels_tests;
#[cfg(test)]
mod once_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod volume_tests;
