//! The graphics cards of this computer, as the system lists them, and what each has to spare.
//!
//! This is the one module of the crate that may use `unsafe`: on Windows the list is DXGI's,
//! and every call into it is marked unsafe. Each use says why it is sound. On a system with
//! no such list there are no cards and nothing is known of their memory.

/// A card with less memory of its own than this shares the computer's: 2 GiB.
const INTEGRATED_BELOW: u64 = 2 << 30;

/// What sort of graphics card one is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardKind {
    /// A card with memory of its own.
    Discrete,
    /// Graphics that share the computer's ordinary memory.
    Integrated,
    /// No card at all: the system drawing with the processor.
    Software,
}

/// One graphics card, as the system describes it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Its place in the system's list now, which is what a network is opened with
    /// ([`Device::Card`](crate::Device::Card)). The list can change from one minute to the
    /// next, so a card is found again by its name, never by a number kept from before.
    pub index: u32,
    /// The name the system gives it. It is never empty. The list can hold one card twice.
    pub name: String,
    /// What sort of card it is.
    pub kind: CardKind,
    /// How much memory it has that is its own, in bytes.
    pub own_bytes: u64,
}

/// The graphics cards, in the system's own order; empty where there is no such list. A card
/// the system gives no name is left out, since it could not be found again.
pub fn cards() -> Vec<Card> {
    system::cards()
}

/// How many bytes the system lets this program use on the card at that place in its list
/// now. The figure already allows for what other programs hold on the card. `None` where it
/// cannot be read: no card at that place, or a system that keeps no such count.
pub fn budget(index: u32) -> Option<u64> {
    system::budget(index)
}

/// How many bytes of the card at that place in the system's list this program holds now. For
/// integrated graphics, which have next to none of their own, it is the computer's memory
/// they hold for this program. `None` where it cannot be read, as for [`budget`].
pub fn in_use(index: u32) -> Option<u64> {
    system::in_use(index)
}

/// What sort of card one is, from what the system says of it: one the system marks as
/// software is that whatever its memory, and one with little memory of its own is
/// integrated.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) fn kind(software: bool, own_bytes: u64) -> CardKind {
    if software {
        CardKind::Software
    } else if own_bytes < INTEGRATED_BELOW {
        CardKind::Integrated
    } else {
        CardKind::Discrete
    }
}

#[cfg(windows)]
mod system {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3, IDXGIFactory1,
    };
    use windows::core::Interface;

    use super::{Card, kind};

    /// More adapters than any computer lists: the walk of the list ends here at the latest.
    const MOST_CARDS: u32 = 64;

    /// The system's list of adapters as it is now.
    fn list() -> Option<IDXGIFactory1> {
        // SAFETY: the call takes nothing of ours. It gives back an error or a counted
        // reference, which the wrapper releases when it is dropped.
        unsafe { CreateDXGIFactory1() }.ok()
    }

    pub(super) fn cards() -> Vec<Card> {
        let Some(list) = list() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for index in 0..MOST_CARDS {
            // SAFETY: `list` is a live interface. The call gives back an error (past the end
            // of the list) or a counted reference that the wrapper releases when dropped.
            let Ok(adapter) = (unsafe { list.EnumAdapters1(index) }) else {
                break;
            };
            // SAFETY: `adapter` is a live interface. The call fills a plain structure of its
            // own and returns it by value.
            let Ok(described) = (unsafe { adapter.GetDesc1() }) else {
                continue;
            };
            let name = name_of(&described.Description);
            if name.is_empty() {
                continue;
            }
            let software = u32::try_from(DXGI_ADAPTER_FLAG_SOFTWARE.0)
                .is_ok_and(|flag| described.Flags & flag != 0);
            let own_bytes = u64::try_from(described.DedicatedVideoMemory).unwrap_or(u64::MAX);
            found.push(Card {
                index,
                name,
                kind: kind(software, own_bytes),
                own_bytes,
            });
        }
        found
    }

    pub(super) fn budget(index: u32) -> Option<u64> {
        memory(index).map(|memory| memory.Budget)
    }

    pub(super) fn in_use(index: u32) -> Option<u64> {
        memory(index).map(|memory| memory.CurrentUsage)
    }

    /// What the system says of this program's memory on the card at that place, now.
    fn memory(index: u32) -> Option<DXGI_QUERY_VIDEO_MEMORY_INFO> {
        let list = list()?;
        // SAFETY: as in `cards`: a live interface, and an error or a counted reference back.
        let adapter = unsafe { list.EnumAdapters1(index) }.ok()?;
        // The budget is kept by a later form of the adapter's interface.
        let adapter: IDXGIAdapter3 = adapter.cast().ok()?;
        let mut memory = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // SAFETY: `adapter` is a live interface, and `memory` is a plain structure of ours
        // that outlives the call, which only writes to it. Node 0 is the card itself.
        unsafe {
            adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &raw mut memory)
        }
        .ok()?;
        Some(memory)
    }

    /// The name in a description: the 16-bit units up to the first zero, without the space
    /// around them.
    fn name_of(units: &[u16]) -> String {
        let end = units.iter().position(|&unit| unit == 0);
        let named = end.and_then(|end| units.get(..end)).unwrap_or(units);
        String::from_utf16_lossy(named).trim().to_owned()
    }
}

#[cfg(not(windows))]
mod system {
    use super::Card;

    pub(super) fn cards() -> Vec<Card> {
        Vec::new()
    }

    pub(super) fn budget(_index: u32) -> Option<u64> {
        None
    }

    pub(super) fn in_use(_index: u32) -> Option<u64> {
        None
    }
}
