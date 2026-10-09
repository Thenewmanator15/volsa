//! Where the volume the 3D scene shows is kept, between the worker that builds it and the
//! window that draws it.

use std::sync::Arc;

use voxa_core::{SeriesId, Volume};

/// The one volume in use. A volume is large, so only the newest is kept.
#[derive(Debug, Default)]
pub struct VolumeStore {
    held: Option<(SeriesId, Arc<Volume>)>,
}

impl VolumeStore {
    /// The volume of a series, if that is the one kept.
    pub fn get(&self, series: &SeriesId) -> Option<Arc<Volume>> {
        let (of, volume) = self.held.as_ref()?;
        (of == series).then(|| Arc::clone(volume))
    }

    /// Keep the volume of a series, in place of any kept before.
    pub fn put(&mut self, series: SeriesId, volume: Arc<Volume>) {
        self.held = Some((series, volume));
    }

    /// Keep nothing.
    pub fn clear(&mut self) {
        self.held = None;
    }
}

#[cfg(test)]
mod tests {
    use voxa_core::Affine;

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
    fn a_second_volume_takes_the_place_of_the_first() {
        let mut store = VolumeStore::default();
        store.put(id(THICK_SCAN), volume(1.0));
        store.put(id(FINE_SCAN), volume(2.0));
        assert!(store.get(&id(THICK_SCAN)).is_none());
        assert_eq!(store.get(&id(FINE_SCAN)), Some(volume(2.0)));
    }
}
