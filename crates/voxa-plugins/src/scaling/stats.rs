//! Statistics of the scan at the plugin's spacing, gathered one slab of the grid at a time so
//! that memory does not grow with the scan: how many values, the lowest and the highest, the
//! mean and the standard deviation, and a histogram of 65 536 bins for the percentiles.

use voxa_core::Volume;
use voxa_models::RunError;

use crate::Voxels;
use crate::grid::{Grid, Region, sample};

/// How many bins the histogram has.
const BINS: usize = 1 << 16;
/// What a voxel outside the scan is sampled as while statistics are gathered.
const SURROUNDINGS: f32 = f32::NEG_INFINITY;

/// One bin of the histogram: how many values fell in it, and the lowest and the highest.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bin {
    count: u64,
    lowest: f64,
    highest: f64,
}

/// What the scan's values are like, at the plugin's spacing.
#[derive(Debug, Clone, PartialEq)]
pub struct Statistics {
    /// The lowest value counted; 0 when none was.
    pub lowest: f32,
    /// The highest value counted; 0 when none was.
    pub highest: f32,
    /// The lowest value of the scan, whether it counted or not: what lies around it; 0 when
    /// the scan has no value.
    pub scan_lowest: f32,
    /// How many values were counted.
    pub count: u64,
    /// Their mean.
    pub mean: f64,
    /// Their standard deviation, as the whole of them: divided by the count.
    pub sd: f64,
    /// The most values sampled at once while gathering.
    pub(crate) largest_slab: usize,
    bins: Vec<Bin>,
}

/// Gathers the statistics of the scan over the whole grid (`over` of `None`), or over a box of
/// it whose surroundings count as the scan's lowest value. With [`Voxels::AboveZero`] only
/// values above 0 count.
///
/// # Errors
///
/// [`RunError::NoRoom`] when there is not the memory for a slab.
pub fn gather(
    volume: &Volume,
    grid: &Grid,
    over: Option<Region>,
    voxels: Voxels,
) -> Result<Statistics, RunError> {
    let region = over.unwrap_or_else(|| grid.whole());
    let counts = move |value: f32| match voxels {
        Voxels::All => true,
        Voxels::AboveZero => value > 0.0,
    };
    let first = first_pass(volume, grid, region, &counts)?;
    // The surroundings are the scan's lowest value, and count only if that value does (it is
    // then also the lowest counted).
    let surroundings = if over.is_some() && counts(first.scan_lowest) {
        first.outside
    } else {
        0
    };
    let count = first.count + surroundings;
    let sum = first.sum + surroundings as f64 * f64::from(first.scan_lowest);
    let mean = if count == 0 { 0.0 } else { sum / count as f64 };
    let (bins, squares) = second_pass(volume, grid, region, &counts, &first, mean, surroundings)?;
    let sd = if count == 0 {
        0.0
    } else {
        (squares / count as f64).sqrt()
    };
    Ok(Statistics {
        lowest: first.lowest,
        highest: first.highest,
        scan_lowest: first.scan_lowest,
        count,
        mean,
        sd,
        largest_slab: first.largest_slab,
        bins,
    })
}

/// What the first pass over the values finds.
struct First {
    /// How many values counted.
    count: u64,
    /// The lowest counted; 0 when none was.
    lowest: f32,
    /// The highest counted; 0 when none was.
    highest: f32,
    /// The lowest of every value, counted or not; 0 when there was none.
    scan_lowest: f32,
    /// Their sum.
    sum: f64,
    /// How many voxels lay outside the scan.
    outside: u64,
    /// The most values sampled at once.
    largest_slab: usize,
}

/// The first pass: how many values count, the lowest, the highest and their sum, and how many
/// voxels lie outside the scan.
fn first_pass(
    volume: &Volume,
    grid: &Grid,
    region: Region,
    counts: &dyn Fn(f32) -> bool,
) -> Result<First, RunError> {
    let (mut count, mut lowest, mut highest, mut sum) =
        (0_u64, f32::INFINITY, f32::NEG_INFINITY, 0.0);
    let (mut outside, mut scan_lowest) = (0_u64, f32::INFINITY);
    let largest_slab = slabs(volume, grid, region, &mut |value| {
        if value == SURROUNDINGS {
            outside += 1;
            return;
        }
        if value.is_finite() {
            scan_lowest = scan_lowest.min(value);
        }
        if value.is_finite() && counts(value) {
            count += 1;
            lowest = lowest.min(value);
            highest = highest.max(value);
            sum += f64::from(value);
        }
    })?;
    if count == 0 {
        (lowest, highest) = (0.0, 0.0);
    }
    if !scan_lowest.is_finite() {
        scan_lowest = 0.0;
    }
    Ok(First {
        count,
        lowest,
        highest,
        scan_lowest,
        sum,
        outside,
        largest_slab,
    })
}

/// The second pass: the histogram from the lowest to the highest, and the sum of the squares
/// about the mean, the surroundings counted at the lowest value.
fn second_pass(
    volume: &Volume,
    grid: &Grid,
    region: Region,
    counts: &dyn Fn(f32) -> bool,
    first: &First,
    mean: f64,
    surroundings: u64,
) -> Result<(Vec<Bin>, f64), RunError> {
    let empty = Bin {
        count: 0,
        lowest: f64::INFINITY,
        highest: f64::NEG_INFINITY,
    };
    let mut bins = vec![empty; BINS];
    let mut squares = 0.0;
    let mut add = |value: f64| {
        let bin = bin_of(value, first.lowest, first.highest);
        if let Some(bin) = bins.get_mut(bin) {
            bin.count += 1;
            bin.lowest = bin.lowest.min(value);
            bin.highest = bin.highest.max(value);
        }
        squares += (value - mean).powi(2);
    };
    slabs(volume, grid, region, &mut |value| {
        if value.is_finite() && counts(value) {
            add(f64::from(value));
        }
    })?;
    for _ in 0..surroundings {
        add(f64::from(first.scan_lowest));
    }
    Ok((bins, squares))
}

/// Samples the region one slab at a time, along the grid's first axis, handing every value to
/// `each`; voxels outside the scan come as [`SURROUNDINGS`]. Gives the most values held at once.
fn slabs(
    volume: &Volume,
    grid: &Grid,
    region: Region,
    each: &mut dyn FnMut(f32),
) -> Result<usize, RunError> {
    let [first, second, third] = region.size;
    let [from, along, across] = region.from;
    let mut largest = 0;
    for step in 0..first {
        let slab = Region {
            from: [
                from + isize::try_from(step).unwrap_or(isize::MAX),
                along,
                across,
            ],
            size: [1, second, third],
        };
        let values = sample(volume, grid, slab, SURROUNDINGS)?;
        largest = largest.max(values.len());
        values.into_iter().for_each(&mut *each);
    }
    Ok(largest)
}

/// The bin a value falls in, from the lowest to the highest.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held within the bins first"
)]
fn bin_of(value: f64, lowest: f32, highest: f32) -> usize {
    let (lowest, highest) = (f64::from(lowest), f64::from(highest));
    if highest <= lowest {
        return 0;
    }
    let place = (value - lowest) / (highest - lowest) * BINS as f64;
    place.clamp(0.0, (BINS - 1) as f64) as usize
}

impl Statistics {
    /// Statistics of no values, for a run whose scaling and surroundings need none.
    pub(crate) fn none() -> Self {
        Self {
            lowest: 0.0,
            highest: 0.0,
            scan_lowest: 0.0,
            count: 0,
            mean: 0.0,
            sd: 0.0,
            largest_slab: 0,
            bins: Vec::new(),
        }
    }

    /// The `index`-th smallest value counted, as near as the histogram tells it: within its
    /// bin's lowest and highest, spread evenly between them.
    fn nth(&self, index: u64) -> f64 {
        let mut before = 0;
        for bin in &self.bins {
            if index < before + bin.count {
                let within = (index - before) as f64;
                if bin.count == 1 {
                    return bin.lowest;
                }
                let share = within / (bin.count - 1) as f64;
                return bin.lowest + share * (bin.highest - bin.lowest);
            }
            before += bin.count;
        }
        f64::from(self.highest)
    }
}

/// The `percent`-th percentile of the values counted, as `NumPy` takes it (the value at rank
/// `percent / 100 × (count − 1)`, drawn straight between the two values around it), from the
/// histogram: within 1/65 536 of the range of its exact value, and exact when the values of
/// each bin it reads are equal or evenly spread.
pub fn percentile(stats: &Statistics, percent: f64) -> f64 {
    if stats.count == 0 {
        return 0.0;
    }
    let rank = percent.clamp(0.0, 100.0) / 100.0 * (stats.count - 1) as f64;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rank from 0 to the count"
    )]
    let below = rank.floor() as u64;
    let low = stats.nth(below);
    if below + 1 >= stats.count {
        return low;
    }
    let high = stats.nth(below + 1);
    low + (rank - below as f64) * (high - low)
}
