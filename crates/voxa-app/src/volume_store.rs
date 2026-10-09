//! Where the volume the 3D scene shows is kept, between the worker that builds it and the
//! window that draws it.

use std::sync::Arc;

use voxa_core::{SeriesId, Volume};
use voxa_render::Prepared;

/// The one volume in use. A volume is large, so only the newest is kept.
#[derive(Debug, Default)]
pub struct VolumeStore {
    held: Option<(SeriesId, Arc<Volume>)>,
    /// The volume kept, made ready for the graphics device, until the window takes it.
    ready: Option<Prepared>,
}

impl VolumeStore {
    /// The volume of a series, if that is the one kept.
    pub fn get(&self, series: &SeriesId) -> Option<Arc<Volume>> {
        let (of, volume) = self.held.as_ref()?;
        (of == series).then(|| Arc::clone(volume))
    }

    /// Whether the volume of a series is the one kept.
    pub fn holds(&self, series: &SeriesId) -> bool {
        self.held.as_ref().is_some_and(|(of, _)| of == series)
    }

    /// Keep the volume of a series, in place of any kept before. The one kept before is
    /// handed back, with its series.
    pub fn put(
        &mut self,
        series: SeriesId,
        volume: Arc<Volume>,
    ) -> Option<(SeriesId, Arc<Volume>)> {
        self.ready = None;
        self.held.replace((series, volume))
    }

    /// Keep the volume of a series as it was made ready for the graphics device, if that
    /// series' volume is the one kept.
    pub fn put_prepared(&mut self, series: &SeriesId, ready: Prepared) {
        if self.holds(series) {
            self.ready = Some(ready);
        }
    }

    /// Take what was made ready for the graphics device of a series' volume. It is handed
    /// over once: the device then holds it, and it is large.
    pub fn take_prepared(&mut self, series: &SeriesId) -> Option<Prepared> {
        if self.holds(series) {
            self.ready.take()
        } else {
            None
        }
    }

    /// Keep nothing.
    pub fn clear(&mut self) {
        self.held = None;
        self.ready = None;
    }
}

#[cfg(test)]
mod tests {
    use voxa_core::Affine;
    use voxa_render::MAX_VOLUME_BYTES;

    use super::*;
    use crate::testing::{FINE_SCAN, THICK_SCAN, id};

    fn volume(value: f32) -> Arc<Volume> {
        Arc::new(Volume::new([1, 1, 2], vec![value; 2], Affine::identity()).expect("adds up"))
    }

    #[test]
    fn a_volume_is_kept_under_its_series_until_cleared() {
        let mut store = VolumeStore::default();
        assert!(store.get(&id(THICK_SCAN)).is_none());
        store.put(id(THICK_SCAN), volume(1.0));
        assert_eq!(store.get(&id(THICK_SCAN)), Some(volume(1.0)));
        assert!(
            store.get(&id(FINE_SCAN)).is_none(),
            "it is not another series' volume"
        );
        store.clear();
        assert!(store.get(&id(THICK_SCAN)).is_none());
    }

    #[test]
    fn what_was_made_ready_for_the_device_is_kept_with_its_volume_and_taken_once() {
        let mut store = VolumeStore::default();
        let ready = || Prepared::of(&volume(1.0), 2048, MAX_VOLUME_BYTES);
        store.put_prepared(&id(THICK_SCAN), ready());
        assert!(
            store.take_prepared(&id(THICK_SCAN)).is_none(),
            "with no volume, nothing"
        );
        store.put(id(THICK_SCAN), volume(1.0));
        store.put_prepared(&id(FINE_SCAN), ready());
        assert!(
            store.take_prepared(&id(FINE_SCAN)).is_none(),
            "nor for another series"
        );
        store.put_prepared(&id(THICK_SCAN), ready());
        assert!(store.take_prepared(&id(FINE_SCAN)).is_none());
        let taken = store.take_prepared(&id(THICK_SCAN)).expect("kept");
        assert_eq!(taken.dims(), [1, 1, 2]);
        assert!(store.take_prepared(&id(THICK_SCAN)).is_none(), "taken once");
        store.put_prepared(&id(THICK_SCAN), ready());
        store.put(id(FINE_SCAN), volume(2.0));
        assert!(
            store.take_prepared(&id(THICK_SCAN)).is_none(),
            "it goes with its volume"
        );
    }

    #[test]
    fn the_store_says_whether_it_holds_a_series_and_hands_back_what_gives_way() {
        let mut store = VolumeStore::default();
        assert!(!store.holds(&id(THICK_SCAN)));
        let before = store.put(id(THICK_SCAN), volume(1.0));
        assert!(before.is_none(), "nothing gave way");
        assert!(store.holds(&id(THICK_SCAN)) && !store.holds(&id(FINE_SCAN)));
        let before = store.put(id(FINE_SCAN), volume(2.0));
        assert_eq!(before, Some((id(THICK_SCAN), volume(1.0))));
    }

    #[test]
    fn a_second_volume_takes_the_place_of_the_first() {
        let mut store = VolumeStore::default();
        store.put(id(THICK_SCAN), volume(1.0));
        store.put(id(FINE_SCAN), volume(2.0));
        assert!(store.get(&id(THICK_SCAN)).is_none());
        assert_eq!(store.get(&id(FINE_SCAN)), Some(volume(2.0)));
    }
}
