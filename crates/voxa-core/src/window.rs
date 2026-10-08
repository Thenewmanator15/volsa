//! Window and level: the range of image values mapped from black to white.

/// The window (width) and level (centre) used to turn image values into grey.
///
/// Values at `center - width / 2` and below are black; values at `center + width / 2` and above
/// are white.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowLevel {
    /// The value shown as mid-grey.
    pub center: f32,
    /// The span of values from black to white; never below 1.0.
    pub width: f32,
}

impl WindowLevel {
    /// A window with its width clamped to at least 1.0, so a zero, negative or not-a-number
    /// width can never invert or blank the picture. A centre that is not a finite number
    /// becomes 0.0.
    pub fn new(center: f32, width: f32) -> Self {
        Self {
            center: if center.is_finite() { center } else { 0.0 },
            width: if width.is_finite() {
                width.max(1.0)
            } else {
                1.0
            },
        }
    }

    /// The value shown as black.
    pub fn low(&self) -> f32 {
        self.center - self.width / 2.0
    }

    /// The value shown as white.
    pub fn high(&self) -> f32 {
        self.center + self.width / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_width_is_clamped() {
        assert_eq!(WindowLevel::new(40., 0.).width, 1.0);
        assert_eq!(WindowLevel::new(40., -5.).width, 1.0);
    }

    #[test]
    fn a_normal_window_is_kept() {
        let wl = WindowLevel::new(502.2, 984.5);
        assert_eq!((wl.center, wl.width), (502.2, 984.5));
    }

    #[test]
    fn values_that_are_not_numbers_fall_back() {
        let wl = WindowLevel::new(f32::NAN, f32::NAN);
        assert_eq!((wl.center, wl.width), (0.0, 1.0));
        assert_eq!(WindowLevel::new(f32::INFINITY, f32::INFINITY).center, 0.0);
    }

    #[test]
    fn low_and_high_are_the_ends_of_the_window() {
        let wl = WindowLevel::new(100.0, 50.0);
        assert_eq!((wl.low(), wl.high()), (75.0, 125.0));
    }
}
