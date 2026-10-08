//! Choosing a window and level from the values of an image.

use voxa_core::WindowLevel;

use crate::histogram::Histogram;

/// Bins used to read percentiles off an image.
const BINS: usize = 4096;

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
        let Some(histogram) = Histogram::from_values(values, BINS) else {
            return WindowLevel::new(0.0, 1.0);
        };
        let low = histogram.percentile(self.low_pct);
        let high = histogram.percentile(self.high_pct);
        WindowLevel::new(low + (high - low) / 2.0, high - low)
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
