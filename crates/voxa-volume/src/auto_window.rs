//! Choosing a window and level from the values of an image.

use voxa_core::WindowLevel;

use crate::histogram::Histogram;

/// Bins used to read percentiles off an image.
const BINS: usize = 4096;
/// A window narrower than this many bins was read too coarsely to be trusted: a few values far
/// outside the rest stretch the bins until nearly every other value falls into one of them.
const TOO_FEW_BINS: f32 = 16.0;
/// How many times, at most, the values about the window found are counted again more finely.
const LOOKS_AGAIN: usize = 3;

/// A named way of choosing a window: from one percentile of the image's values to another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    /// The name shown in the interface.
    pub name: &'static str,
    /// The percentile shown as black.
    pub low_pct: f64,
    /// The percentile shown as white.
    pub high_pct: f64,
}

/// The presets offered, the automatic one first. "Soft" keeps nearly every value within the
/// window; "Hard" spends the greys on the middle of the range.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "Auto",
        low_pct: 1.0,
        high_pct: 99.5,
    },
    Preset {
        name: "Soft",
        low_pct: 0.5,
        high_pct: 99.9,
    },
    Preset {
        name: "Hard",
        low_pct: 5.0,
        high_pct: 98.0,
    },
];

impl Preset {
    /// The window this preset gives for an image with these values. An image with no finite
    /// values gets centre 0 and width 1; one whose values are all the same gets width 1.
    pub fn window(&self, values: &[f32]) -> WindowLevel {
        let Some((low, high)) = self.ends(values) else {
            return WindowLevel::new(0.0, 1.0);
        };
        WindowLevel::new(low + (high - low) / 2.0, high - low)
    }

    /// The values at this preset's two percentiles; `None` when there is nothing to count.
    ///
    /// Percentiles are read off a count of the values into equal bins from the smallest to
    /// the largest. When the window that gives is only a few bins wide, the values about it
    /// are counted again by themselves, more finely; the percentiles are still those of the
    /// whole image.
    fn ends(&self, values: &[f32]) -> Option<(f32, f32)> {
        let all = Histogram::from_values(values, BINS)?;
        let last_rank = all.count().saturating_sub(1) as f64;
        let mut bin = all.bin_width();
        let (mut low, mut high) = (all.percentile(self.low_pct), all.percentile(self.high_pct));
        for _ in 0..LOOKS_AGAIN {
            if bin <= 0.0 || high - low >= TOO_FEW_BINS * bin {
                break;
            }
            let (from, to) = (low - bin, high + bin);
            let finite = values.iter().copied().filter(|value| value.is_finite());
            let below = finite.clone().filter(|value| *value < from).count() as f64;
            let about: Vec<f32> = finite.filter(|value| (from..=to).contains(value)).collect();
            let Some(finer) = Histogram::from_values(&about, BINS) else {
                break;
            };
            // The same rank in the whole image, as a percentile of the values counted again.
            let last_here = (finer.count().saturating_sub(1) as f64).max(1.0);
            let here = |pct: f64| (pct / 100.0 * last_rank - below) / last_here * 100.0;
            low = finer.percentile(here(self.low_pct));
            high = finer.percentile(here(self.high_pct));
            bin = finer.bin_width();
        }
        Some((low, high))
    }
}

/// The window Voxa picks for an image by itself: from the 1st to the 99.5th percentile of its
/// values, so a few very dark or very bright pixels do not set the contrast.
pub fn auto_window(values: &[f32]) -> WindowLevel {
    let [auto, ..] = PRESETS;
    auto.window(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Vec<f32> {
        (0..=999u16).map(f32::from).collect()
    }

    #[test]
    fn a_ramp_is_windowed_from_its_1st_to_its_99_5th_percentile() {
        let window = auto_window(&ramp());
        assert!((window.center - 502.2).abs() <= 2.0, "{window:?}");
        assert!((window.width - 984.5).abs() <= 4.0, "{window:?}");
    }

    #[test]
    fn a_constant_image_gets_a_width_of_exactly_one() {
        // Review focus 4: no spread of values to window across.
        let window = auto_window(&[7.0; 4096]);
        assert_eq!((window.center, window.width), (7.0, 1.0));
    }

    #[test]
    fn an_empty_image_gets_the_fallback_window() {
        assert_eq!(auto_window(&[]), WindowLevel::new(0.0, 1.0));
        assert_eq!(auto_window(&[f32::NAN; 8]), WindowLevel::new(0.0, 1.0));
    }

    #[test]
    fn values_that_are_not_numbers_do_not_move_the_window() {
        let mut dirty = ramp();
        dirty.extend([f32::NAN, f32::INFINITY, f32::NEG_INFINITY]);
        assert_eq!(auto_window(&dirty), auto_window(&ramp()));
    }

    #[test]
    fn one_value_far_outside_the_rest_does_not_collapse_the_window() {
        // A thousand million times the step of the ramp: every other value falls in one bin
        // of the first count.
        for far in [1.0e9_f32, -1.0e9] {
            let mut values = ramp();
            values.push(far);
            let window = auto_window(&values);
            assert!((window.low() - 10.0).abs() <= 2.0, "{far}: {window:?}");
            assert!((window.high() - 995.0).abs() <= 2.0, "{far}: {window:?}");
        }
    }

    #[test]
    fn values_far_outside_on_both_sides_and_at_two_scales_are_seen_past() {
        let mut values = ramp();
        values.extend([1.0e9, -1.0e9, 1.0e6, -1.0e6]);
        let window = auto_window(&values);
        assert!(window.low() > 0.0 && window.high() < 1000.0, "{window:?}");
        assert!(window.width > 950.0, "{window:?}");
    }

    #[test]
    fn a_few_very_bright_pixels_do_not_set_the_window() {
        // A hundred times brighter than anything else, as a spike of noise would be.
        let mut values: Vec<f32> = (0..10_000u16).map(|i| f32::from(i % 1000)).collect();
        values.extend([100_000.0; 3]);
        let window = auto_window(&values);
        assert!(window.high() < 1100.0, "{window:?}");
        assert!(window.width > 900.0, "{window:?}");
    }

    #[test]
    fn the_presets_are_auto_soft_and_hard() {
        let listed: Vec<(&str, f64, f64)> = PRESETS
            .iter()
            .map(|preset| (preset.name, preset.low_pct, preset.high_pct))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("Auto", 1.0, 99.5),
                ("Soft", 0.5, 99.9),
                ("Hard", 5.0, 98.0)
            ]
        );
    }

    #[test]
    fn a_harder_preset_gives_a_narrower_window() {
        let values = ramp();
        let [auto, soft, hard] = PRESETS.each_ref().map(|preset| preset.window(&values));
        assert!(
            hard.width < auto.width && auto.width < soft.width,
            "{hard:?} {auto:?} {soft:?}"
        );
        assert!(
            (hard.low() - 49.95).abs() <= 1.0 && (hard.high() - 979.02).abs() <= 1.0,
            "{hard:?}"
        );
    }

    #[test]
    fn automatic_windowing_is_the_first_preset() {
        let values = ramp();
        assert_eq!(auto_window(&values), PRESETS[0].window(&values));
    }
}
