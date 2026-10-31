//! One memory budget for the decoded values Voxa keeps in ordinary memory: the volume of the
//! series in 3D, the slices decoded for the 2D viewports and, on a graphics card that shares
//! the computer's memory, the card's copy of the volume. By default 2 GiB, or a quarter of the
//! computer's memory where that is less.

/// The least the frame cache is given, however much a volume holds: enough slices to scroll.
pub const CACHE_FLOOR: usize = 64 << 20;
/// The most the budget is unless the person sets it.
const MOST_BY_DEFAULT: usize = 2 << 30;

/// The variable that sets the budget, in mebibytes, for measuring Voxa: it comes before
/// what the preferences say.
pub const VARIABLE: &str = "VOXA_BUDGET_MIB";

/// The budget asked for in [`VARIABLE`], in mebibytes, if it is a number.
pub fn asked() -> Option<u32> {
    std::env::var(VARIABLE).ok()?.trim().parse().ok()
}

/// The budget for a computer of `physical_bytes` of memory, where that is known: 2 GiB, or a
/// quarter of its memory if that is less.
pub fn default_for(physical_bytes: Option<u64>) -> usize {
    let quarter = physical_bytes.map(|bytes| usize::try_from(bytes / 4).unwrap_or(usize::MAX));
    quarter.map_or(MOST_BY_DEFAULT, |quarter| quarter.min(MOST_BY_DEFAULT))
}

/// The budget in bytes: `set_mib` mebibytes where the person set it, otherwise the default
/// for a computer of `physical_bytes`.
pub fn bytes_for(set_mib: Option<u32>, physical_bytes: Option<u64>) -> usize {
    set_mib.map_or_else(
        || default_for(physical_bytes),
        |mib| usize::try_from(mib).map_or(usize::MAX, |mib| mib.saturating_mul(1 << 20)),
    )
}

/// What the frame cache may hold of a `budget` when the volumes kept hold `held` bytes: what is
/// left, and never less than [`CACHE_FLOOR`] (or the whole budget, if that is less).
pub fn cache_share(budget: usize, held: usize) -> usize {
    budget.saturating_sub(held).max(CACHE_FLOOR.min(budget))
}

/// What counts against the budget, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    /// The values of the volumes kept, as far as they are in memory.
    pub values: usize,
    /// The slices in the frame cache.
    pub cache: usize,
    /// What the graphics card holds of the volume.
    pub card: usize,
}

/// How far what is held goes past the `budget`, in bytes: the card's copy counted only where
/// the card shares the computer's memory.
pub fn over(budget: usize, held: Held, card_shares_memory: bool) -> usize {
    let card = if card_shares_memory { held.card } else { 0 };
    held.values
        .saturating_add(held.cache)
        .saturating_add(card)
        .saturating_sub(budget)
}

#[cfg(test)]
mod tests;
