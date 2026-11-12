//! A list of measurements and what is kept for its series: nothing is written where they are
//! kept until what is there has been read, and what is read is put before whatever was
//! measured while it was on its way.

use super::{Annotation, Annotations, MAX_ANNOTATIONS};

impl Annotations {
    /// Whether what is kept for the series has been asked for and has not come yet. Until it
    /// comes the list is not written where it is kept: what is there would be written over
    /// unread.
    pub fn awaits_kept(&self) -> bool {
        self.awaited
    }

    /// Note that what is kept for the series has been asked for. Only a list nothing has been
    /// done to starts waiting: one taken back with its history already holds what was kept.
    pub(crate) fn await_kept(&mut self) {
        self.awaited = self.awaited || self.untouched();
    }

    /// Take in what was kept for the series. A list nothing has been done to becomes what was
    /// kept, with nothing to undo. A list changed while what was kept was on its way has the
    /// kept ones put before its own, at every step of its history, and its own numbered on
    /// from theirs; past [`MAX_ANNOTATIONS`] the newest are left out. A changed list that was
    /// not waiting holds what was kept already and is left as it is.
    ///
    /// True when the list was joined with what was kept, and so is to be written.
    pub(crate) fn take_in(&mut self, mut kept: Vec<Annotation>) -> bool {
        kept.truncate(MAX_ANNOTATIONS);
        if self.untouched() {
            *self = Self::loaded(kept);
            return false;
        }
        if !std::mem::take(&mut self.awaited) {
            return false;
        }
        let after = kept.iter().map(|item| item.id).max().unwrap_or(0);
        let joined = |own: &mut Vec<Annotation>| {
            for item in own.iter_mut() {
                // A name never given follows the number.
                let unnamed = item.label == item.default_label();
                item.id = item.id.saturating_add(after);
                if unnamed {
                    item.label = item.default_label();
                }
            }
            let mut all = kept.clone();
            all.append(own);
            all.truncate(MAX_ANNOTATIONS);
            *own = all;
        };
        joined(&mut self.items);
        self.undo.iter_mut().for_each(&joined);
        self.redo.iter_mut().for_each(&joined);
        self.numbered = self.numbered.saturating_add(after);
        true
    }

    /// Whether nothing has been done to the list: it is empty, with nothing to undo or do again.
    fn untouched(&self) -> bool {
        self.items.is_empty() && self.undo.is_empty() && self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::super::Shape;
    use super::*;
    use crate::measuring::Scale;

    fn marker() -> Shape {
        Shape::Marker {
            at: [1.0, 2.0, 3.0],
        }
    }

    fn kept(id: u64, label: &str) -> Annotation {
        Annotation {
            id,
            label: label.to_owned(),
            shape: marker(),
            scale: Scale::Patient,
        }
    }

    fn said(list: &Annotations) -> Vec<(u64, &str)> {
        let items = list.items.iter();
        items.map(|item| (item.id, item.label.as_str())).collect()
    }

    #[test]
    fn a_list_nothing_was_done_to_becomes_what_was_kept_and_is_not_written_again() {
        let mut list = Annotations::default();
        list.await_kept();
        assert!(list.awaits_kept());
        assert!(!list.take_in(vec![kept(4, "the tip")]), "nothing to write");
        assert_eq!(said(&list), [(4, "the tip")]);
        assert!(!list.awaits_kept() && !list.can_undo());
        // What is added next is numbered on from what was kept.
        list.add(marker(), Scale::Patient);
        assert_eq!(said(&list), [(4, "the tip"), (5, "Marker 5")]);
    }

    #[test]
    fn what_was_measured_before_the_kept_ones_came_is_put_after_them_and_numbered_on() {
        let mut list = Annotations::default();
        list.await_kept();
        list.add(marker(), Scale::Patient);
        list.add(marker(), Scale::Patient);
        list.rename(2, "mine");
        assert!(list.awaits_kept(), "still waiting, whatever is done to it");
        assert!(list.take_in(vec![kept(3, "old"), kept(7, "older")]));
        assert_eq!(
            said(&list),
            [(3, "old"), (7, "older"), (8, "Marker 8"), (9, "mine")]
        );
        assert!(!list.awaits_kept());
        // Every step back still holds what was kept: undoing all of it leaves the kept ones.
        while list.undo() {}
        assert_eq!(said(&list), [(3, "old"), (7, "older")]);
        while list.redo() {}
        assert_eq!(said(&list).len(), 4);
        list.add(marker(), Scale::Patient);
        assert_eq!(list.items.last().map(|item| item.id), Some(10));
    }

    #[test]
    fn a_changed_list_that_was_not_waiting_is_left_as_it_is() {
        // Taken back with its history: it held what was kept when it was put away.
        let mut list = Annotations::loaded(vec![kept(1, "old")]);
        list.clear();
        list.await_kept();
        assert!(!list.awaits_kept(), "it has been read already");
        assert!(!list.take_in(vec![kept(1, "old")]));
        assert_eq!(said(&list), []);
        assert!(list.can_undo());
    }

    #[test]
    fn asking_again_does_not_stop_a_changed_list_waiting() {
        let mut list = Annotations::default();
        list.await_kept();
        list.add(marker(), Scale::Patient);
        list.await_kept();
        assert!(list.awaits_kept());
    }

    #[test]
    fn joined_lists_hold_no_more_than_a_list_may() {
        let mut list = Annotations::default();
        list.await_kept();
        list.add(marker(), Scale::Patient);
        let many = (1..=MAX_ANNOTATIONS as u64).map(|id| kept(id, "old"));
        assert!(list.take_in(many.collect()));
        assert_eq!(list.items.len(), MAX_ANNOTATIONS);
        assert!(list.items.iter().all(|item| item.label == "old"));
    }
}
