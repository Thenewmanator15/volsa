//! One open scan and everything working on it, with or without a window.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use voxa_core::{SeriesId, Volume};
use voxa_dicom::Frame;

use crate::action::Action;
use crate::cache::FrameCache;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::volume_store::VolumeStore;
use crate::worker::Workers;

/// The state, the workers, the frame cache and the volume store, joined up: an action goes in, the state
/// changes, and the work it calls for is handed to the workers, whose reports come back as
/// further actions. The window is a view of this; the tests drive it with no window at all.
pub struct Session {
    state: AppState,
    workers: Workers,
    reports: Receiver<Action>,
    cache: Arc<Mutex<FrameCache>>,
    volumes: Arc<Mutex<VolumeStore>>,
}

impl Session {
    /// A session with nothing open. The workers report through `to_reports`, and `reports` is
    /// where those reports are read from: the two ends of one channel, or of two channels
    /// with something in between, as the window has to wake itself up. The cache holds at
    /// most `cache_bytes` of pixel values. Saved views are kept in the folder `settings`; with
    /// `None` they last only as long as the session.
    pub fn new(
        to_reports: Sender<Action>,
        reports: Receiver<Action>,
        cache_bytes: usize,
        settings: Option<PathBuf>,
    ) -> Self {
        let cache = Arc::new(Mutex::new(FrameCache::new(cache_bytes)));
        let volumes = Arc::new(Mutex::new(VolumeStore::default()));
        let (kept, built) = (Arc::clone(&cache), Arc::clone(&volumes));
        let workers = Workers::new(to_reports, kept, built, settings);
        Self {
            state: AppState::default(),
            workers,
            reports,
            cache,
            volumes,
        }
    }

    /// Everything the application remembers.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The workers, for work that is not asked for through an action.
    pub fn workers(&self) -> &Workers {
        &self.workers
    }

    /// The decoded slices.
    pub fn cache(&self) -> &Mutex<FrameCache> {
        &self.cache
    }

    /// A decoded slice, if it is in the cache.
    pub fn cached(&self, series: &SeriesId, slice: usize) -> Option<Arc<Frame>> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series, slice)
    }

    /// The volume built for a series, if it is the one kept for the 3D scene.
    pub fn volume(&self, series: &SeriesId) -> Option<Arc<Volume>> {
        self.volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series)
    }

    /// Apply an action and hand the work it calls for to the workers.
    pub fn dispatch(&mut self, action: Action) {
        for effect in reduce(&mut self.state, action) {
            self.workers.run(effect, self.state.index.as_ref());
        }
    }

    /// Take in everything the workers have reported so far; how many reports that was.
    pub fn pump(&mut self) -> usize {
        let mut taken = 0;
        while let Ok(report) = self.reports.try_recv() {
            self.dispatch(report);
            taken += 1;
        }
        taken
    }

    /// Wait up to `at_most` for the workers to report, then take in everything they have
    /// reported; how many reports that was.
    pub fn wait(&mut self, at_most: Duration) -> usize {
        match self.reports.recv_timeout(at_most) {
            Ok(report) => {
                self.dispatch(report);
                1 + self.pump()
            }
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests;
