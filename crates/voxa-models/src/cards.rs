//! The graphics cards of this computer, as the system lists them, and what each has to spare;
//! and how much ordinary memory the system can still give this program.
//!
//! This is the one module of the crate that may use `unsafe`: on Windows the list is DXGI's,
//! and every call into it is marked unsafe, as is the call that says how much memory is left.
//! Each use says why it is sound. On a system with no such list there are no cards and
//! nothing is known of their memory.
//!
//! A card is known by what it is, a [`CardId`], and not by where it stands in the list. The
//! list changes its order while a program runs, and a place kept from before is then another
//! card's. Only this module turns a card into a place, at the moment the place is used.

#[cfg(test)]
mod tests;

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

/// What a graphics card is, as the system itself tells its cards apart. It is the same for
/// one card wherever the card stands in the system's list, and for as long as the computer
/// runs; after a restart it may be another, so it is not for keeping in a file. The number
/// means nothing in itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardId(pub u64);

/// One graphics card, as the system describes it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// What the card is. This is what a card is asked for by
    /// ([`Device::Card`](crate::Device::Card)), and what is kept of it.
    pub id: CardId,
    /// Its place in the system's list at the moment the list was read: for showing, and for
    /// this module's own use. It must never be kept. The list changes its order, and the
    /// place is then another card's.
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

/// Where a card stands now: its place in `cards`, a list as the system has just given it.
/// The first card of that identity, when the list holds it twice. `None` when the list no
/// longer holds the card.
pub fn place_of(cards: &[Card], id: CardId) -> Option<u32> {
    let card = cards.iter().find(|card| card.id == id);
    card.map(|card| card.index)
}

/// The place a card has in the system's list at this moment; `None` when it is not listed.
/// For this crate's own use, at the moment the place is needed.
pub(crate) fn place_now(id: CardId) -> Option<u32> {
    place_of(&cards(), id)
}

/// How many bytes the system lets this program use on a card now. The figure already allows
/// for what other programs hold on the card. `None` where it cannot be read: the card is no
/// longer listed, or the system keeps no such count.
pub fn budget(id: CardId) -> Option<u64> {
    place_now(id).and_then(system::budget)
}

/// How many bytes of a card this program holds now. For integrated graphics, which have
/// next to none of their own, it is the computer's memory they hold for this program. `None`
/// where it cannot be read, as for [`budget`].
pub fn in_use(id: CardId) -> Option<u64> {
    place_now(id).and_then(system::in_use)
}

/// How much more ordinary memory this program can have now, as far as the system says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeMemory {
    /// What the system can still promise this program, in bytes: on Windows the commit still
    /// available to it. It is what an allocation that is refused runs into.
    Promised(u64),
    /// The free physical memory, in bytes: all that could be read on a system that does not
    /// say what it can still promise.
    Physical(u64),
    /// Nothing could be read.
    Unknown,
}

/// How much more ordinary memory this program can have now: on Windows the commit still
/// available to it; elsewhere the free physical memory, where that can be read.
pub fn free_memory() -> FreeMemory {
    system::free_memory()
}

/// The memory Linux says is available, in bytes, from the text of its `/proc/meminfo`: the
/// line `MemAvailable`, which is in kibibytes. `None` when there is no such line or it holds
/// no such figure.
#[cfg_attr(all(windows, not(test)), allow(dead_code))]
pub(crate) fn available_in(meminfo: &str) -> Option<u64> {
    let line = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:"))?;
    let kibibytes: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
    kibibytes.checked_mul(1024)
}

/// A card's identity from the two halves of the number the system gives it.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) fn identity(high: i32, low: u32) -> CardId {
    CardId(u64::from(high.cast_unsigned()) << 32 | u64::from(low))
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
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::core::Interface;

    use super::{Card, FreeMemory, identity, kind};

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
            let luid = described.AdapterLuid;
            found.push(Card {
                id: identity(luid.HighPart, luid.LowPart),
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

    /// The commit still available to this program: what the system says this process can
    /// still commit, which is never more than the whole system has left to commit.
    pub(super) fn free_memory() -> FreeMemory {
        let mut status = MEMORYSTATUSEX {
            // The call reads the length first, to know the structure it is given.
            dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or_default(),
            ..MEMORYSTATUSEX::default()
        };
        // SAFETY: `status` is a plain structure of ours that outlives the call, with its
        // length set as the call asks. The call reads that length and writes the rest.
        match unsafe { GlobalMemoryStatusEx(&raw mut status) } {
            Ok(()) => FreeMemory::Promised(status.ullAvailPageFile),
            Err(_) => FreeMemory::Unknown,
        }
    }
}

#[cfg(not(windows))]
mod system {
    use super::{Card, FreeMemory, available_in};

    pub(super) fn cards() -> Vec<Card> {
        Vec::new()
    }

    pub(super) fn budget(_index: u32) -> Option<u64> {
        None
    }

    pub(super) fn in_use(_index: u32) -> Option<u64> {
        None
    }

    /// The memory Linux says is available, where its list can be read. What the system
    /// would still promise is not what refuses an allocation there, so it is not read.
    pub(super) fn free_memory() -> FreeMemory {
        let listed = std::fs::read_to_string("/proc/meminfo").ok();
        match listed.as_deref().and_then(available_in) {
            Some(bytes) => FreeMemory::Physical(bytes),
            None => FreeMemory::Unknown,
        }
    }
}
