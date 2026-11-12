//! Reading and writing where series were left, off the interface's thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use volsa_core::{SeriesId, StudyId};

use super::newest::Newest;
use super::{Workers, lock};
use crate::action::{Action, Effect};
use crate::places::{self, Place, PlacesAction};
use crate::views::series_key;

/// What the work on the kept places keeps between one piece of it and the next.
#[derive(Default)]
pub(super) struct Kit {
    /// How many writes have been asked for.
    asked: AtomicU64,
    /// The newest write made for each series and study. It is held while the file is read
    /// or written, so that the file is never read half written.
    written: Mutex<Newest>,
    /// Whether a failure to write has been logged, so that it is logged once.
    failed: AtomicBool,
    /// How many writes are on a worker's thread and not yet done.
    in_flight: Mutex<usize>,
    /// Told each time a write on a worker's thread is done.
    settled: Condvar,
}

/// The longest Volsa waits on closing for writes already under way, so that a file system
/// that has stopped answering cannot hold the window open.
const LONGEST_WAIT: Duration = Duration::from_secs(5);

/// A write on a worker's thread, counted from when it is asked for until it is done, whether
/// it wrote or found nothing to write.
struct Flying(Arc<super::Shared>);

impl Flying {
    /// Count a write asked for.
    fn start(shared: &Arc<super::Shared>) -> Self {
        *lock(&shared.places.in_flight) += 1;
        Self(Arc::clone(shared))
    }
}

impl Drop for Flying {
    fn drop(&mut self) {
        let kit = &self.0.places;
        let mut count = lock(&kit.in_flight);
        *count = count.saturating_sub(1);
        kit.settled.notify_all();
    }
}

/// Wait until no write is left on a worker's thread, or [`LONGEST_WAIT`] has gone by.
fn wait_for_writes(kit: &Kit) {
    let count = lock(&kit.in_flight);
    let waited = kit
        .settled
        .wait_timeout_while(count, LONGEST_WAIT, |count| *count > 0);
    drop(waited);
}

/// The name a study's first series is filed under: the same hash as a series' key, of the
/// study's identifier, which is not kept.
fn study_key(study: &StudyId) -> String {
    series_key(&SeriesId(study.0.clone()))
}

impl Workers {
    /// Do what an effect about the kept places asks.
    pub(super) fn places(&self, effect: Effect) {
        match effect {
            Effect::LoadPlaces {
                opened,
                series,
                studies,
            } => self.load_places(opened, series, studies),
            Effect::StorePlaces { places, first } => self.store_places(places, first, false),
            _ => {}
        }
    }

    /// Read what is kept for these series and studies and report it: the place of each
    /// series that has one, and the series kept as the first of whichever of the studies was
    /// written last, if it is among these. With nowhere to keep settings there is nothing to
    /// read and nothing is reported.
    fn load_places(&self, opened: u64, series: Vec<SeriesId>, studies: Vec<StudyId>) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            // Not while the file is half written.
            let _written = lock(&shared.places.written);
            let kept = places::load(&folder);
            let studies: Vec<String> = studies.iter().map(study_key).collect();
            let wanted = kept.first(&studies);
            let first = series
                .iter()
                .find(|id| wanted == Some(series_key(id).as_str()))
                .cloned();
            let places = series
                .into_iter()
                .filter_map(|id| Some((kept.place(&series_key(&id))?, id)))
                .map(|(place, id)| (id, place))
                .collect();
            shared.report(Action::Places(PlacesAction::Loaded {
                opened,
                places,
                first,
            }));
        });
    }

    /// Write where these series were left, and a study's first series, to the file. Of two
    /// writes of one series asked for close together, the later is the one left in the file,
    /// whichever a worker gets to first. With `at_once` the file is written before this
    /// returns, after any write still under way, as it must be when Volsa is closing;
    /// otherwise on a worker's thread.
    pub(crate) fn store_places(
        &self,
        places: Vec<(SeriesId, Place)>,
        first: Option<(StudyId, SeriesId)>,
        at_once: bool,
    ) {
        let Some(folder) = self.shared.settings.clone() else {
            return;
        };
        let number = self.shared.places.asked.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        let keyed = |(id, place): (SeriesId, Place)| (series_key(&id), place);
        let places: Vec<(String, Place)> = places.into_iter().map(keyed).collect();
        let first = first.map(|(study, series)| (study_key(&study), series_key(&series)));
        let write = move || {
            let mut newest = lock(&shared.places.written);
            let places: Vec<(String, Place)> = places
                .into_iter()
                .filter(|(key, _)| newest.takes(key, number))
                .collect();
            // A study's count is apart from that of a series that happens to share its key.
            let first = first.filter(|(study, _)| newest.takes(&format!("study {study}"), number));
            if places.is_empty() && first.is_none() {
                return;
            }
            let first = first.as_ref().map(|(study, series)| (&**study, &**series));
            let failed = places::save(&folder, &places, first).is_err();
            if failed && !shared.places.failed.swap(true, Ordering::SeqCst) {
                tracing::warn!("where the scans were left could not be written");
            }
        };
        if at_once {
            // A write asked for earlier may still be on a worker's thread: it lands first, so
            // that nothing asked for before the window closed is lost with it.
            wait_for_writes(&self.shared.places);
            write();
        } else {
            let flying = Flying::start(&self.shared);
            self.spawn(move || {
                write();
                drop(flying);
            });
        }
    }
}
