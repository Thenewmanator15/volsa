//! Cutting the turned volume to the head, when it must be made smaller.
//!
//! This step is Voxa's own: `SynthSeg`'s program has no such cut. The head is the largest
//! connected piece of the voxels that are brighter than a mark. Voxels are connected when they
//! share a face, as `SciPy`'s `label` joins them by default (the module `pieces` finds the
//! piece). The mark lies a tenth of the way up from the 0.5th percentile to the 99.5th, both
//! taken over the whole turned grid. The cut is the smallest box that holds the piece, with 16
//! voxels added on every side, kept within the grid. With nothing above the mark, the cut is
//! the whole grid.

use voxa_core::Volume;
use voxa_models::Cut;

use super::align::Turned;
use super::{PrepareError, make_ready, padded_voxels};
pub(super) use crate::pieces::Piece;

/// How far up from the low percentile to the high one the mark lies.
const MARK_PART: f64 = 0.1;
/// This many voxels are added on every side of the head.
const MARGIN: usize = 16;

/// How many voxels the image a network is given has, with each side padded as [`crate::prepare`]
/// pads it, for the whole grid and for the head cut out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    /// The voxels of the image when the whole grid is given.
    pub whole: u64,
    /// The voxels of the image when it is cut to the head. It is the same as `whole` when
    /// the head's box fills the grid, or when there is no head to find.
    pub head: u64,
}

/// The sizes of the image that [`crate::prepare`] would make of this volume, whole and cut to
/// the head, for deciding where a run fits before it is made.
///
/// # Errors
///
/// Exactly what [`crate::prepare`] gives for this volume with `Cut::Head`:
/// [`PrepareError::TooFewSlices`] and [`PrepareError::TooLarge`] for the sizes,
/// [`PrepareError::NothingToSee`] when the two percentiles of the whole grid are equal, and
/// the same when the two percentiles of the head's box are equal. That last is the one
/// refusal that `prepare` with `Cut::Whole` does not make: the whole grid can be given, but
/// the head cannot.
pub fn sizes(volume: &Volume) -> Result<Sizes, PrepareError> {
    let ready = make_ready(volume, Cut::Head)?;
    Ok(Sizes {
        whole: padded_voxels(ready.whole),
        head: padded_voxels(ready.cut.size),
    })
}

/// A box of the turned grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Block {
    /// Where the box begins along x, y and z.
    pub(super) from: [usize; 3],
    /// How many voxels it has along x, y and z.
    pub(super) size: [usize; 3],
}

/// How bright a voxel must be to count as part of the head, given the two percentiles.
pub(super) fn mark(low: f64, high: f64) -> f64 {
    low + MARK_PART * (high - low)
}

/// The turned grid cut to the head, and where the cut began. `low` and `high` are the
/// percentiles of the whole grid.
pub(super) fn cut_to_head(turned: Turned, low: f64, high: f64) -> (Turned, [usize; 3]) {
    let block = head_block(&turned, mark(low, high));
    (cut(turned, &block), block.from)
}

/// The largest piece of the voxels brighter than `mark`, joined by their faces. The first one
/// found, in the order of the values, is taken when two are as large. `None` when no voxel is
/// brighter.
pub(super) fn largest_piece(turned: &Turned, mark: f64) -> Option<Piece> {
    let above = turned.values.iter().map(|&value| f64::from(value) > mark);
    let mut unseen: Vec<bool> = above.collect();
    crate::pieces::largest(&mut unseen, turned.size).map(|(_, piece)| piece)
}

/// The box that holds the head: the largest piece above `mark`, with 16 voxels more on every
/// side, kept within the grid. The whole grid when nothing is above `mark`.
pub(super) fn head_block(turned: &Turned, mark: f64) -> Block {
    let Some(piece) = largest_piece(turned, mark) else {
        return Block {
            from: [0; 3],
            size: turned.size,
        };
    };
    let span = |least: usize, most: usize, size: usize| {
        let from = least.saturating_sub(MARGIN);
        let end = (most + 1 + MARGIN).min(size);
        (from, end.saturating_sub(from))
    };
    let ([least_x, least_y, least_z], [most_x, most_y, most_z]) = (piece.least, piece.most);
    let [size_x, size_y, size_z] = turned.size;
    let ((from_x, wide), (from_y, deep), (from_z, long)) = (
        span(least_x, most_x, size_x),
        span(least_y, most_y, size_y),
        span(least_z, most_z, size_z),
    );
    Block {
        from: [from_x, from_y, from_z],
        size: [wide, deep, long],
    }
}

/// The values of the turned grid inside `block`, which lies within the grid.
pub(super) fn cut(turned: Turned, block: &Block) -> Turned {
    if block.size == turned.size {
        return turned;
    }
    let [_, size_y, size_z] = turned.size;
    let ([from_x, from_y, from_z], [wide, deep, long]) = (block.from, block.size);
    let mut values = Vec::with_capacity(wide * deep * long);
    for x in from_x..from_x + wide {
        for y in from_y..from_y + deep {
            let start = (x * size_y + y) * size_z + from_z;
            values.extend_from_slice(turned.values.get(start..start + long).unwrap_or_default());
        }
    }
    Turned {
        size: block.size,
        values,
    }
}
