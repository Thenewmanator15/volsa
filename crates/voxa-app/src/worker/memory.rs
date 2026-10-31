//! The memory budget as the workers keep to it: the frame cache is given what the volumes
//! kept leave of it, and what goes past it is counted, for slabs to be put away.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use voxa_core::HeldVolume;

use super::{Shared, Workers, lock};
use crate::action::Action;
use crate::budget::{self, Held};
use crate::spill::SpillFile;
use crate::state::{Notice, NoticeKind};

/// What tells how much more memory the system can give Voxa now.
pub(super) type FreeMemorySource = Box<dyn Fn() -> voxa_models::FreeMemory + Send + Sync>;

/// What is kept free for everything else besides a volume being built: 512 MiB.
const MARGIN: usize = 512 << 20;

/// What is said when a volume past the budget cannot be put on disk.
const NO_ROOM_ON_DISK: &str =
    "Voxa could not make room on disk, so this series uses more memory than the amount set.";

impl Shared {
    /// Give the frame cache what the budget has over once the volumes kept are counted.
    pub(super) fn share_the_budget(&self) {
        let held = lock(&self.volumes).bytes_held();
        lock(&self.cache).set_budget(budget::cache_share(self.budget, held));
    }

    /// How many bytes what is held goes past the budget: the values of the volumes kept that
    /// are in memory, the frame cache, and the card's copy where the card shares memory.
    pub(super) fn over_budget(&self) -> usize {
        self.over_budget_with(0)
    }

    /// As [`Shared::over_budget`], with `building` more bytes of a volume not yet kept.
    fn over_budget_with(&self, building: usize) -> usize {
        let store = lock(&self.volumes);
        let (values, card) = (store.bytes_held() + building, store.card_bytes());
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

impl Shared {
    /// The most bytes in one slab of a volume built.
    pub(super) fn slab_bytes(&self) -> usize {
        self.slab_bytes.load(Ordering::SeqCst)
    }

    /// Put whole slabs of `held` away, the least used first, until what is held is within the
    /// budget: `building` when `held` is not yet the volume kept, and counts on top of it.
    /// Where they cannot be put away, `held` stays in memory and that is said once a build.
    pub(super) fn keep_within(&self, held: &HeldVolume, building: bool) {
        loop {
            let more = if building { held.bytes_held() } else { 0 };
            if self.over_budget_with(more) == 0 || !self.make_room(held) {
                return;
            }
        }
    }

    /// Put one slab of `held` away, making its spill file first if it has none. Whether one
    /// went.
    fn make_room(&self, held: &HeldVolume) -> bool {
        // A run that keeps nothing on this computer has nowhere to put them, and says nothing.
        let Some(local) = &self.settings else {
            return false;
        };
        // One at a time: a thread that finds another at it leaves the rest to that one.
        let Ok(_turn) = self.putting_away.try_lock() else {
            return false;
        };
        if !held.keeps_away() {
            let me = self.me.clone();
            match SpillFile::create(local, held.slab_bytes()) {
                Ok(file) => held.keep_away_in(Arc::new(file.when_brought_back(move || {
                    within_budget_later(&me);
                }))),
                Err(why) => {
                    tracing::warn!("no spill file could be made: {}", why.kind());
                    self.no_room_on_disk();
                    return false;
                }
            }
        }
        match held.put_away_least_used() {
            Ok(freed) => freed > 0,
            Err(why) => {
                tracing::warn!("a slab could not be put on disk: {}", why.kind());
                self.no_room_on_disk();
                false
            }
        }
    }

    /// Say, once a build, that its volume could not be put on disk.
    fn no_room_on_disk(&self) {
        if !self.told_no_room.swap(true, Ordering::SeqCst) {
            self.report(Action::Post(Notice {
                kind: NoticeKind::Warning,
                text: NO_ROOM_ON_DISK.to_owned(),
                details: Vec::new(),
                key: Some("spill"),
            }));
        }
    }
}

/// Bring the volume kept back within the budget on another thread: a slab was brought back
/// into memory where it was read, which may be the window's thread.
fn within_budget_later(me: &Weak<Shared>) {
    let Some(shared) = me.upgrade() else {
        return;
    };
    rayon::spawn(move || {
        let kept = lock(&shared.volumes).kept();
        if let Some(volume) = kept {
            shared.keep_within(&volume, false);
        }
    });
}

impl Shared {
    /// Why a volume of `values` bytes cannot be built now, in a sentence: the system cannot
    /// give Voxa what it will hold, the values or the budget if that is less, a slab more, and
    /// a margin. `None` when it can, or when the system does not say.
    pub(super) fn too_little_memory(&self, values: usize) -> Option<String> {
        let needed = values.min(self.budget) + self.slab_bytes() + MARGIN;
        let free = (lock(&self.free_memory))();
        let (has, said) = match free {
            voxa_models::FreeMemory::Promised(bytes) => (bytes, "Windows can promise Voxa"),
            voxa_models::FreeMemory::Physical(bytes) => (bytes, "the computer has free"),
            voxa_models::FreeMemory::Unknown => return None,
        };
        let has = usize::try_from(has).unwrap_or(usize::MAX);
        (has < needed).then(|| {
            format!(
                "Not enough memory to show this series in 3D: it needs {} GB, and {said} {} GB. \
                 Close other programs and open it again.",
                gigabytes(needed),
                gigabytes(has)
            )
        })
    }

    /// Whether a 32-bit copy of `voxels` values, which the processor's curve is made from,
    /// would pass the budget.
    pub(super) fn copy_passes_budget(&self, voxels: usize) -> bool {
        voxels.saturating_mul(size_of::<f32>()) > self.budget
    }
}

/// Bytes in gigabytes of 1,024 MiB, to one decimal place, as Windows counts them.
fn gigabytes(bytes: usize) -> String {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a memory size said to one decimal place"
    )]
    let gigabytes = bytes as f64 / f64::from(1_u32 << 30);
    format!("{gigabytes:.1}")
}

impl Workers {
    /// Have the workers ask `free` how much memory the system can give, in place of the
    /// system itself, for tests.
    #[cfg(test)]
    pub(crate) fn set_free_memory(
        &self,
        free: impl Fn() -> voxa_models::FreeMemory + Send + Sync + 'static,
    ) {
        *lock(&self.shared.free_memory) = Box::new(free);
    }

    /// Build volumes in slabs of at most `bytes`, for tests that want many slabs.
    #[cfg(test)]
    pub(crate) fn set_slab_bytes(&self, bytes: usize) {
        self.shared.slab_bytes.store(bytes, Ordering::SeqCst);
    }
}
