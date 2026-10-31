//! Decoded slices kept in memory, within a budget.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use voxa_core::SeriesId;
use voxa_dicom::Frame;

/// Decoded slices, kept until their memory is needed for slices nearer the one being shown.
#[derive(Debug)]
pub struct FrameCache {
    budget: usize,
    bytes: usize,
    frames: HashMap<SeriesId, BTreeMap<usize, Arc<Frame>>>,
    /// The slices held that were put by without their windows being said: those of a
    /// volume that gave way or failed. Their windows are said when they are shown.
    unannounced: HashSet<(SeriesId, usize)>,
}

impl FrameCache {
    /// An empty cache that tries to hold no more than `budget_bytes` of pixel values.
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget: budget_bytes,
            bytes: 0,
            frames: HashMap::new(),
            unannounced: HashSet::new(),
        }
    }

    /// The most pixel values, in bytes, the cache tries to hold.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// The pixel values held now, in bytes.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Hold no more than `budget_bytes` from now on, letting slices go until the cache is
    /// within it: a whole series at a time, the one with the fewest slices held first.
    pub fn set_budget(&mut self, budget_bytes: usize) {
        self.budget = budget_bytes;
        while self.bytes > self.budget {
            let fewest = self
                .frames
                .iter()
                .min_by_key(|(_, slices)| slices.len())
                .map(|(series, _)| series.clone());
            let Some(series) = fewest else {
                break;
            };
            let slices = self.frames.get_mut(&series);
            let last = slices
                .as_ref()
                .and_then(|slices| slices.keys().next_back().copied());
            let gone = slices
                .zip(last)
                .and_then(|(slices, last)| slices.remove(&last));
            self.bytes -= gone.as_deref().map_or(0, size_of);
            self.frames.retain(|_, slices| !slices.is_empty());
        }
        let frames = &self.frames;
        self.unannounced
            .retain(|(series, slice)| frames.get(series).is_some_and(|it| it.contains_key(slice)));
    }

    /// A slice, if it is held.
    pub fn get(&self, series: &SeriesId, slice: usize) -> Option<Arc<Frame>> {
        self.frames.get(series)?.get(&slice).cloned()
    }

    /// Whether a slice is held.
    pub fn contains(&self, series: &SeriesId, slice: usize) -> bool {
        self.frames
            .get(series)
            .is_some_and(|slices| slices.contains_key(&slice))
    }

    /// Forget every slice of one series.
    pub fn forget(&mut self, series: &SeriesId) {
        let gone = self.frames.remove(series);
        let freed: usize = gone
            .iter()
            .flat_map(BTreeMap::values)
            .map(|frame| size_of(frame))
            .sum();
        self.bytes -= freed;
        self.unannounced.retain(|(of, _)| of != series);
    }

    /// Forget every slice.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.unannounced.clear();
        self.bytes = 0;
    }

    /// Hold a slice. If that takes the cache over its budget, slices are dropped until it is
    /// within it again: those of other series first, then those of this series farthest from
    /// `keep_near`, the slice being shown. The slice just put in is never the one dropped, so
    /// even a budget smaller than one slice leaves something to show.
    pub fn insert(&mut self, series: &SeriesId, slice: usize, frame: Arc<Frame>, keep_near: usize) {
        self.bytes += size_of(&frame);
        let replaced = self
            .frames
            .entry(series.clone())
            .or_default()
            .insert(slice, frame);
        self.bytes -= replaced.as_deref().map_or(0, size_of);
        while self.bytes > self.budget {
            let Some((from, gone)) = self.farthest(series, slice, keep_near) else {
                break;
            };
            let dropped = self
                .frames
                .get_mut(&from)
                .and_then(|slices| slices.remove(&gone));
            self.bytes -= dropped.as_deref().map_or(0, size_of);
            self.unannounced.remove(&(from, gone));
        }
        self.frames.retain(|_, slices| !slices.is_empty());
        self.unannounced.remove(&(series.clone(), slice));
    }

    /// Hold a slice, as [`FrameCache::insert`] does, whose windows have not been said: it is
    /// given once by [`FrameCache::take_unannounced`], so that they are said when it is shown.
    pub fn insert_unannounced(
        &mut self,
        series: &SeriesId,
        slice: usize,
        frame: Arc<Frame>,
        keep_near: usize,
    ) {
        self.insert(series, slice, frame, keep_near);
        if self.contains(series, slice) {
            self.unannounced.insert((series.clone(), slice));
        }
    }

    /// A slice held whose windows have not been said, once: after this they count as said.
    pub fn take_unannounced(&mut self, series: &SeriesId, slice: usize) -> Option<Arc<Frame>> {
        let key = (series.clone(), slice);
        self.unannounced
            .remove(&key)
            .then(|| self.get(series, slice))?
    }

    /// The slice to drop next: any but the newest, another series' before this one's, then the
    /// one farthest from the slice being shown.
    fn farthest(
        &self,
        series: &SeriesId,
        newest: usize,
        keep_near: usize,
    ) -> Option<(SeriesId, usize)> {
        let held = self
            .frames
            .iter()
            .flat_map(|(id, slices)| slices.keys().map(move |slice| (id, *slice)));
        held.filter(|(id, slice)| (*id, *slice) != (series, newest))
            .max_by_key(|(id, slice)| (*id != series, slice.abs_diff(keep_near)))
            .map(|(id, slice)| (id.clone(), slice))
    }
}

/// The memory one frame's values take.
fn size_of(frame: &Frame) -> usize {
    std::mem::size_of_val(frame.values.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::id;

    /// Bytes held by one 4 × 4 frame: sixteen values of four bytes.
    const ONE: usize = 64;

    fn frame(marker: f32) -> Arc<Frame> {
        Arc::new(Frame {
            rows: 4,
            cols: 4,
            values: vec![marker; 16],
            colour: None,
        })
    }

    fn held(cache: &FrameCache, series: &str, slices: std::ops::Range<usize>) -> Vec<usize> {
        slices
            .filter(|slice| cache.get(&id(series), *slice).is_some())
            .collect()
    }

    #[test]
    fn a_frame_put_by_unannounced_is_given_once_to_be_announced() {
        let mut cache = FrameCache::new(10 * ONE);
        cache.insert(&id("a"), 0, frame(1.0), 0);
        cache.insert_unannounced(&id("a"), 1, frame(2.0), 0);
        assert!(
            cache.take_unannounced(&id("a"), 0).is_none(),
            "said already"
        );
        let first = cache
            .take_unannounced(&id("a"), 1)
            .map(|frame| frame.values[0]);
        assert_eq!(first, Some(2.0));
        assert!(cache.take_unannounced(&id("a"), 1).is_none(), "once only");
        assert!(cache.contains(&id("a"), 1), "and it is still held");
        cache.insert_unannounced(&id("a"), 2, frame(3.0), 0);
        cache.forget(&id("a"));
        cache.insert(&id("a"), 2, frame(3.0), 0);
        assert!(
            cache.take_unannounced(&id("a"), 2).is_none(),
            "a frame forgotten and put back as said is not unannounced"
        );
    }

    #[test]
    fn a_lower_budget_lets_slices_go_until_the_cache_is_within_it() {
        let mut cache = FrameCache::new(10 * ONE);
        for slice in 0..10 {
            cache.insert(&id("a"), slice, frame(0.0), 0);
        }
        cache.set_budget(4 * ONE);
        assert_eq!((cache.bytes(), cache.budget()), (4 * ONE, 4 * ONE));
        cache.set_budget(20 * ONE);
        assert_eq!(cache.bytes(), 4 * ONE, "a higher one takes nothing back");
    }

    #[test]
    fn frames_within_the_budget_are_all_kept() {
        let mut cache = FrameCache::new(10 * ONE);
        for slice in 0..10 {
            cache.insert(&id("a"), slice, frame(0.0), 0);
        }
        assert_eq!(held(&cache, "a", 0..10).len(), 10);
        assert_eq!((cache.bytes(), cache.budget()), (10 * ONE, 10 * ONE));
    }

    #[test]
    fn a_frame_larger_than_the_whole_budget_is_still_kept_while_it_is_the_newest() {
        // Review focus 5: a budget smaller than one frame must not mean nothing can be shown.
        let mut cache = FrameCache::new(ONE - 1);
        cache.insert(&id("a"), 3, frame(7.0), 3);
        assert_eq!(cache.get(&id("a"), 3), Some(frame(7.0)));
        assert_eq!(cache.bytes(), ONE);
        cache.insert(&id("a"), 4, frame(8.0), 4);
        assert_eq!(
            held(&cache, "a", 0..10),
            vec![4],
            "the newest replaces the one before"
        );
    }

    #[test]
    fn the_frames_farthest_from_the_slice_shown_go_first() {
        let mut cache = FrameCache::new(3 * ONE);
        for slice in 0..10 {
            cache.insert(&id("a"), slice, frame(0.0), 9);
        }
        assert_eq!(held(&cache, "a", 0..10), vec![7, 8, 9]);
        assert_eq!(cache.bytes(), 3 * ONE);
    }

    #[test]
    fn a_frame_far_from_the_slice_shown_is_kept_only_as_the_one_just_put_in() {
        let mut cache = FrameCache::new(2 * ONE);
        cache.insert(&id("a"), 10, frame(0.0), 10);
        cache.insert(&id("a"), 11, frame(0.0), 10);
        cache.insert(&id("a"), 40, frame(0.0), 10);
        assert_eq!(
            held(&cache, "a", 0..50),
            vec![10, 40],
            "11 went: 40 is exempt this once"
        );
        cache.insert(&id("a"), 9, frame(0.0), 10);
        assert_eq!(held(&cache, "a", 0..50), vec![9, 10]);
    }

    #[test]
    fn frames_of_other_series_go_before_any_of_the_series_being_filled() {
        let mut cache = FrameCache::new(3 * ONE);
        cache.insert(&id("old"), 0, frame(0.0), 0);
        cache.insert(&id("old"), 1, frame(0.0), 0);
        for slice in 0..3 {
            cache.insert(&id("new"), slice, frame(0.0), 0);
        }
        assert_eq!(held(&cache, "old", 0..2), Vec::<usize>::new());
        assert_eq!(held(&cache, "new", 0..3), vec![0, 1, 2]);
    }

    #[test]
    fn putting_a_slice_in_again_replaces_it_without_counting_it_twice() {
        let mut cache = FrameCache::new(10 * ONE);
        cache.insert(&id("a"), 0, frame(1.0), 0);
        cache.insert(&id("a"), 0, frame(2.0), 0);
        assert_eq!(cache.get(&id("a"), 0), Some(frame(2.0)));
        assert_eq!(cache.bytes(), ONE);
        assert!(cache.contains(&id("a"), 0) && !cache.contains(&id("a"), 1));
    }

    #[test]
    fn forgetting_a_series_drops_its_frames_and_leaves_the_others() {
        let mut cache = FrameCache::new(10 * ONE);
        cache.insert(&id("a"), 0, frame(1.0), 0);
        cache.insert(&id("a"), 1, frame(2.0), 0);
        cache.insert(&id("b"), 0, frame(3.0), 0);
        cache.forget(&id("a"));
        assert_eq!(held(&cache, "a", 0..2), Vec::<usize>::new());
        assert_eq!(
            (cache.bytes(), cache.get(&id("b"), 0)),
            (ONE, Some(frame(3.0)))
        );
        cache.forget(&id("a"));
        assert_eq!(
            cache.bytes(),
            ONE,
            "forgetting what is not held changes nothing"
        );
    }

    #[test]
    fn clearing_forgets_every_frame() {
        let mut cache = FrameCache::new(10 * ONE);
        cache.insert(&id("a"), 0, frame(1.0), 0);
        cache.clear();
        assert_eq!((cache.bytes(), cache.get(&id("a"), 0)), (0, None));
    }
}
