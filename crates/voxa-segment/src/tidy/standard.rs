//! What is kept of the standard network's answer: `SynthSeg`'s tidying with `fast` off. The
//! largest piece of everything is found on the padded grid, and then the largest piece of
//! each class of structures above the lowest.

use voxa_models::{Lists, RunError, Structure};

use super::{Chances, KEEP_ABOVE, counts};
use crate::find::room;
use crate::pieces::{Bits, keep_largest};

/// Which chances are kept, for each voxel of the padded grid.
pub(super) struct Kept {
    /// Whether each voxel lies in the largest piece of everything found.
    whole: Vec<bool>,
    /// For each structure, which of `pieces` says where it is kept. `None` for a structure of
    /// the lowest class, which no piece of a class holds back.
    piece_of: Vec<Option<usize>>,
    /// For each class above the lowest, in rising order: the voxels of its largest piece.
    pieces: Vec<Bits>,
}

impl Kept {
    /// Finds the largest piece of everything, and then of each class above the lowest.
    pub(super) fn of(chances: &Chances, lists: &Lists) -> Result<Self, RunError> {
        let voxels = chances.voxels().len();
        let mut whole = room(voxels)?;
        whole.extend(chances.voxels().map(counts));
        let (mut mask, mut spare) = (room(voxels)?, room(voxels)?);
        keep_largest(&mut whole, &mut spare, chances.size);

        // A class is whatever number the lists give it: only which are alike is read.
        let mut classes: Vec<u32> = lists.structures.iter().map(|part| part.class).collect();
        classes.sort_unstable();
        classes.dedup();
        let above_lowest = |part: &Structure| {
            let rank = classes.iter().position(|&class| class == part.class)?;
            rank.checked_sub(1)
        };
        let piece_of: Vec<Option<usize>> = lists.structures.iter().map(above_lowest).collect();
        let mut pieces = Vec::new();
        for _ in 1..classes.len() {
            pieces.push(Bits::none(voxels)?);
        }
        // A voxel counts for a class by what the first step left of its chances: outside the
        // largest piece of everything, only the first structure has any.
        for (voxel, (its, &in_whole)) in chances.voxels().zip(&whole).enumerate() {
            for ((place, &chance), piece) in its.iter().enumerate().zip(&piece_of) {
                let counts = chance > KEEP_ABOVE && (place == 0 || in_whole);
                let piece = piece.filter(|_| counts);
                if let Some(bits) = piece.and_then(|piece| pieces.get_mut(piece)) {
                    bits.set(voxel);
                }
            }
        }
        for bits in &mut pieces {
            bits.keep_largest(&mut mask, &mut spare, chances.size);
        }
        Ok(Self {
            whole,
            piece_of,
            pieces,
        })
    }

    /// Puts in `left` what is left of the chances of the voxel at this place of the padded
    /// grid: each as it was, or nought outside the largest piece of its structure's class.
    /// False, with nothing put in `left`, for a voxel outside the largest piece of
    /// everything, where only the first structure has a chance left.
    pub(super) fn left(&self, voxel: usize, chances: &[f32], left: &mut Vec<f32>) -> bool {
        if !self.whole.get(voxel).is_some_and(|&in_whole| in_whole) {
            return false;
        }
        let in_piece = |piece: usize| self.pieces.get(piece).is_some_and(|bits| bits.holds(voxel));
        left.clear();
        for (&chance, piece) in chances.iter().zip(&self.piece_of) {
            let wiped = piece.is_some_and(|piece| !in_piece(piece));
            left.push(if wiped { 0.0 } else { chance });
        }
        true
    }
}
