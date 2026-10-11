//! The largest connected piece of a set of voxels. Voxels are connected when they share a
//! face, as `SciPy`'s `label` joins them by default: `SynthSeg`'s tidying keeps such pieces, and
//! the cut to the head is the box round one.
//!
//! A grid is a list with a yes or a no for each voxel, the last of its three sides changing
//! fastest. The pieces are met in the order of that list, which is the order `SciPy` numbers
//! them in, so of two pieces as large the same one is kept as `SynthSeg` keeps.

use std::collections::VecDeque;

use voxa_models::RunError;

use crate::find::room;

/// A connected piece of voxels, and how far it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Piece {
    /// How many voxels the piece has.
    pub(crate) count: usize,
    /// The least place the piece reaches along each side of the grid.
    pub(crate) least: [usize; 3],
    /// The greatest place the piece reaches along each side.
    pub(crate) most: [usize; 3],
}

/// The largest piece of the voxels set in `mask`, joined by their faces: a mask of the same
/// grid in which only the voxels of that piece are set. Of two pieces as large, it is the
/// first one met in the order of the mask. A mask with nothing set is given back as it is.
///
/// `mask` holds a voxel for each place of a grid of `dims` voxels along its three sides, the
/// last side changing fastest. A mask that is not of that size is given back as it is.
pub fn largest_piece(mask: &[bool], dims: [usize; 3]) -> Vec<bool> {
    let mut kept = mask.to_vec();
    let voxels = dims
        .iter()
        .try_fold(1_usize, |all, &side| all.checked_mul(side));
    if voxels == Some(mask.len()) {
        keep_largest(&mut kept, &mut Vec::new(), dims);
    }
    kept
}

/// Leaves set in `mask` only the voxels of its largest piece: what [`largest_piece`] gives,
/// made where the mask lies. `spare` is a list to work in. When it has room for as many
/// voxels as the mask, no memory is asked for but for the edge of the piece being followed.
/// What it held is lost.
pub(crate) fn keep_largest(mask: &mut [bool], spare: &mut Vec<bool>, dims: [usize; 3]) {
    spare.clear();
    spare.extend_from_slice(mask);
    // Looking for the largest piece takes every voxel off the copy.
    let Some((first, _)) = largest(spare, dims) else {
        return;
    };
    // The piece is followed once more from its first voxel. It is taken off the mask and set
    // in the copy, which then holds the piece and nothing else.
    take(mask, first);
    follow(first, mask, dims, |index, _| {
        if let Some(kept) = spare.get_mut(index) {
            *kept = true;
        }
    });
    for (voxel, &kept) in mask.iter_mut().zip(spare.iter()) {
        *voxel = kept;
    }
}

/// A yes or a no for each voxel of a grid, a bit each: an eighth of what a list of them takes.
pub(crate) struct Bits(Vec<u64>);

impl Bits {
    /// A no for each of so many voxels.
    pub(crate) fn none(voxels: usize) -> Result<Self, RunError> {
        let words = voxels.div_ceil(64);
        let mut bits = room(words)?;
        bits.resize(words, 0);
        Ok(Self(bits))
    }

    /// Makes it a yes for this voxel.
    pub(crate) fn set(&mut self, voxel: usize) {
        if let Some(word) = self.0.get_mut(voxel / 64) {
            *word |= 1 << (voxel % 64);
        }
    }

    /// Whether it is a yes for this voxel.
    pub(crate) fn holds(&self, voxel: usize) -> bool {
        let word = self.0.get(voxel / 64);
        word.is_some_and(|word| word >> (voxel % 64) & 1 == 1)
    }

    /// Keeps of its voxels only the largest piece of them, on a grid of this size. `mask` and
    /// `spare` are lists to work in, each with room for every voxel.
    pub(crate) fn keep_largest(
        &mut self,
        mask: &mut Vec<bool>,
        spare: &mut Vec<bool>,
        size: [usize; 3],
    ) {
        let voxels = size.iter().product();
        mask.clear();
        mask.extend((0..voxels).map(|voxel| self.holds(voxel)));
        keep_largest(mask, spare, size);
        self.0.fill(0);
        let kept = mask.iter().enumerate().filter(|&(_, &kept)| kept);
        kept.for_each(|(voxel, _)| self.set(voxel));
    }
}

/// The largest piece of the voxels of `unseen` that are set, and where it was first met in the
/// list. The first one met is taken when two are as large; `None` when no voxel is set. Every
/// voxel is taken off `unseen`. Each is looked at once, and no function calls itself, so a
/// piece that winds through the whole grid is followed to its end without growing the stack.
pub(crate) fn largest(unseen: &mut [bool], dims: [usize; 3]) -> Option<(usize, Piece)> {
    let mut largest: Option<(usize, Piece)> = None;
    for start in 0..unseen.len() {
        if !take(unseen, start) {
            continue;
        }
        let first = place_of(start, dims);
        let mut piece = Piece {
            count: 0,
            least: first,
            most: first,
        };
        follow(start, unseen, dims, |_, at| {
            piece.count += 1;
            piece.least = merged(piece.least, at, usize::min);
            piece.most = merged(piece.most, at, usize::max);
        });
        let larger = |(_, held): &(usize, Piece)| piece.count > held.count;
        if largest.as_ref().is_none_or(larger) {
            largest = Some((start, piece));
        }
    }
    largest
}

/// Takes a voxel off the list of those still to be looked at: true when it was on the list.
fn take(unseen: &mut [bool], place: usize) -> bool {
    unseen.get_mut(place).is_some_and(std::mem::take)
}

/// Follows the piece that holds the voxel at `start`, which has been taken off the list. Every
/// other voxel of the piece is taken off too, and `met` is told of each voxel of the piece
/// once: its place in the list, and where that is in the grid. The voxels to look at next
/// wait in a queue, nearest first, so it holds the edge of what has been seen and not the
/// whole piece.
fn follow(
    start: usize,
    unseen: &mut [bool],
    dims: [usize; 3],
    mut met: impl FnMut(usize, [usize; 3]),
) {
    let mut waiting = VecDeque::from([start]);
    while let Some(index) = waiting.pop_front() {
        let at = place_of(index, dims);
        met(index, at);
        let next = neighbours(index, at, dims).into_iter().flatten();
        waiting.extend(next.filter(|&place| take(unseen, place)));
    }
}

/// Each place of `held` and of `at` along the three sides, made one by `pick`.
fn merged(held: [usize; 3], at: [usize; 3], pick: fn(usize, usize) -> usize) -> [usize; 3] {
    let ([held_x, held_y, held_z], [x, y, z]) = (held, at);
    [pick(held_x, x), pick(held_y, y), pick(held_z, z)]
}

/// Where the voxel at `index` of the list lies in the grid: along the first side, the second
/// and the third, which is fastest.
fn place_of(index: usize, [_, size_y, size_z]: [usize; 3]) -> [usize; 3] {
    let (size_y, size_z) = (size_y.max(1), size_z.max(1));
    [
        index / (size_y * size_z),
        index / size_z % size_y,
        index % size_z,
    ]
}

/// The places in the list of the voxels that share a face with the one at `index`, which lies
/// at `at`: none where the grid ends.
fn neighbours(index: usize, at: [usize; 3], dims: [usize; 3]) -> [Option<usize>; 6] {
    let ([x, y, z], [size_x, size_y, size_z]) = (at, dims);
    let (along_x, along_y) = (size_y * size_z, size_z);
    let before = |place: usize, step: usize| {
        if place > 0 {
            index.checked_sub(step)
        } else {
            None
        }
    };
    let after = |place: usize, size: usize, step: usize| {
        if place + 1 < size {
            Some(index + step)
        } else {
            None
        }
    };
    [
        before(x, along_x),
        after(x, size_x, along_x),
        before(y, along_y),
        after(y, size_y, along_y),
        before(z, 1),
        after(z, size_z, 1),
    ]
}
