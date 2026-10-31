//! The memory budget as the workers keep to it: the frame cache is given what the volumes
//! kept leave of it, and what goes past it is counted, for slabs to be put away.

use super::{Shared, Workers, lock};
use crate::budget::{self, Held};

impl Shared {
    /// Give the frame cache what the budget has over once the volumes kept are counted.
    pub(super) fn share_the_budget(&self) {
        let held = lock(&self.volumes).bytes_held();
        lock(&self.cache).set_budget(budget::cache_share(self.budget, held));
    }

    /// How many bytes what is held goes past the budget: the values of the volumes kept that
    /// are in memory, the frame cache, and the card's copy where the card shares memory.
    pub(super) fn over_budget(&self) -> usize {
        let store = lock(&self.volumes);
        let (values, card) = (store.bytes_held(), store.card_bytes());
        drop(store);
        let cache = lock(&self.cache).bytes();
        let shares = lock(&self.card_room).shares_memory;
        budget::over(
            self.budget,
            Held {
                values,
                cache,
                card,
            },
            shares,
        )
    }
}

impl Workers {
    /// How many bytes what Voxa holds goes past its memory budget; 0 within it.
    pub fn over_budget(&self) -> usize {
        self.shared.over_budget()
    }
}
