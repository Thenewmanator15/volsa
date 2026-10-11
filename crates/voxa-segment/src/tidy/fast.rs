//! What is kept of the robust networks' answer: `SynthSeg`'s tidying with `fast` on, as its own
//! program has it whenever those networks run. The padding is taken off first, so the largest
//! piece of everything is found on the image alone. No piece of a class is looked for:
//! instead every chance that is not above 0.2 goes, but for the first structure's.

use voxa_models::RunError;

use super::{Chances, FAST_ABOVE, counts};
use crate::find::room;
use crate::pieces::keep_largest;
use crate::prepare::Back;

/// Which chances are kept, for each voxel of the image without its padding.
pub(super) struct Kept {
    /// Whether each voxel of the image lies in the largest piece of everything found: x by
    /// x, then y, then z.
    whole: Vec<bool>,
}

impl Kept {
    /// Finds the largest piece of everything in the image without its padding. `back` fits
    /// the chances' grid, which the caller has seen to.
    pub(super) fn of(chances: &Chances, back: &Back) -> Result<Self, RunError> {
        let voxels = back.size.iter().product();
        let mut whole = room(voxels)?;
        let image = chances.image(back);
        whole.extend(image.map(|voxel| counts(chances.at(voxel))));
        let mut spare = room(voxels)?;
        keep_largest(&mut whole, &mut spare, back.size);
        Ok(Self { whole })
    }

    /// Puts in `left` what is left of the chances of the voxel at this place of the image
    /// without its padding: the first structure's as it was, and each other as it was when
    /// it lies above 0.2, or nought. False, with nothing put in `left`, for a voxel outside
    /// the largest piece of everything, where only the first structure has a chance left.
    pub(super) fn left(&self, inside: usize, chances: &[f32], left: &mut Vec<f32>) -> bool {
        if !self.whole.get(inside).is_some_and(|&in_whole| in_whole) {
            return false;
        }
        let stays = |(place, &chance): (usize, &f32)| place == 0 || chance > FAST_ABOVE;
        let chances = chances.iter().enumerate();
        left.clear();
        left.extend(chances.map(|each| if stays(each) { *each.1 } else { 0.0 }));
        true
    }
}
