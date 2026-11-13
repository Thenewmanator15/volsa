//! Putting the dark tubes found on one scan onto labels already open, as one more part, so
//! that both are shown together. The labels keep their own grid: a voxel of theirs is taken
//! to be in a tube when any of 27 points spread through it falls in a voxel found to be one,
//! so a tube thinner than their voxels is not lost between their centres.

use rayon::prelude::*;
use volsa_core::{Point, Voxel};
use volsa_labels::{LabelGrid, LabelSet, Part, Side};

use super::{VEINS_COLOUR, VEINS_GROUP, VEINS_HOW, VEINS_PART, VeinsError, whole};

/// Where the points are spread through a voxel, along each side, in voxels from its centre.
const SPREAD: [f64; 3] = [-1.0 / 3.0, 0.0, 1.0 / 3.0];

/// `set` with the tubes of `veins` (1 for a tube, on a grid of its own) as one more part, drawn
/// over any part a voxel had. A part of that name from before is taken over, not added again.
/// The voxels and volume of every part are counted again.
///
/// # Errors
///
/// [`VeinsError::NoneFound`] when no tube falls inside the labels' box,
/// [`VeinsError::TooLarge`] when the labels have no room for one more part, and
/// [`VeinsError::NoMemory`] when there is not the room for the new numbers.
pub fn veins_onto(set: &LabelSet, veins: &LabelGrid) -> Result<LabelSet, VeinsError> {
    let before = set.parts.iter().position(|part| part.name == VEINS_PART);
    let own = before.unwrap_or(set.parts.len());
    let number = u16::try_from(own + 1).map_err(|_| VeinsError::TooLarge)?;
    let numbers = marked(set, veins, number)?;
    let mut parts = set.parts.clone();
    let tubes = Part {
        name: VEINS_PART.to_owned(),
        group: VEINS_GROUP.to_owned(),
        side: Side::Midline,
        colour: VEINS_COLOUR,
        trust: volsa_labels::Trust::Approximate(VEINS_HOW.to_owned()),
        voxels: 0,
        ml: 0.0,
    };
    match parts.get_mut(own) {
        Some(part) => *part = tubes,
        None => parts.push(tubes),
    }
    counted(&mut parts, &numbers, set);
    if parts.get(own).is_none_or(|part| part.voxels == 0) {
        return Err(VeinsError::NoneFound);
    }
    Ok(LabelSet {
        numbers,
        parts,
        ..set.clone()
    })
}

/// The numbers of `set` with every voxel in a tube of `veins` made `number`, and no other.
fn marked(set: &LabelSet, veins: &LabelGrid, number: u16) -> Result<Vec<u16>, VeinsError> {
    let to_veins = veins
        .placement
        .inverse()
        .map(|back| set.placement.then(&back))
        .ok_or(VeinsError::NoneFound)?;
    let sides = veins.dims.map(|side| usize::try_from(side).unwrap_or(0));
    let in_tube = |at: Point<Voxel>| {
        let index = |value: f64, side: usize| {
            let rounded = value.round();
            (rounded >= 0.0 && whole(rounded) < side).then(|| whole(rounded))
        };
        let [columns, rows, slices] = sides;
        let found = index(at.x, columns)
            .zip(index(at.y, rows))
            .zip(index(at.z, slices))
            .map(|((x, y), z)| (z * rows + y) * columns + x);
        found.and_then(|place| veins.ids.get(place)) == Some(&1)
    };
    let [columns, rows, _] = set.dims.map(|side| usize::try_from(side).unwrap_or(0));
    let plane = columns * rows;
    let mut numbers = super::zeroed::<u16>(set.numbers.len())?;
    numbers.copy_from_slice(&set.numbers);
    numbers
        .par_chunks_mut(plane.max(1))
        .enumerate()
        .for_each(|(slice, out)| {
            for (at, slot) in out.iter_mut().enumerate() {
                if *slot == number {
                    *slot = 0;
                }
                let centre = [at % columns, at / columns, slice].map(|value| value as f64);
                let hit = SPREAD.iter().any(|dz| {
                    SPREAD.iter().any(|dy| {
                        SPREAD.iter().any(|dx| {
                            let [x, y, z] = centre;
                            in_tube(to_veins.apply(Point::new(x + dx, y + dy, z + dz)))
                        })
                    })
                });
                if hit {
                    *slot = number;
                }
            }
        });
    Ok(numbers)
}

/// Count again the voxels, and so the volume, of every part, from `numbers` on `set`'s grid.
fn counted(parts: &mut [Part], numbers: &[u16], set: &LabelSet) {
    let mut counts = vec![0_u64; parts.len() + 1];
    for number in numbers {
        if let Some(count) = counts.get_mut(usize::from(*number)) {
            *count += 1;
        }
    }
    let [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] = set.placement.columns();
    let voxel_ml = (ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx))
        .abs()
        / 1000.0;
    for (part, count) in parts.iter_mut().zip(counts.iter().skip(1)) {
        part.voxels = *count;
        part.ml = *count as f64 * voxel_ml;
    }
}
