//! The brightness of what a network is given, scaled as its description says: by two
//! percentiles, by the mean and standard deviation, by fixed numbers, or not at all.

use crate::{Outside, Scaling, Voxels};

mod stats;
pub use stats::{Statistics, gather, percentile};

#[cfg(test)]
mod tests;

/// Why a scan cannot be scaled. The sentence is fit to be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScaleError {
    /// The two percentiles are equal, or the values have no spread, or none were counted.
    #[error(
        "This series holds no values that differ, so there is nothing in it for the plugin \
         to see."
    )]
    Flat,
}

/// A scaling with its numbers worked out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// `(x − low) / (high − low)`, held between 0 and 1.
    Between {
        /// The value that becomes 0.
        low: f64,
        /// The value that becomes 1.
        high: f64,
    },
    /// `(x − mean) / sd`.
    Z {
        /// The mean.
        mean: f64,
        /// The standard deviation, above 0.
        sd: f64,
    },
    /// Held within `[low, high]`, then `(x − subtract) / divide`.
    Fixed {
        /// The lowest value kept.
        low: f64,
        /// The highest value kept.
        high: f64,
        /// What is taken away.
        subtract: f64,
        /// What the rest is divided by.
        divide: f64,
    },
    /// The values as they are.
    AsStored,
}

impl Scale {
    /// The numbers of a scaling, from the statistics of the scan.
    ///
    /// # Errors
    ///
    /// [`ScaleError::Flat`] when there is nothing to scale by: equal percentiles, no spread,
    /// or no values counted.
    pub fn new(scaling: &Scaling, stats: &Statistics) -> Result<Self, ScaleError> {
        match *scaling {
            Scaling::Percentiles { low, high, .. } => {
                let (low, high) = (percentile(stats, low), percentile(stats, high));
                if stats.count == 0 || high <= low {
                    return Err(ScaleError::Flat);
                }
                Ok(Scale::Between { low, high })
            }
            Scaling::ZScore { .. } => {
                if stats.count == 0 || stats.sd <= 0.0 {
                    return Err(ScaleError::Flat);
                }
                Ok(Scale::Z {
                    mean: stats.mean,
                    sd: stats.sd,
                })
            }
            Scaling::Fixed {
                clip: [low, high],
                subtract,
                divide,
            } => Ok(Scale::Fixed {
                low,
                high,
                subtract,
                divide,
            }),
            Scaling::None => Ok(Scale::AsStored),
        }
    }

    /// Scales the values in place. A value that is not a number, which is how a voxel outside
    /// the scan is given when it is to be 0 after scaling, becomes 0.
    pub fn apply(&self, values: &mut [f32]) {
        for value in values {
            *value = if value.is_nan() {
                0.0
            } else {
                narrowed(self.one(f64::from(*value)))
            };
        }
    }

    /// One value scaled.
    fn one(&self, value: f64) -> f64 {
        match *self {
            Scale::Between { low, high } => ((value - low) / (high - low)).clamp(0.0, 1.0),
            Scale::Z { mean, sd } => (value - mean) / sd,
            Scale::Fixed {
                low,
                high,
                subtract,
                divide,
            } => (value.clamp(low, high) - subtract) / divide,
            Scale::AsStored => value,
        }
    }
}

/// A scaled value as a network is given it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a network is given singles"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// What a voxel outside the scan is sampled as: the scan's lowest value, or a value that is not
/// a number, which [`Scale::apply`] makes 0.
pub fn outside_value(outside: Outside, stats: &Statistics) -> f32 {
    match outside {
        Outside::Lowest => stats.scan_lowest,
        Outside::Zero => f32::NAN,
    }
}

/// Which voxels the statistics of a scaling count.
pub fn voxels_of(scaling: &Scaling) -> Voxels {
    match *scaling {
        Scaling::ZScore { voxels, .. } => voxels,
        _ => Voxels::All,
    }
}
