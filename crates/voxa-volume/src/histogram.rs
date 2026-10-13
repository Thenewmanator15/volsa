//! Counting how often each value occurs, to read off percentiles.

/// How often each value occurs in an image, in equal bins from the smallest value to the
/// largest. Values that are not finite numbers are left out.
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    /// The smallest value counted.
    low: f64,
    /// The span of values one bin covers; 0.0 when every value is the same.
    bin_width: f64,
    counts: Vec<u64>,
    /// How many values were counted; never 0.
    total: u64,
}

impl Histogram {
    /// Count `values` into `bins` equal bins (at least one). `None` when there is nothing to
    /// count: no values, or none that is a finite number.
    pub fn from_values(values: &[f32], bins: usize) -> Option<Self> {
        let finite = || values.iter().copied().filter(|value| value.is_finite());
        let low = f64::from(finite().reduce(f32::min)?);
        let high = f64::from(finite().reduce(f32::max)?);
        let mut counts = vec![0u64; bins.max(1)];
        let last = counts.len() - 1;
        let bin_width = (high - low) / counts.len() as f64;
        let mut total = 0;
        for value in finite() {
            let bin = if bin_width > 0.0 {
                whole((f64::from(value) - low) / bin_width)
            } else {
                0
            };
            let at = usize::try_from(bin).map_or(last, |bin| bin.min(last));
            if let Some(count) = counts.get_mut(at) {
                *count += 1;
            }
            total += 1;
        }
        Some(Self {
            low,
            bin_width,
            counts,
            total,
        })
    }

    /// How many values were counted.
    pub fn count(&self) -> u64 {
        self.total
    }

    /// The span of values one bin covers: how finely percentiles are resolved. 0.0 when every
    /// value is the same.
    pub fn bin_width(&self) -> f32 {
        narrowed(self.bin_width)
    }

    /// The value below which `p` percent of the values lie, to within one bin. `p` runs from
    /// 0.0 (the smallest value) to 100.0 (the largest); anything outside is read as the
    /// nearest end.
    ///
    /// Between two neighbouring values the result is interpolated, as the usual definition of
    /// a percentile has it, so the median of 0, 1, …, 999 is 499.5.
    pub fn percentile(&self, p: f64) -> f32 {
        let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 100.0) };
        let rank = p / 100.0 * (self.total - 1) as f64;
        let lower = self.value_at(whole(rank.floor()));
        let upper = self.value_at(whole(rank.ceil()));
        // Clamped so that rounding can never make a higher percentile the smaller value.
        narrowed((lower + rank.fract() * (upper - lower)).clamp(lower, upper))
    }

    /// The middle of the bin holding the value of the given rank, counting from 0 upwards.
    fn value_at(&self, rank: u64) -> f64 {
        let mut seen = 0u64;
        let holding = self.counts.iter().position(|count| {
            seen += count;
            seen > rank
        });
        let bin = holding.unwrap_or(self.counts.len().saturating_sub(1));
        self.low + (bin as f64 + 0.5) * self.bin_width
    }
}

/// The whole part of a number that is known to be zero or more.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the callers pass bin numbers and ranks, which are never negative, and a value too large saturates"
)]
fn whole(value: f64) -> u64 {
    value as u64
}

/// A statistic as the single-precision number the image values are held in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "it lies within the range of the f32 values it came from"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn ramp() -> Vec<f32> {
        (0..=999u16).map(f32::from).collect()
    }

    #[test]
    fn the_median_of_a_ramp_is_within_one_bin_of_the_true_median() {
        let histogram = Histogram::from_values(&ramp(), 4096).expect("has values");
        let median = histogram.percentile(50.0);
        assert!((median - 499.5).abs() <= histogram.bin_width(), "{median}");
    }

    #[test]
    fn the_ends_are_the_smallest_and_largest_values() {
        let histogram = Histogram::from_values(&ramp(), 4096).expect("has values");
        assert!(histogram.percentile(0.0).abs() <= histogram.bin_width());
        assert!((histogram.percentile(100.0) - 999.0).abs() <= histogram.bin_width());
    }

    #[test]
    fn a_percentile_outside_0_to_100_is_read_as_the_nearest_end() {
        let histogram = Histogram::from_values(&ramp(), 4096).expect("has values");
        assert_eq!(histogram.percentile(-5.0), histogram.percentile(0.0));
        assert_eq!(histogram.percentile(250.0), histogram.percentile(100.0));
        assert_eq!(histogram.percentile(f64::NAN), histogram.percentile(0.0));
    }

    #[test]
    fn nothing_to_count_means_no_histogram() {
        assert!(Histogram::from_values(&[], 4096).is_none());
        assert!(Histogram::from_values(&[f32::NAN, f32::NAN], 4096).is_none());
        assert!(Histogram::from_values(&[f32::INFINITY, f32::NEG_INFINITY], 4096).is_none());
    }

    #[test]
    fn values_that_are_not_numbers_are_left_out() {
        let mut dirty = ramp();
        dirty.extend([f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN]);
        let clean = Histogram::from_values(&ramp(), 4096).expect("has values");
        let dirty = Histogram::from_values(&dirty, 4096).expect("has values");
        for p in [0.0, 1.0, 50.0, 99.5, 100.0] {
            assert_eq!(dirty.percentile(p), clean.percentile(p), "at {p}");
        }
    }

    #[test]
    fn a_constant_image_has_that_value_at_every_percentile() {
        let histogram = Histogram::from_values(&[7.5; 100], 4096).expect("has values");
        assert_eq!(histogram.bin_width(), 0.0);
        for p in [0.0, 1.0, 50.0, 99.5, 100.0] {
            assert_eq!(histogram.percentile(p), 7.5, "at {p}");
        }
    }

    #[test]
    fn asking_for_no_bins_still_counts_into_one() {
        let histogram = Histogram::from_values(&[0.0, 10.0], 0).expect("has values");
        assert_eq!(histogram.bin_width(), 10.0);
        assert_eq!(histogram.percentile(50.0), 5.0);
    }

    proptest! {
        #[test]
        fn a_higher_percentile_is_never_a_lower_value(
            values in proptest::collection::vec(-1.0e6f32..1.0e6, 1..300),
            a in 0.0f64..=100.0,
            b in 0.0f64..=100.0,
        ) {
            let histogram = Histogram::from_values(&values, 4096).expect("has values");
            let (lower, higher) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(histogram.percentile(lower) <= histogram.percentile(higher));
        }

        #[test]
        fn every_percentile_lies_between_the_smallest_and_largest_value(
            values in proptest::collection::vec(-1.0e6f32..1.0e6, 1..300),
            p in 0.0f64..=100.0,
        ) {
            let histogram = Histogram::from_values(&values, 4096).expect("has values");
            let low = values.iter().copied().fold(f32::INFINITY, f32::min);
            let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let found = histogram.percentile(p);
            prop_assert!((low..=high).contains(&found), "{found} outside {low}..={high}");
        }
    }
}
