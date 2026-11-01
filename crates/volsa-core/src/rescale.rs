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

    /// The stored number whose value is `value`, to the bit; `None` when no whole number is.
    /// For a value read before its stored number was known, so that it is held as that.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the number is checked to lie within the range of an i32 first"
    )]
    pub fn stored_for(self, value: f32) -> Option<i32> {
        let guess = ((f64::from(value) - self.intercept) / self.slope).round();
        if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&guess) {
            return None;
        }
        let stored = guess as i32;
        (self.value(stored).to_bits() == value.to_bits()).then_some(stored)
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

    #[test]
    fn a_value_gives_back_its_stored_number_only_when_that_is_exact() {
        let ct = Rescale {
            slope: 1.0,
            intercept: -1024.0,
        };
        assert_eq!(ct.stored_for(0.0), Some(1024));
        assert_eq!(ct.stored_for(-1024.0), Some(0));
        assert_eq!(ct.stored_for(0.5), None, "between two stored numbers");
        assert_eq!(ct.stored_for(f32::NAN), None);
        let fine = Rescale {
            slope: 0.1,
            intercept: 0.0,
        };
        for stored in [0, 1, 7, 1234, 65535] {
            let value = fine.value(stored);
            assert_eq!(fine.stored_for(value), Some(stored), "{stored}");
        }
    }
}
