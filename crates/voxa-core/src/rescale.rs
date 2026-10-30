//! How a stored number becomes a scan value: the slope and intercept a DICOM file gives.

/// The slope and intercept that turn a stored number into the scan's own value, as a DICOM
/// file's Rescale Slope and Rescale Intercept say.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rescale {
    /// What each step of the stored number is worth.
    pub slope: f64,
    /// The value of a stored 0.
    pub intercept: f64,
}

impl Rescale {
    /// The scan value of a stored number, worked out as the DICOM library works it out, in
    /// 64 bits and then made a 32-bit number, so that it is the same to the bit.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a 32-bit value is what the scan's values have always been read as"
    )]
    pub fn value(self, stored: i32) -> f32 {
        (self.slope * f64::from(stored) + self.intercept) as f32
    }
}

impl Default for Rescale {
    /// Stored numbers that are their own values.
    fn default() -> Self {
        Self {
            slope: 1.0,
            intercept: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_number_is_rescaled_by_slope_and_intercept() {
        let ct = Rescale {
            slope: 1.0,
            intercept: -1024.0,
        };
        assert_eq!(ct.value(1024), 0.0);
        assert_eq!(ct.value(0), -1024.0);
        assert_eq!(Rescale::default().value(-1024), -1024.0);
        let fine = Rescale {
            slope: 2.5,
            intercept: 0.25,
        };
        assert_eq!(fine.value(3).to_bits(), 7.75_f32.to_bits());
    }
}
