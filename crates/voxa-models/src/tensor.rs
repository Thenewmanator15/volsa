//! The numbers that go into a network and come out of it.

/// The numbers of a [`Tensor`], of one of the two kinds the networks take and give.
#[derive(Debug, Clone, PartialEq)]
pub enum Numbers {
    /// Floating point numbers: an image, the chance of each structure, the scores.
    F32(Vec<f32>),
    /// Bytes: a map of classes or of areas.
    U8(Vec<u8>),
}

impl Numbers {
    /// How many numbers there are.
    fn count(&self) -> usize {
        match self {
            Numbers::F32(numbers) => numbers.len(),
            Numbers::U8(numbers) => numbers.len(),
        }
    }
}

/// A block of numbers and the size of each of its sides, the last index changing fastest.
///
/// The fields are open, so a tensor can be taken apart without a copy. One put together by
/// hand may hold another count of numbers than its shape says; a network refuses it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    /// The size of each side, the first index first.
    pub shape: Vec<usize>,
    /// The numbers, the last index changing fastest.
    pub numbers: Numbers,
}

impl Tensor {
    /// A tensor of floating point numbers; `None` when there are not as many as the shape
    /// says.
    pub fn f32(shape: Vec<usize>, numbers: Vec<f32>) -> Option<Self> {
        Self::whole(shape, Numbers::F32(numbers))
    }

    /// A tensor of bytes; `None` when there are not as many as the shape says.
    pub fn u8(shape: Vec<usize>, numbers: Vec<u8>) -> Option<Self> {
        Self::whole(shape, Numbers::U8(numbers))
    }

    /// Whether there are as many numbers as the shape says.
    pub(crate) fn is_whole(&self) -> bool {
        holds(&self.shape) == Some(self.numbers.count())
    }

    fn whole(shape: Vec<usize>, numbers: Numbers) -> Option<Self> {
        let tensor = Self { shape, numbers };
        tensor.is_whole().then_some(tensor)
    }
}

/// How many numbers a shape holds: one for a shape with no sides. `None` when that is more
/// than can be counted.
fn holds(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1_usize, |count, &side| count.checked_mul(side))
}
