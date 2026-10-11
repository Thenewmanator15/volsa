//! Tidying what the networks found into labels on the volume's own grid, as `SynthSeg`'s own
//! program tidies it. That program goes one of two ways, by which networks ran, and so does
//! [`tidy`].
//!
//! *What the standard network gives* (the module `standard`):
//!
//! 1. *The largest piece of everything.* A voxel counts when its chances of every structure
//!    but the first, which is nothing, add to more than 0.25. The largest piece of such
//!    voxels is kept, and outside it those chances are put at nought. The piece is found on
//!    the padded grid: two blobs joined only through the padding are one piece.
//! 2. *The largest piece of each class.* For each class of structures above the lowest, a
//!    voxel counts when one of the class's structures has a chance over 0.25 there. The
//!    largest piece of such voxels is kept, and outside it the chances of the class's
//!    structures are put at nought.
//! 3. *The padding is taken off.*
//!
//! *What the robust networks give* (the module `fast`). `SynthSeg`'s program turns on what it
//! calls `fast` whenever it runs them, and its tidying is then shorter:
//!
//! 1. *The padding is taken off* first.
//! 2. *The largest piece of everything,* as above, but found on the image without its
//!    padding: two blobs joined only through the padding are two pieces.
//! 3. *The small chances go.* Every chance that is not above 0.2 is put at nought, but for
//!    the first structure's. No piece of a class is looked for.
//!
//! Then, either way:
//!
//! - *Each voxel takes the label* of the largest of its chances, the first when several are
//!   equal.
//! - *The cortex is divided.* Where that label is one of the two cortex labels, it becomes
//!   the label of the area the cortex network named there.
//! - *What was cut to the head is put back* where it was cut from, with nothing round it,
//!   and *the axes are turned back,* so the labels lie on the 1 mm grid as the volume stores
//!   it.
//!
//! The steps follow `postprocess` in `SynthSeg`'s `predict_synthseg.py`, as `SynthSeg` 2.0's
//! own program calls it: with `fast` off for the standard network and on for the robust ones.
//!
//! Its arithmetic is followed too, since a label can turn on the last bit of a number. A
//! voxel's chances are added in the order `NumPy` adds them. And before the largest is looked
//! for they are divided by their sum, in single precision: two chances a hair apart can
//! become equal by it, and then the first wins.
//!
//! The chances of a whole head are over 2 GB, so no second set of them is made. What is kept
//! is remembered as a yes or a no for each voxel, and for the standard network a bit a voxel
//! more for each class. The chances are put at nought one voxel at a time, as the largest is
//! looked for. Every list here that grows with the scan is asked for in a way that can be
//! refused: running out of memory is [`RunError::NoRoom`], not the end of the program.

use voxa_labels::LabelGrid;
use voxa_models::{Lists, Numbers, RunError, Tensor, Wanted};

use crate::find::{Found, failed, holds, room};
use crate::prepare::Back;

mod fast;
#[cfg(test)]
mod fast_tests;
#[cfg(test)]
mod label_tests;
#[cfg(test)]
mod pieces_tests;
#[cfg(test)]
mod real_lists_tests;
#[cfg(test)]
mod refusal_tests;
mod standard;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;

/// `SynthSeg`'s mark for what is kept, either way: the sum of a voxel's chances for the piece
/// of everything, and one chance for the piece of a class. What is kept lies above it.
const KEEP_ABOVE: f32 = 0.25;
/// `SynthSeg`'s mark the fast way, which is the way of the robust networks: a chance other
/// than the first structure's is kept when it lies above this.
const FAST_ABOVE: f32 = 0.2;
/// `NumPy` adds up to this many numbers eight abreast, and a longer list a half at a time.
const ABREAST_UP_TO: usize = 128;

// What a failure carries, for the log. Each is a plain sentence that names no file.
const NOT_CHANCES: &str = "What was found is not the chance of each structure at each voxel.";
const NOT_AREAS: &str = "What the cortex network found is not an area for each voxel.";
const NO_WAY_BACK: &str = "What was found does not fit the scan it is said to be found in.";

/// Tidies what the networks found into labels on the 1 mm grid of the volume, as the volume
/// stores it: `back`'s grid and its place in the patient, columns fastest.
///
/// `found` is what [`find`](crate::find()) gave for the image that `back` came with, `lists`
/// the lists it was found with, and `wanted` what it was asked for. When the robust networks
/// ran (`wanted.robust`), the chances are tidied the way `SynthSeg`'s program calls fast, as
/// that program tidies them; when the standard network ran, the standard way. Nothing else
/// of `wanted` is read: the cortex is divided when `found` has its areas.
///
/// A voxel's label is its structure's own number, or its cortex area's when the cortex was
/// divided; 0 is nothing. A voxel of the grid that was cut away before the networks ran is
/// 0. Only the chances and the areas of `found` are read.
///
/// # Errors
///
/// [`RunError::NoRoom`] when there is not the memory for the labels or for what is kept
/// while they are made. [`RunError::Failed`] when `found` is not the chances of the lists'
/// structures on a grid of at least one voxel, with areas for the same grid if it has any,
/// or when `back` does not fit that grid.
pub fn tidy(
    found: &Found,
    lists: &Lists,
    wanted: Wanted,
    back: &Back,
) -> Result<LabelGrid, RunError> {
    let chances = Chances::of(&found.probabilities, lists.structures.len())?;
    let areas = areas_of(found.areas.as_ref(), chances.size)?;
    let way = Way::of(back, chances.size)?;
    let mut ids = room(way.voxels)?;
    ids.resize(way.voxels, 0);
    let kept = if wanted.robust {
        Kept::Fast(fast::Kept::of(&chances, back)?)
    } else {
        Kept::Standard(standard::Kept::of(&chances, lists)?)
    };
    let mut held = Vec::with_capacity(chances.count);
    // The voxels of the image without its padding, with the place of each in the grid.
    let voxels = chances.image(back).zip(way.places()).enumerate();
    for (inside, (voxel, place)) in voxels {
        let its = chances.at(voxel);
        let any_left = match &kept {
            Kept::Standard(kept) => kept.left(voxel, its, &mut held),
            Kept::Fast(kept) => kept.left(inside, its, &mut held),
        };
        // Outside the largest piece of everything only the first structure has a chance
        // left. Whatever that is, and whatever is made of it, no other is larger.
        let top = if any_left { top_of(&held) } else { 0 };
        let area = areas.and_then(|areas| areas.get(voxel));
        if let Some(id) = ids.get_mut(place) {
            *id = label_of(lists, top, area.copied());
        }
    }
    Ok(LabelGrid {
        dims: back.grid_dims,
        ids,
        placement: back.grid_placement,
    })
}

/// What is kept of the chances, by one of `SynthSeg`'s two ways.
enum Kept {
    /// The standard network's: found on the padded grid, with a piece for each class.
    Standard(standard::Kept),
    /// The robust networks': found on the image without its padding.
    Fast(fast::Kept),
}

/// Whether a voxel counts towards the largest piece of everything: its chances of every
/// structure but the first, which is nothing, add to more than 0.25.
fn counts(chances: &[f32]) -> bool {
    summed(chances.get(1..).unwrap_or_default()) > KEEP_ABOVE
}

/// The place among the structures of the largest of what is left of a voxel's chances, once
/// they are divided by their sum.
fn top_of(left: &[f32]) -> usize {
    let sum = summed(left);
    largest(left.iter().map(|chance| chance / sum))
}

/// The label of a voxel whose top structure is at this place in the lists. Where that is
/// cortex and the cortex network named an area, it is the area's label. An area the lists do
/// not have leaves the cortex as it is.
fn label_of(lists: &Lists, top: usize, area: Option<u8>) -> u32 {
    let label = lists.structures.get(top).map_or(0, |part| part.label);
    let area = area.filter(|_| lists.cortex.contains(&label));
    let area = area.and_then(|area| lists.areas.get(usize::from(area)));
    area.map_or(label, |area| area.label)
}

/// The chance of each structure at each voxel of the padded grid, as the networks left them.
struct Chances<'a> {
    /// So many numbers for each voxel as there are structures; among the voxels z is fastest.
    values: &'a [f32],
    /// How many voxels the padded grid has along x, y and z.
    size: [usize; 3],
    /// How many structures there are: at least one.
    count: usize,
}

impl<'a> Chances<'a> {
    /// Takes a tensor for the chances of `count` structures, if it is that: floating point
    /// numbers, `[1, x, y, z, count]`, as many as its shape says, with at least one voxel.
    /// So no side is longer than there are voxels held, and a list as long as a side can be
    /// asked for.
    fn of(found: &'a Tensor, count: usize) -> Result<Self, RunError> {
        let (&[1, x, y, z, structures], Numbers::F32(values)) =
            (found.shape.as_slice(), &found.numbers)
        else {
            return Err(failed(NOT_CHANCES));
        };
        let whole = structures == count && holds(&found.shape) == Some(values.len());
        if count == 0 || [x, y, z].contains(&0) || !whole {
            return Err(failed(NOT_CHANCES));
        }
        Ok(Self {
            values,
            size: [x, y, z],
            count,
        })
    }

    /// The chances of each voxel of the padded grid in turn.
    fn voxels(&self) -> std::slice::ChunksExact<'a, f32> {
        self.values.chunks_exact(self.count)
    }

    /// The chances of the voxel at this place in the padded grid.
    fn at(&self, voxel: usize) -> &'a [f32] {
        let from = voxel * self.count;
        self.values.get(from..from + self.count).unwrap_or_default()
    }

    /// The place in the padded grid of each voxel of the image without its padding: x by x,
    /// then y, then z. `back` fits the padded grid, which [`Way::of`] has seen to.
    fn image(&self, back: &Back) -> impl Iterator<Item = usize> + use<> {
        let ([before_x, before_y, before_z], [size_x, size_y, size_z]) =
            (back.padding_before, back.size);
        let [_, padded_y, padded_z] = self.size;
        let line = move |x: usize, y: usize| {
            let start = ((x + before_x) * padded_y + y + before_y) * padded_z + before_z;
            start..start + size_z
        };
        (0..size_x).flat_map(move |x| (0..size_y).flat_map(move |y| line(x, y)))
    }
}

/// The area the cortex network named at each voxel of a padded grid of this size, when the
/// cortex was divided: a byte for each voxel.
fn areas_of(found: Option<&Tensor>, size: [usize; 3]) -> Result<Option<&[u8]>, RunError> {
    let Some(areas) = found else {
        return Ok(None);
    };
    let [x, y, z] = size;
    match &areas.numbers {
        Numbers::U8(bytes) if areas.shape == [1, x, y, z] && bytes.len() == x * y * z => {
            Ok(Some(bytes))
        }
        _ => Err(failed(NOT_AREAS)),
    }
}

/// Where the voxels of the image lie in the 1 mm grid as the volume stores it.
struct Way {
    /// For each place along the image's x, y and z, counted from where the padding ends: what
    /// that place adds to the voxel's place in the list of the grid, columns fastest.
    steps: [Vec<usize>; 3],
    /// How many voxels the grid has.
    voxels: usize,
}

impl Way {
    /// The way back that `back` describes, from an image padded to `padded`. No room when
    /// the grid has more voxels than can be counted, and a failure when `back` does not fit:
    /// its axes are not the grid's three, the image does not lie within the padded grid, or
    /// what was cut does not lie within the grid.
    fn of(back: &Back, padded: [usize; 3]) -> Result<Self, RunError> {
        let dims = back
            .grid_dims
            .map(|side| usize::try_from(side).unwrap_or(usize::MAX));
        let [cols, rows, slices] = dims;
        let slab = cols.checked_mul(rows).ok_or(RunError::NoRoom)?;
        let voxels = slab.checked_mul(slices).ok_or(RunError::NoRoom)?;
        let mut each_once = back.order;
        each_once.sort_unstable();
        let along = |axis| steps_along(back, axis, padded, dims, [1, cols, slab]);
        match (each_once, along(0), along(1), along(2)) {
            ([0, 1, 2], Some(x), Some(y), Some(z)) => Ok(Self {
                steps: [x, y, z],
                voxels,
            }),
            _ => Err(failed(NO_WAY_BACK)),
        }
    }

    /// The place in the grid as stored of each voxel of the image without its padding, in
    /// the order of [`Chances::image`].
    fn places(&self) -> impl Iterator<Item = usize> + '_ {
        let [xs, ys, zs] = &self.steps;
        let line = move |x: usize, y: usize| zs.iter().map(move |z| x + y + z);
        xs.iter()
            .flat_map(move |&x| ys.iter().flat_map(move |&y| line(x, y)))
    }
}

/// What each place along one of the image's axes adds to a voxel's place in the list of the
/// grid as stored, by the rule of [`Back`]: take the cut's start with it, count from the far
/// end when the axis was turned round, and step as the grid's own axis steps. `None` when
/// `back` does not fit along this axis.
fn steps_along(
    back: &Back,
    axis: usize,
    padded: [usize; 3],
    dims: [usize; 3],
    strides: [usize; 3],
) -> Option<Vec<usize>> {
    let from = *back.order.get(axis)?;
    let (length, stride) = (*dims.get(from)?, *strides.get(from)?);
    let (size, cut_from) = (*back.size.get(axis)?, *back.cut_from.get(axis)?);
    let padded_end = back.padding_before.get(axis)?.checked_add(size)?;
    if padded_end > *padded.get(axis)? || cut_from.checked_add(size)? > length {
        return None;
    }
    let round = *back.turned_round.get(axis)?;
    let place = |along: usize| {
        let turned = along + cut_from;
        if round { length - 1 - turned } else { turned }
    };
    Some((0..size).map(|along| place(along) * stride).collect())
}

/// The sum of single precision numbers as `NumPy` adds them along the last axis of an array,
/// which is how `SynthSeg` adds a voxel's chances. Under eight numbers are added in turn. Up to
/// 128 are added in eight lanes, every eighth number to a lane; the lanes are added in pairs,
/// and what is left over is added in turn. A longer list is halved, at a multiple of eight,
/// and its halves added.
fn summed(numbers: &[f32]) -> f32 {
    if numbers.len() > ABREAST_UP_TO {
        let half = numbers.len() / 2;
        let (first, second) = numbers.split_at(half - half % 8);
        return summed(first) + summed(second);
    }
    let (eights, rest) = numbers.as_chunks::<8>();
    let Some((first, more)) = eights.split_first() else {
        return rest.iter().fold(0.0, |sum, &number| sum + number);
    };
    let mut lanes = *first;
    for eight in more {
        for (lane, &number) in lanes.iter_mut().zip(eight) {
            *lane += number;
        }
    }
    let [l1, l2, l3, l4, l5, l6, l7, l8] = lanes;
    let sum = ((l1 + l2) + (l3 + l4)) + ((l5 + l6) + (l7 + l8));
    rest.iter().fold(sum, |sum, &number| sum + number)
}

/// The place of the largest number, as `NumPy`'s `argmax` gives it: the first of those that
/// are equal, and the first that is no number when there is one. Nought divided by nought is
/// no number, so a voxel with no chance of anything is the first structure, nothing.
fn largest(numbers: impl Iterator<Item = f32>) -> usize {
    let mut top: Option<(usize, f32)> = None;
    for (place, number) in numbers.enumerate() {
        if number.is_nan() {
            return place;
        }
        if top.is_none_or(|(_, held)| number > held) {
            top = Some((place, number));
        }
    }
    top.map_or(0, |(place, _)| place)
}
