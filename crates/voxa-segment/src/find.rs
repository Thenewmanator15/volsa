// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `build_model` in SynthSeg/predict_synthseg.py, translated to Rust and changed
// for Voxa.
//! The steps between `SynthSeg`'s networks: what each network is given and what is made of its
//! answer, in the order `SynthSeg`'s own program takes them.
//!
//! 1. *The structures.* The standard network runs on the image and on its mirror, and the two
//!    answers are averaged (the module `standard`). Or the three robust networks run one
//!    after another (the module `robust`). Either way the result is the chance of each
//!    structure at each voxel.
//! 2. *The top structure* of each voxel is the one with the largest chance.
//! 3. *The cortex,* when it is wanted: a network is shown where the top structure is cortex,
//!    and names the areas there.
//! 4. *The scores.* A box is cut from the map of top structures, and a network scores what
//!    is in it (the module `scores`).
//!
//! The steps follow `build_model` in `SynthSeg`'s `predict_synthseg.py`, where they are layers
//! between the networks. Nothing is tidied here: the chances are given as the networks left
//! them.
//!
//! The networks are opened by whoever calls [`find`], through [`Networks`]. The chances of a
//! whole head are over 2 GB. So one network is open at a time, and no more than two sets of
//! the chances are held at once: the result, and the answer that has just arrived. Every list
//! made here that grows with the scan or with the box is asked for in a way that can be
//! refused: running out of memory is [`RunError::NoRoom`], not the end of the program.

use voxa_models::{Lists, Network, Numbers, Role, RunError, Tensor, Wanted};

#[cfg(test)]
mod finding_tests;
#[cfg(test)]
mod refusal_tests;
mod robust;
mod scores;
#[cfg(test)]
mod scores_tests;
mod standard;
#[cfg(test)]
mod tests;

/// The most structures the lists may name: the top structure of a voxel is kept in a byte.
const MOST_STRUCTURES: usize = 256;
/// The longest side the box may have: the longest the list of a model folder may give.
const LARGEST_BOX: usize = 512;

// What a failure carries, for the log. Each is a plain sentence that names no file.
const NOT_AN_IMAGE: &str = "What the networks were to be given is not an image of one channel.";
const LISTS_DO_NOT_FIT: &str = "The lists that go with the networks do not fit one another.";
const WRONG_COUNT: &str = "A network gave more or fewer answers than it should.";
const WRONG_KIND: &str = "A network gave an answer of the wrong kind of numbers.";
const WRONG_SHAPE: &str = "A network gave an answer of the wrong size.";
const NOT_FINITE: &str = "A network gave an answer with a number that is not finite.";

/// How far the labelling of a scan has come, for whoever shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The volume is being made ready for the networks. [`find`] never says this: it is for
    /// its caller to say.
    Preparing,
    /// A network that finds the structures is about to be run.
    Finding {
        /// Which of the runs this is, the first being 1.
        step: u8,
        /// How many such runs there are: two for the standard network, three for the robust
        /// ones.
        of: u8,
    },
    /// The network that names the areas of the cortex is about to be run.
    Cortex,
    /// The network that scores the result is about to be run.
    Checking,
    /// The result is being tidied. [`find`] never says this: it is for its caller to say.
    Tidying,
}

/// Where the networks come from: something that can open each of them.
pub trait Networks {
    /// Opens the network of this role for a grid of these sizes along x, y and z. A network
    /// is opened for one size and refuses another. [`find`] drops each network before it
    /// asks for the next.
    ///
    /// # Errors
    ///
    /// [`RunError`] says why the network could not be opened.
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError>;
}

/// What the networks found in an image.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The chance of each structure at each voxel: `[1, x, y, z, n]` for the `n` structures
    /// of the lists, in the lists' order, as the networks left them.
    pub probabilities: Tensor,
    /// The area of the cortex at each voxel, as the cortex network gave it: `[1, x, y, z]` of
    /// bytes. `None` when the cortex was not wanted.
    pub areas: Option<Tensor>,
    /// A score for each score group of the lists, in their order: from 0 to 1, to four
    /// places.
    pub scores: Vec<f32>,
}

/// Runs the networks on a prepared image and gives what they found.
///
/// `image` is `[1, x, y, z, 1]`, as [`prepare`](crate::prepare) makes it. The three robust
/// networks stand in for the standard one when `wanted.robust` is set, and the cortex network
/// runs only when `wanted.cortex` is. The scores are always made. Before each run `said` is
/// told what is about to be done.
///
/// # Errors
///
/// A [`RunError`] from opening or running a network is passed on as it is.
/// [`RunError::NoRoom`] also when there is not the memory for what a network is handed or
/// for what is made of its answer. [`RunError::Failed`] when a network's answer is not of
/// the count, the kind and the size it should be, or holds a number that is not finite; and
/// when `image` is not an image of one channel, or the lists do not fit one another.
pub fn find(
    image: &Tensor,
    lists: &Lists,
    wanted: Wanted,
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Found, RunError> {
    let image = Image::of(image)?;
    let count = structures(lists)?;
    let chances = if wanted.robust {
        robust::chances(&image, count, networks, said)?
    } else {
        standard::chances(&image, &lists.flip, networks, said)?
    };
    let top = top_structures(&chances, count)?;
    let areas = if wanted.cortex {
        Some(areas(&image, &top, lists, networks, said)?)
    } else {
        None
    };
    let scores = scores::scores(&top, image.size, lists, networks, said)?;
    let probabilities = Tensor {
        shape: image.with(count),
        numbers: Numbers::F32(chances),
    };
    Ok(Found {
        probabilities,
        areas,
        scores,
    })
}

/// The image the networks are given, with its numbers and its size at hand.
struct Image<'a> {
    /// Its numbers, z changing fastest.
    values: &'a [f32],
    /// How many voxels it has along x, y and z. None is nought.
    size: [usize; 3],
}

impl<'a> Image<'a> {
    /// Takes a tensor for an image, if it is one: a single image of one channel, in floating
    /// point numbers, as many as its shape says, with at least one voxel.
    fn of(tensor: &'a Tensor) -> Result<Self, RunError> {
        let (&[1, x, y, z, 1], Numbers::F32(values)) = (tensor.shape.as_slice(), &tensor.numbers)
        else {
            return Err(failed(NOT_AN_IMAGE));
        };
        let size = [x, y, z];
        if size.contains(&0) || holds(&size) != Some(values.len()) {
            return Err(failed(NOT_AN_IMAGE));
        }
        Ok(Self { values, size })
    }

    /// A copy of the image, to hand to a network: `[1, x, y, z, 1]`.
    fn copy(&self) -> Result<Tensor, RunError> {
        let mut values = room(self.values.len())?;
        values.extend_from_slice(self.values);
        Ok(Tensor {
            shape: self.with(1),
            numbers: Numbers::F32(values),
        })
    }

    /// The shape of a map of the image's grid: one number for each voxel.
    fn map(&self) -> Vec<usize> {
        let [x, y, z] = self.size;
        vec![1, x, y, z]
    }

    /// The shape of so many numbers for each voxel of the image's grid.
    fn with(&self, channels: usize) -> Vec<usize> {
        let [x, y, z] = self.size;
        vec![1, x, y, z, channels]
    }
}

/// How many structures the lists name, when the lists fit one another as the steps here need
/// them to. There are at least two structures and no more than a byte can tell apart. The
/// swap has a place for each, and each place is one of them. Each has a score class that a
/// byte can hold. And the box has a side, of no more than the longest a model folder's list
/// may give: a longer one has more voxels than could be held, or than can be counted.
fn structures(lists: &Lists) -> Result<usize, RunError> {
    let count = lists.structures.len();
    let fit = (2..=MOST_STRUCTURES).contains(&count)
        && lists.flip.len() == count
        && lists.flip.iter().all(|&place| place < count)
        && lists.score_class.len() == count
        && lists
            .score_class
            .iter()
            .all(|&class| u8::try_from(class).is_ok())
        && (1..=LARGEST_BOX).contains(&lists.box_side);
    if fit {
        Ok(count)
    } else {
        Err(failed(LISTS_DO_NOT_FIT))
    }
}

/// A failure that carries these words.
pub(crate) fn failed(why: &str) -> RunError {
    RunError::Failed(why.to_owned())
}

/// A list with room for so many numbers and none in it yet: filling it up to that count asks
/// for no more memory. The lists made here are as large as the scan, and a list asked for in
/// the plain way ends the program when there is not the memory for it. This one says so
/// instead.
pub(crate) fn room<T>(count: usize) -> Result<Vec<T>, RunError> {
    let mut numbers = Vec::new();
    numbers
        .try_reserve_exact(count)
        .map_err(|_| RunError::NoRoom)?;
    Ok(numbers)
}

/// Opens a network, runs it once and drops it, so that the next can be opened.
fn run_once(
    networks: &mut dyn Networks,
    role: Role,
    sizes: [usize; 3],
    inputs: Vec<Tensor>,
) -> Result<Vec<Tensor>, RunError> {
    networks.open(role, sizes)?.run(inputs)
}

/// The tensors of an answer that should hold just so many.
fn exactly<const N: usize>(answer: Vec<Tensor>) -> Result<[Tensor; N], RunError> {
    answer.try_into().map_err(|_| failed(WRONG_COUNT))
}

/// How many numbers a shape holds; `None` when that is more than can be counted.
pub(crate) fn holds(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1_usize, |count, &side| count.checked_mul(side))
}

/// The numbers of an answer that should be floating point numbers of this shape, every one
/// of them finite.
fn floats(answer: Tensor, shape: &[usize]) -> Result<Vec<f32>, RunError> {
    let Numbers::F32(numbers) = answer.numbers else {
        return Err(failed(WRONG_KIND));
    };
    if answer.shape != shape || holds(shape) != Some(numbers.len()) {
        return Err(failed(WRONG_SHAPE));
    }
    if !all_finite(&numbers) {
        return Err(failed(NOT_FINITE));
    }
    Ok(numbers)
}

/// Whether every number is finite.
fn all_finite(numbers: &[f32]) -> bool {
    numbers.iter().all(|number| number.is_finite())
}

/// The map of an answer that should be one map of this shape and nothing else: a byte for
/// each voxel.
fn map(answer: Vec<Tensor>, shape: &[usize]) -> Result<Tensor, RunError> {
    let [map] = exactly(answer)?;
    match &map.numbers {
        Numbers::U8(bytes) if map.shape == shape && holds(shape) == Some(bytes.len()) => Ok(map),
        Numbers::U8(_) => Err(failed(WRONG_SHAPE)),
        Numbers::F32(_) | Numbers::I32(_) | Numbers::I64(_) => Err(failed(WRONG_KIND)),
    }
}

/// The chance of each structure as a network gives it: in two tensors, the first holding the
/// first so many structures of every voxel and the second the rest.
struct Halves {
    /// The numbers of the first tensor.
    first: Vec<f32>,
    /// How many structures the first tensor holds for a voxel: at least one.
    wide: usize,
    /// The numbers of the second tensor.
    second: Vec<f32>,
    /// How many structures the second tensor holds for a voxel: at least one.
    rest: usize,
}

impl Halves {
    /// Takes a network's answer for the two halves of `count` structures over the image's
    /// grid, if it is that. How the structures are shared between the two is read from the
    /// first; neither holds none.
    fn of(answer: Vec<Tensor>, image: &Image, count: usize) -> Result<Self, RunError> {
        let [first, second] = exactly(answer)?;
        let wide = first.shape.last().copied();
        let wide = wide.filter(|wide| (1..count).contains(wide));
        let wide = wide.ok_or_else(|| failed(WRONG_SHAPE))?;
        let rest = count - wide;
        Ok(Self {
            first: floats(first, &image.with(wide))?,
            wide,
            second: floats(second, &image.with(rest))?,
            rest,
        })
    }

    /// The two halves side by side: for each voxel the numbers of the first, then those of
    /// the second.
    fn joined(self) -> Result<Vec<f32>, RunError> {
        let mut joined = room(self.first.len() + self.second.len())?;
        let firsts = self.first.chunks_exact(self.wide);
        for (first, second) in firsts.zip(self.second.chunks_exact(self.rest)) {
            joined.extend_from_slice(first);
            joined.extend_from_slice(second);
        }
        Ok(joined)
    }
}

/// The top structure of each voxel: the place of the largest of its chances, and the first
/// such place when several are equal. `chances` holds `count` numbers for each voxel, and
/// `count` is no more than a byte can tell apart.
fn top_structures(chances: &[f32], count: usize) -> Result<Vec<u8>, RunError> {
    let top_of = |chances: &[f32]| {
        let mut top = (0, f32::NEG_INFINITY);
        for (&chance, place) in chances.iter().zip(0..=u8::MAX) {
            if chance > top.1 {
                top = (place, chance);
            }
        }
        top.0
    };
    let voxels = chances.chunks_exact(count);
    let mut top = room(voxels.len())?;
    top.extend(voxels.map(top_of));
    Ok(top)
}

/// The areas of the cortex. The cortex network is given the image, and a map that is 1
/// wherever the label of the top structure is one of the two cortex labels.
fn areas(
    image: &Image,
    top: &[u8],
    lists: &Lists,
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Tensor, RunError> {
    let is_cortex = |place: u8| {
        let part = lists.structures.get(usize::from(place));
        u8::from(part.is_some_and(|part| lists.cortex.contains(&part.label)))
    };
    let mut cortex = room(top.len())?;
    cortex.extend(top.iter().map(|&place| is_cortex(place)));
    let cortex = Tensor {
        shape: image.map(),
        numbers: Numbers::U8(cortex),
    };
    said(Stage::Cortex);
    let inputs = vec![image.copy()?, cortex];
    let answer = run_once(networks, Role::Cortex, image.size, inputs)?;
    map(answer, &image.map())
}
