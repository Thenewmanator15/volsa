//! The numbers of one slab of a held volume, of the kind it holds.

use std::ops::Range;

use super::{HeldError, SlabRaw, SliceIn, Stored};
use crate::rescale::Rescale;

/// One slab's numbers.
pub(super) enum Values {
    Unsigned(Box<[u16]>),
    Signed(Box<[i16]>),
    Float(Box<[f32]>),
}

impl Values {
    /// `len` numbers of the kind `stored`, all 0.
    pub(super) fn zeroed(stored: Stored, len: usize) -> Self {
        match stored {
            Stored::Unsigned16 => Self::Unsigned(vec![0; len].into_boxed_slice()),
            Stored::Signed16 => Self::Signed(vec![0; len].into_boxed_slice()),
            Stored::Float32 => Self::Float(vec![0.0; len].into_boxed_slice()),
        }
    }

    pub(super) fn raw(&self) -> SlabRaw<'_> {
        match self {
            Self::Unsigned(values) => SlabRaw::Unsigned(values),
            Self::Signed(values) => SlabRaw::Signed(values),
            Self::Float(values) => SlabRaw::Float(values),
        }
    }

    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Unsigned(values) => size_of_val(&**values),
            Self::Signed(values) => size_of_val(&**values),
            Self::Float(values) => size_of_val(&**values),
        }
    }

    /// Copy a slice's numbers in from `at`.
    pub(super) fn put(&mut self, at: usize, slice: SliceIn<'_>) -> Result<(), HeldError> {
        fn copy<T: Copy>(into: &mut [T], at: usize, from: &[T]) -> Result<(), HeldError> {
            let place = into
                .get_mut(at..at + from.len())
                .ok_or(HeldError::WrongSize)?;
            place.copy_from_slice(from);
            Ok(())
        }
        match (self, slice) {
            (Self::Unsigned(into), SliceIn::Unsigned(from)) => copy(into, at, from),
            (Self::Signed(into), SliceIn::Signed(from)) => copy(into, at, from),
            (Self::Float(into), SliceIn::Float(from)) => copy(into, at, from),
            _ => Err(HeldError::WrongKind),
        }
    }

    /// The value of the number at `at`.
    pub(super) fn value(&self, at: usize, rescale: Rescale) -> Option<f32> {
        match self {
            Self::Unsigned(values) => Some(rescale.value(i32::from(*values.get(at)?))),
            Self::Signed(values) => Some(rescale.value(i32::from(*values.get(at)?))),
            Self::Float(values) => values.get(at).copied(),
        }
    }

    /// How many numbers there are.
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Unsigned(values) => values.len(),
            Self::Signed(values) => values.len(),
            Self::Float(values) => values.len(),
        }
    }

    /// The numbers `numbers` as bytes, little-end first, put on the end of `into`: part of
    /// a slab put away.
    pub(super) fn bytes_of(&self, numbers: Range<usize>, into: &mut Vec<u8>) {
        match self {
            Self::Unsigned(values) => {
                let part = values.get(numbers).unwrap_or_default();
                into.extend(part.iter().flat_map(|n| n.to_le_bytes()));
            }
            Self::Signed(values) => {
                let part = values.get(numbers).unwrap_or_default();
                into.extend(part.iter().flat_map(|n| n.to_le_bytes()));
            }
            Self::Float(values) => {
                let part = values.get(numbers).unwrap_or_default();
                into.extend(part.iter().flat_map(|n| n.to_le_bytes()));
            }
        }
    }

    /// Set the numbers from the `first` on from bytes as [`Values::bytes_of`] gives them:
    /// part of a slab brought back.
    pub(super) fn set_from_bytes(&mut self, first: usize, bytes: &[u8]) {
        match self {
            Self::Unsigned(values) => {
                let (pairs, _) = bytes.as_chunks::<2>();
                let into = values.iter_mut().skip(first);
                into.zip(pairs)
                    .for_each(|(n, pair)| *n = u16::from_le_bytes(*pair));
            }
            Self::Signed(values) => {
                let (pairs, _) = bytes.as_chunks::<2>();
                let into = values.iter_mut().skip(first);
                into.zip(pairs)
                    .for_each(|(n, pair)| *n = i16::from_le_bytes(*pair));
            }
            Self::Float(values) => {
                let (fours, _) = bytes.as_chunks::<4>();
                let into = values.iter_mut().skip(first);
                into.zip(fours)
                    .for_each(|(n, four)| *n = f32::from_le_bytes(*four));
            }
        }
    }
}
