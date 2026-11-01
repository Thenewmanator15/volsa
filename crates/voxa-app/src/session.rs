//! One open scan and everything working on it, with or without a window.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use voxa_core::{Grid, HeldVolume, SeriesId};
use voxa_dicom::Frame;
use voxa_labels::LabelSet;
use voxa_render::Smoothed;

use crate::action::{Action, Effect};
use crate::cache::FrameCache;
use crate::choices::load_choices;
use crate::measuring::SliceValues;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::volume_store::VolumeStore;
use crate::worker::Workers;

#[cfg(test)]
thread_local! {
    /// How many slices [`Session::cached`] has copied out of a volume on this thread: what the
    /// tests count, to show that reading a value copies none.
    static SLICES_COPIED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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
    /// most `cache_bytes` of pixel values. Saved views, annotations, a result of labelling and
    /// what is chosen for labelling are kept in the folder `settings`, which the window gives
    /// as the folder on this computer alone, and what was chosen in the run before is chosen
    /// again; with `None` they last only as long as the session.
    pub fn new(
        to_reports: Sender<Action>,
        reports: Receiver<Action>,
        budget_bytes: usize,
        settings: Option<PathBuf>,
    ) -> Self {
        // The frame cache has the whole budget until a volume is kept.
        let cache = Arc::new(Mutex::new(FrameCache::new(budget_bytes)));
        let volumes = Arc::new(Mutex::new(VolumeStore::default()));
        let (kept, built) = (Arc::clone(&cache), Arc::clone(&volumes));
        let mut state = AppState::default();
        if let Some(folder) = &settings {
            load_choices(folder).put_in(&mut state.labelling);
        }
        let workers = Workers::new(to_reports, kept, built, settings, None);
        Self {
            state,
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

    /// A decoded slice: from the frame cache if it is there, and otherwise read out of the
    /// series' volume, if that is the one kept. A series shown in 3D is held once, as its
    /// volume; a slice read out of it is a copy made for the asking.
    pub fn cached(&self, series: &SeriesId, slice: usize) -> Option<Arc<Frame>> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let cached = cache.get(series, slice);
        drop(cache);
        cached.or_else(|| {
            // A viewport still waiting for its automatic window is not given the slice yet:
            // it would be drawn once in a window chosen for nothing. Its window comes with
            // the slice it asked for, a moment after the volume is kept.
            let waiting = self.state.viewports.iter().any(|shown| {
                shown.series.as_ref() == Some(series) && shown.slice == slice && shown.wl_pending
            });
            if waiting {
                return None;
            }
            let volume = self.volume(series)?;
            let [cols, rows, _] = volume.dims();
            let values = volume.slice_values(u32::try_from(slice).ok()?)?;
            let colour = None;
            #[cfg(test)]
            SLICES_COPIED.with(|copied| copied.set(copied.get() + 1));
            Some(Arc::new(Frame {
                rows,
                cols,
                values,
                colour,
            }))
        })
    }

    /// Read the values of a slice where they lie, with `read`, and give what it gives: from the
    /// decoded slice if the cache holds it, and otherwise from the series' kept volume, whose
    /// slice is turned from its values as stored. Unlike [`Session::cached`], nothing is built
    /// or kept. This is the one way the window reads a
    /// slice's values: the value under the pointer as it moves, and what a region drawn on a
    /// slice holds. `None` when neither holds the slice, or it is a picture in colour, whose
    /// values only say how bright each pixel is.
    pub fn read_slice<T>(
        &self,
        series: &SeriesId,
        slice: usize,
        read: impl FnOnce(SliceValues<'_>) -> T,
    ) -> Option<T> {
        let cached = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series, slice);
        if let Some(frame) = cached {
            return frame
                .colour
                .is_none()
                .then(|| read(SliceValues::from(&*frame)));
        }
        let volume = self.volume(series)?;
        let [cols, rows, _] = volume.dims();
        let values = volume.slice_values(u32::try_from(slice).ok()?)?;
        Some(read(SliceValues {
            values: &values,
            size: [cols, rows],
        }))
    }

    /// The value of one pixel, as its column and row, of a slice, read where it lies as
    /// [`Session::read_slice`] reads it, so that it can be asked for each time the pointer
    /// moves: from the decoded slice if the cache holds it, and otherwise the one voxel of the
    /// kept volume, with no slice turned or copied. `None` when the slice is not there to read,
    /// the pixel is not in it, or it is a picture in colour.
    pub fn value_at(&self, series: &SeriesId, slice: usize, pixel: (usize, usize)) -> Option<f32> {
        let cached = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series, slice);
        if let Some(frame) = cached {
            let grey = frame.colour.is_none();
            return SliceValues::from(&*frame).at(pixel).filter(|_| grey);
        }
        let volume = self.volume(series)?;
        let place = |at: usize| u32::try_from(at).ok();
        volume.value(place(pixel.0)?, place(pixel.1)?, place(slice)?)
    }

    /// The volume built for a series, if it is the one kept for the 3D scene.
    pub fn volume(&self, series: &SeriesId) -> Option<Arc<HeldVolume>> {
        self.volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(series)
    }

    /// What the graphics device is to hold of a series' volume, and how many times it was
    /// halved to fit: the volume itself, or a copy halved by a worker when the device cannot
    /// hold it whole. `None` when the series' volume is not the one kept.
    pub fn for_card(&self, series: &SeriesId) -> Option<(Arc<HeldVolume>, u32)> {
        let store = self.volumes.lock().unwrap_or_else(PoisonError::into_inner);
        store
            .for_card(series)
            .or_else(|| store.get(series).map(|volume| (volume, 0)))
    }

    /// Take the preview a worker made of a series' volume while it builds, if it is there
    /// to take. It is handed over once.
    pub fn take_preview(&self, series: &SeriesId) -> Option<voxa_core::Volume> {
        self.volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take_preview(series)
    }

    /// Take the curve a worker made through a series' volume, for planes drawn smooth, if
    /// it is there to take. It is handed over once.
    pub fn take_smoothed(&self, series: &SeriesId) -> Option<Smoothed> {
        self.volumes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take_smoothed(series)
    }

    /// The labels a worker has read, if there are any and they have not been given up.
    pub fn labels(&self) -> Option<Arc<LabelSet>> {
        self.workers.labels()
    }

    /// Apply an action and hand the work it calls for to the workers.
    pub fn dispatch(&mut self, action: Action) {
        let (indexed, before) = (action == Action::IndexDone, self.state.notices.clone());
        let mut effects = reduce(&mut self.state, action);
        crate::log::what_changed(&before, &self.state, indexed);
        // A volume asked for is made known to the workers before the frames of its series
        // are: the slices are then decoded once, for the volume.
        effects.sort_by_key(|effect| !matches!(effect, Effect::BuildVolume { .. }));
        for effect in effects {
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
mod annotation_tests;
#[cfg(test)]
mod holding;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod lost_tests;
#[cfg(test)]
mod shown_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod value_tests;
