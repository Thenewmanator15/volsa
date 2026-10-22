// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `build_model` and the layer `MakeShape` in SynthSeg/predict_synthseg.py,
// translated to Rust and changed for Voxa.
//! The scores. `SynthSeg`'s scores network is given a box of one fixed size, cut from the map
//! of top structures so that what was found lies in its middle, with each structure turned
//! into its score class. The cut is `SynthSeg`'s own layer `MakeShape`, and the tests hold it
//! against what that layer makes.

use voxa_models::{Lists, Numbers, Role, RunError, Tensor};

use super::{Networks, Stage, exactly, floats, room, run_once};

/// `SynthSeg`'s layer leaves out the voxels whose top structure is at this place, as well as
/// those at place 0, when it looks for where the structures lie. It goes by the place among
/// the structures, not by a structure's label.
const LEFT_OUT: u8 = 24;
/// A score is rounded to four places: to so many parts of one.
const PARTS: f32 = 10_000.0;

/// A score for each score group of the lists. `top` is the top structure of each voxel of a
/// grid of this size, z changing fastest. The network is opened for the box, whatever the
/// size of the grid.
///
/// Of the network's answer the first number is dropped, as `SynthSeg` drops it: it goes with
/// score class 0, which is no group. The rest are held between 0 and 1 and rounded to four
/// places, a half going away from 0.
pub(super) fn scores(
    top: &[u8],
    size: [usize; 3],
    lists: &Lists,
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Vec<f32>, RunError> {
    let box_side = lists.box_side;
    let classes = Tensor {
        shape: vec![1, box_side, box_side, box_side],
        numbers: Numbers::U8(boxed(top, size, lists)?),
    };
    said(Stage::Checking);
    let answer = run_once(networks, Role::Scores, [box_side; 3], vec![classes])?;
    let [answer] = exactly(answer)?;
    let raw = floats(answer, &[1, lists.score_groups.len() + 1])?;
    let rounded = |&score: &f32| (score.clamp(0.0, 1.0) * PARTS).round() / PARTS;
    Ok(raw.iter().skip(1).map(rounded).collect())
}

/// The least place and one past the greatest, along one axis.
#[derive(Clone, Copy)]
struct Reach {
    from: usize,
    to: usize,
}

impl Reach {
    /// What reaches as far as either does.
    fn with(self, other: Self) -> Self {
        Self {
            from: self.from.min(other.from),
            to: self.to.max(other.to),
        }
    }
}

/// The box the scores network is given: a cube of the lists' box side, z changing fastest,
/// holding the score class of each voxel's top structure.
///
/// The box is laid over the grid so that what counts lies in its middle (see [`laid`]).
/// Where the box lies outside the grid there is nothing, which is place 0, and so it holds
/// the score class of place 0 there.
fn boxed(top: &[u8], size: [usize; 3], lists: &Lists) -> Result<Vec<u8>, RunError> {
    let box_side = lists.box_side;
    // The lists have been checked: every place has a class, and a byte holds it.
    let in_a_byte = |&class: &usize| u8::try_from(class).unwrap_or(u8::MAX);
    let classes: Vec<u8> = lists.score_class.iter().map(in_a_byte).collect();
    let class_of = |place: u8| classes.get(usize::from(place)).copied().unwrap_or_default();
    let [size_x, size_y, size_z] = size;
    let up_to_the_box = |along: usize| Reach {
        from: 0,
        to: along.min(box_side),
    };
    let reach = reach(top, size).unwrap_or_else(|| size.map(up_to_the_box));
    let [reach_x, reach_y, reach_z] = reach;
    let x = laid(reach_x, size_x, box_side);
    let y = laid(reach_y, size_y, box_side);
    let z = laid(reach_z, size_z, box_side);
    let voxels = box_side * box_side * box_side;
    let mut boxed = room(voxels)?;
    boxed.resize(voxels, class_of(0));
    // One line along z at a time.
    for dx in 0..x.count {
        for dy in 0..y.count {
            let from = ((x.from + dx) * size_y + y.from + dy) * size_z + z.from;
            let into = ((x.into + dx) * box_side + y.into + dy) * box_side + z.into;
            let theirs = top.get(from..from + z.count);
            let ours = boxed.get_mut(into..into + z.count);
            if let (Some(theirs), Some(ours)) = (theirs, ours) {
                for (class, &place) in ours.iter_mut().zip(theirs) {
                    *class = class_of(place);
                }
            }
        }
    }
    Ok(boxed)
}

/// How far along each axis the voxels that count reach: those whose top structure is neither
/// place 0 nor the place `SynthSeg` leaves out. `None` when no voxel counts.
fn reach(top: &[u8], [_, size_y, size_z]: [usize; 3]) -> Option<[Reach; 3]> {
    let counts = |&place: &u8| place != 0 && place != LEFT_OUT;
    let one = |place: usize| Reach {
        from: place,
        to: place + 1,
    };
    let mut reach: Option<[Reach; 3]> = None;
    for (line, places) in top.chunks_exact(size_z).enumerate() {
        let (Some(first), Some(last)) = (
            places.iter().position(counts),
            places.iter().rposition(counts),
        ) else {
            continue;
        };
        let along_z = Reach {
            from: first,
            to: last + 1,
        };
        let here = [one(line / size_y), one(line % size_y), along_z];
        reach = Some(match reach {
            None => here,
            Some([x, y, z]) => {
                let [here_x, here_y, here_z] = here;
                [x.with(here_x), y.with(here_y), z.with(here_z)]
            }
        });
    }
    reach
}

/// How the box lies over the grid along one axis: `count` voxels are copied, from the grid's
/// place `from` on, to the box's place `into` on.
struct Laid {
    from: usize,
    into: usize,
    count: usize,
}

/// Lays a box along one axis of a grid that is `grid` voxels long, round the stretch `reach`
/// of it. This is `SynthSeg`'s rule. With `e` the length of the stretch, the stretch's start
/// moves back by half of `box_side - e` rounded up, and its end moves on by half of it
/// rounded down, so that the two are `box_side` apart. A stretch longer than the box is cut
/// by the same sum: its start moves on by half of what hangs over rounded down, and its end
/// moves back by the rest. Only what then lies in the grid is copied.
fn laid(reach: Reach, grid: usize, box_side: usize) -> Laid {
    let length = reach.to.saturating_sub(reach.from);
    let (from, into) = match box_side.checked_sub(length) {
        Some(room) => {
            let back = room.div_ceil(2);
            (
                reach.from.saturating_sub(back),
                back.saturating_sub(reach.from),
            )
        }
        None => (reach.from + (length - box_side) / 2, 0),
    };
    let count = (box_side - into).min(grid.saturating_sub(from));
    Laid { from, into, count }
}
