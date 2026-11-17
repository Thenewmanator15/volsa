//! A slice's values as they are kept: the whole numbers the file stores, with the rescale that
//! makes them values, where the slice has them, at half the memory of 32-bit values; or the
//! values themselves. Read one at a time they are the same to the bit either way.

use std::borrow::Cow;

use super::stored::StoredSamples;

/// A slice's values, row by row.
#[derive(Debug, Clone)]
pub struct Values(StoredSamples);

/// The same values, however each is kept.
impl PartialEq for Values {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

/// The same values as these, in order.
impl PartialEq<Vec<f32>> for Values {
    fn eq(&self, other: &Vec<f32>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter().copied())
    }
}

impl Values {
    /// How many values there are.
    pub fn len(&self) -> usize {
        match &self.0 {
            StoredSamples::Unsigned(numbers, _) => numbers.len(),
            StoredSamples::Signed(numbers, _) => numbers.len(),
            StoredSamples::Float(values) => values.len(),
        }
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The value at `at`, counted row by row; `None` past the end.
    pub fn get(&self, at: usize) -> Option<f32> {
        match &self.0 {
            StoredSamples::Unsigned(numbers, rescale) => {
                numbers.get(at).map(|n| rescale.value(i32::from(*n)))
            }
            StoredSamples::Signed(numbers, rescale) => {
                numbers.get(at).map(|n| rescale.value(i32::from(*n)))
            }
            StoredSamples::Float(values) => values.get(at).copied(),
        }
    }

    /// Every value, row by row.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        (0..self.len()).filter_map(|at| self.get(at))
    }

    /// The values as 32-bit numbers: borrowed where they are kept so, worked out otherwise.
    pub fn floats(&self) -> Cow<'_, [f32]> {
        match &self.0 {
            StoredSamples::Float(values) => Cow::Borrowed(values),
            _ => Cow::Owned(self.iter().collect()),
        }
    }

    /// The bytes of memory the values take.
    pub fn bytes(&self) -> usize {
        match &self.0 {
            StoredSamples::Unsigned(numbers, _) => size_of_val(numbers.as_slice()),
            StoredSamples::Signed(numbers, _) => size_of_val(numbers.as_slice()),
            StoredSamples::Float(values) => size_of_val(values.as_slice()),
        }
    }

    /// How the values are kept, given up.
    pub fn into_kept(self) -> StoredSamples {
        self.0
    }

    /// How the values are kept: whole numbers with their rescale, or the values.
    pub fn kept(&self) -> &StoredSamples {
        &self.0
    }
}

impl From<Vec<f32>> for Values {
    fn from(values: Vec<f32>) -> Self {
        Self(StoredSamples::Float(values))
    }
}

impl FromIterator<f32> for Values {
    fn from_iter<T: IntoIterator<Item = f32>>(values: T) -> Self {
        Self(StoredSamples::Float(values.into_iter().collect()))
    }
}

impl From<StoredSamples> for Values {
    fn from(samples: StoredSamples) -> Self {
        Self(samples)
    }
}
