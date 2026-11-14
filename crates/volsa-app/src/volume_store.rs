//! Where the volume the 3D scene shows is kept, between the worker that builds it and the
//! window that draws it.

use std::sync::Arc;

use volsa_core::{Grid, HeldVolume, SeriesId, Volume};
use volsa_render::Smoothed;

/// The one volume in use. A volume is large, so only the newest is kept.
#[derive(Debug, Default)]
pub struct VolumeStore {
    held: Option<(SeriesId, Arc<HeldVolume>)>,
    /// A copy of the volume kept, halved as often as it says to fit the graphics device,
    /// when the device cannot hold the volume itself.
    for_card: Option<(Arc<HeldVolume>, u32)>,
    /// The curve through the volume kept, for planes drawn smooth, until the window takes it.
    curve: Option<Smoothed>,
    /// Whether such a curve is wanted: one that arrives when it is not is not kept.
    curve_wanted: bool,
    /// A small, rough volume of a long series being built, for the window to take and draw
    /// until the volume itself is kept.
    preview: Option<(SeriesId, Volume)>,
    /// True when the volume kept is a susceptibility map worked out from its series, held in
    /// place of the series' own values.
    map: bool,
}

impl VolumeStore {
    /// The volume of a series, if that is the one kept.
    pub fn get(&self, series: &SeriesId) -> Option<Arc<HeldVolume>> {
        let (of, volume) = self.held.as_ref()?;
        (of == series).then(|| Arc::clone(volume))
    }

    /// The volume kept, whichever series it is of.
    pub fn kept(&self) -> Option<Arc<HeldVolume>> {
        self.held.as_ref().map(|(_, volume)| Arc::clone(volume))
    }

    /// The volume of a series, if that is the one kept and it holds the series' own values,
    /// not a map worked out from them. What works on a scan's values reads it here.
    pub fn own(&self, series: &SeriesId) -> Option<Arc<HeldVolume>> {
        self.get(series).filter(|_| !self.map)
    }

    /// Whether the volume of a series is the one kept.
    pub fn holds(&self, series: &SeriesId) -> bool {
        self.held.as_ref().is_some_and(|(of, _)| of == series)
    }

    /// Keep the volume of a series, in place of any kept before. The one kept before is
    /// handed back, with its series; a map kept before is not, its values not being its
    /// series' own.
    pub fn put(
        &mut self,
        series: SeriesId,
        volume: Arc<HeldVolume>,
    ) -> Option<(SeriesId, Arc<HeldVolume>)> {
        self.for_card = None;
        self.curve = None;
        self.preview = None;
        let was_map = std::mem::take(&mut self.map);
        self.held.replace((series, volume)).filter(|_| !was_map)
    }

    /// Keep a susceptibility map worked out from a series in place of its volume, as
    /// [`VolumeStore::put`] keeps a volume.
    pub fn put_map(&mut self, series: SeriesId, map: Arc<HeldVolume>) {
        self.put(series, map);
        self.map = true;
    }

    /// Give up the volume kept if it is a susceptibility map; whether it was.
    pub fn drop_map(&mut self) -> bool {
        let was_map = self.map;
        if was_map {
            self.clear();
        }
        was_map
    }

    /// Keep a copy of a series' volume halved to fit the graphics device, with how many
    /// times it was halved, if that series' volume is the one kept.
    pub fn put_for_card(&mut self, series: &SeriesId, copy: (Arc<HeldVolume>, u32)) {
        if self.holds(series) {
            self.for_card = Some(copy);
        }
    }

    /// The copy of a series' volume halved to fit the graphics device, and how many times it
    /// was halved; `None` when the device holds the volume itself, or another is kept.
    pub fn for_card(&self, series: &SeriesId) -> Option<(Arc<HeldVolume>, u32)> {
        let (copy, halved) = self.for_card.as_ref().filter(|_| self.holds(series))?;
        Some((Arc::clone(copy), *halved))
    }

    /// Say whether the curve through the volume kept is wanted. When it is not, the one
    /// kept is given up, and one that arrives later is not kept.
    pub fn want_smoothed(&mut self, wanted: bool) {
        self.curve_wanted = wanted;
        if !wanted {
            self.curve = None;
        }
    }

    /// Keep the curve through `of`, the volume of a series, if it is wanted and that very
    /// volume is still the one kept. Whether it was kept.
    pub fn put_smoothed(
        &mut self,
        series: &SeriesId,
        of: &Arc<HeldVolume>,
        curve: Smoothed,
    ) -> bool {
        let held = self.held.as_ref();
        let its_own = held.is_some_and(|(kept, volume)| kept == series && Arc::ptr_eq(volume, of));
        if its_own && self.curve_wanted {
            self.curve = Some(curve);
        }
        its_own && self.curve_wanted
    }

    /// Take the curve through a series' volume. It is handed over once: the graphics device
    /// then holds it, and it is as large as the volume.
    pub fn take_smoothed(&mut self, series: &SeriesId) -> Option<Smoothed> {
        if self.holds(series) {
            self.curve.take()
        } else {
            None
        }
    }

    /// Keep the preview of a series being built, in place of any kept before.
    pub fn put_preview(&mut self, series: SeriesId, preview: Volume) {
        self.preview = Some((series, preview));
    }

    /// Take the preview of a series, if that is the one kept. It is handed over once.
    pub fn take_preview(&mut self, series: &SeriesId) -> Option<Volume> {
        let (of, _) = self.preview.as_ref()?;
        if of != series {
            return None;
        }
        self.preview.take().map(|(_, preview)| preview)
    }

    /// The bytes of values the volumes kept hold in memory: the volume and any copy of it
    /// halved for the graphics card.
    pub fn bytes_held(&self) -> usize {
        let volume = self
            .held
            .as_ref()
            .map_or(0, |(_, volume)| volume.bytes_held());
        let copy = self
            .for_card
            .as_ref()
            .map_or(0, |(copy, _)| copy.bytes_held());
        volume.saturating_add(copy)
    }

    /// The bytes the graphics card holds of the volume kept: two a voxel of what it is given.
    pub fn card_bytes(&self) -> usize {
        let given = self
            .for_card
            .as_ref()
            .map(|(copy, _)| copy)
            .or(self.held.as_ref().map(|(_, volume)| volume));
        let voxels: usize = given.map_or(0, |given| {
            given
                .dims()
                .iter()
                .map(|&side| side as usize)
                .product::<usize>()
        });
        voxels.saturating_mul(2)
    }

    /// Keep nothing.
    pub fn clear(&mut self) {
        self.held = None;
        self.for_card = None;
        self.curve = None;
        self.preview = None;
        self.map = false;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_map_is_shown_in_place_of_its_series_but_is_never_taken_for_its_own_values() {
        let mut store = VolumeStore::default();
        let head = id(THICK_SCAN);
        store.put(head.clone(), volume(1.0));
        assert_eq!(value_of(store.own(&head)), Some(1.0));
        store.put_map(head.clone(), volume(7.0));
        assert_eq!(
            value_of(store.get(&head)),
            Some(7.0),
            "what is drawn is the map"
        );
        assert!(
            store.own(&head).is_none(),
            "but it is not the scan's own values"
        );
        let before = store.put(id(FINE_SCAN), volume(2.0));
        assert!(
            before.is_none(),
            "a map is not handed back as its series' slices"
        );
        assert_eq!(value_of(store.own(&id(FINE_SCAN))), Some(2.0));
        assert!(
            !store.drop_map(),
            "a series' own volume is not a map to give up"
        );
        store.put_map(head.clone(), volume(7.0));
        assert!(store.drop_map() && store.get(&head).is_none());
    }

    use volsa_core::{Affine, Grid, Volume};
    use volsa_render::MAX_VOLUME_BYTES;

    use super::*;
    use crate::testing::{FINE_SCAN, THICK_SCAN, held_of, id};

    fn plain(value: f32) -> Volume {
        Volume::new([1, 1, 2], vec![value; 2], Affine::identity()).expect("adds up")
    }

    fn volume(value: f32) -> Arc<HeldVolume> {
        Arc::new(held_of(&plain(value)))
    }

    /// The value a volume holds, all of its values being one.
    fn value_of(volume: Option<Arc<HeldVolume>>) -> Option<f32> {
        volume?.value(0, 0, 0)
    }

    #[test]
    fn a_volume_is_kept_under_its_series_until_cleared() {
        let mut store = VolumeStore::default();
        assert!(store.get(&id(THICK_SCAN)).is_none());
        store.put(id(THICK_SCAN), volume(1.0));
        assert_eq!(value_of(store.get(&id(THICK_SCAN))), Some(1.0));
        assert!(
            store.get(&id(FINE_SCAN)).is_none(),
            "it is not another series' volume"
        );
        store.clear();
        assert!(store.get(&id(THICK_SCAN)).is_none());
    }

    #[test]
    fn a_copy_for_the_card_is_kept_with_its_volume_and_goes_with_it() {
        let mut store = VolumeStore::default();
        let copy = || (volume(1.0), 1);
        store.put_for_card(&id(THICK_SCAN), copy());
        assert!(
            store.for_card(&id(THICK_SCAN)).is_none(),
            "with no volume, nothing"
        );
        store.put(id(THICK_SCAN), volume(1.0));
        store.put_for_card(&id(FINE_SCAN), copy());
        assert!(
            store.for_card(&id(FINE_SCAN)).is_none(),
            "nor for another series"
        );
        store.put_for_card(&id(THICK_SCAN), copy());
        let (kept, halved) = store.for_card(&id(THICK_SCAN)).expect("kept");
        assert_eq!((kept.dims(), halved), ([1, 1, 2], 1));
        assert!(store.for_card(&id(THICK_SCAN)).is_some(), "and still kept");
        store.put(id(FINE_SCAN), volume(2.0));
        assert!(
            store.for_card(&id(FINE_SCAN)).is_none(),
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
        let (was, gone) = before.expect("one gave way");
        assert_eq!((was, value_of(Some(gone))), (id(THICK_SCAN), Some(1.0)));
    }

    #[test]
    fn the_curve_is_kept_only_with_the_very_volume_it_runs_through_and_while_it_is_wanted() {
        let mut store = VolumeStore::default();
        let first = volume(1.0);
        let curve = || Smoothed::of(&plain(1.0), 2048, MAX_VOLUME_BYTES);
        let head = id(THICK_SCAN);
        store.put(head.clone(), Arc::clone(&first));
        assert!(
            !store.put_smoothed(&head, &first, curve()),
            "nobody wants it"
        );
        store.want_smoothed(true);
        assert!(!store.put_smoothed(&id(FINE_SCAN), &first, curve()));
        let alike = volume(1.0);
        assert!(
            !store.put_smoothed(&head, &alike, curve()),
            "alike is not the same"
        );
        assert!(store.put_smoothed(&head, &first, curve()));
        assert!(store.take_smoothed(&id(FINE_SCAN)).is_none());
        assert_eq!(store.take_smoothed(&head).expect("kept").dims(), [1, 1, 2]);
        assert!(store.take_smoothed(&head).is_none(), "handed over once");
    }

    #[test]
    fn a_newer_volume_leaves_no_curve_of_the_old_one_and_nor_does_turning_it_off() {
        let mut store = VolumeStore::default();
        let (first, head) = (volume(1.0), id(THICK_SCAN));
        let curve = || Smoothed::of(&plain(1.0), 2048, MAX_VOLUME_BYTES);
        store.want_smoothed(true);
        store.put(head.clone(), Arc::clone(&first));
        assert!(store.put_smoothed(&head, &first, curve()));
        let second = volume(2.0);
        store.put(head.clone(), Arc::clone(&second));
        assert!(
            store.take_smoothed(&head).is_none(),
            "it went with its volume"
        );
        assert!(
            !store.put_smoothed(&head, &first, curve()),
            "and cannot come back late"
        );
        assert!(store.put_smoothed(&head, &second, curve()));
        store.want_smoothed(false);
        assert!(
            store.take_smoothed(&head).is_none(),
            "given up when turned off"
        );
        store.want_smoothed(true);
        assert!(store.put_smoothed(&head, &second, curve()));
        store.clear();
        assert!(store.take_smoothed(&head).is_none());
    }

    #[test]
    fn a_second_volume_takes_the_place_of_the_first() {
        let mut store = VolumeStore::default();
        store.put(id(THICK_SCAN), volume(1.0));
        store.put(id(FINE_SCAN), volume(2.0));
        assert!(store.get(&id(THICK_SCAN)).is_none());
        assert_eq!(value_of(store.get(&id(FINE_SCAN))), Some(2.0));
    }
}
