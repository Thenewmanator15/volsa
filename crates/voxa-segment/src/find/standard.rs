//! The standard network. It runs twice, on the image and on the image's mirror, and its two
//! answers are averaged: `SynthSeg` does the same inside its own model (`build_model`, where it
//! is given `flip_indices`).

use voxa_models::{Numbers, Role, RunError, Tensor};

use super::{Halves, Image, NOT_FINITE, Networks, Stage, all_finite, failed, room};

/// The chance of each structure at each voxel, the structures changing fastest: the mean of
/// the network's answer on the image and its answer on the mirror. `flip` has a place for
/// each structure.
///
/// The second answer is blended into the first where the first lies, so no more than two
/// sets of chances are held at once.
pub(super) fn chances(
    image: &Image,
    flip: &[usize],
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Vec<f32>, RunError> {
    let count = flip.len();
    said(Stage::Finding { step: 1, of: 2 });
    let mut network = networks.open(Role::Standard, image.size)?;
    let answer = network.run(vec![image.copy()?])?;
    let mut chances = Halves::of(answer, image, count)?.joined()?;
    said(Stage::Finding { step: 2, of: 2 });
    let answer = network.run(vec![mirror(image)?])?;
    drop(network);
    let on_the_mirror = Halves::of(answer, image, count)?;
    blend(&mut chances, &on_the_mirror, image, flip);
    drop(on_the_mirror);
    // Two numbers that are each finite can have a sum that is not.
    if all_finite(&chances) {
        Ok(chances)
    } else {
        Err(failed(NOT_FINITE))
    }
}

/// The image turned round along x: its slabs, each of one x, in the other order.
fn mirror(image: &Image) -> Result<Tensor, RunError> {
    let [_, y, z] = image.size;
    let mut values = room(image.values.len())?;
    for slab in image.values.chunks_exact(y * z).rev() {
        values.extend_from_slice(slab);
    }
    Ok(Tensor {
        shape: image.with(1),
        numbers: Numbers::F32(values),
    })
}

/// Blends the answer on the mirror into the answer on the image, which `chances` holds. The
/// mirror's answer is turned back along x, and its channels are put in the order `flip`
/// gives: channel `i` is its channel `flip[i]`. Each number then becomes half the sum of the
/// two, in single precision, as `SynthSeg` works it out.
#[expect(
    clippy::manual_midpoint,
    reason = "SynthSeg adds the two and halves the sum, and its numbers are kept to the last bit"
)]
fn blend(chances: &mut [f32], on_the_mirror: &Halves, image: &Image, flip: &[usize]) {
    let [_, y, z] = image.size;
    let slab = y * z;
    let (wide, rest) = (on_the_mirror.wide, on_the_mirror.rest);
    // The voxels of the mirror's answer in the order of the image's own: its slabs, each of
    // one x, taken from the far end.
    let firsts = on_the_mirror.first.chunks_exact(slab * wide).rev();
    let seconds = on_the_mirror.second.chunks_exact(slab * rest).rev();
    let firsts = firsts.flat_map(|slab| slab.chunks_exact(wide));
    let seconds = seconds.flat_map(|slab| slab.chunks_exact(rest));
    let ours = chances.chunks_exact_mut(flip.len());
    for ((ours, first), second) in ours.zip(firsts).zip(seconds) {
        for (chance, &old) in ours.iter_mut().zip(flip) {
            // A channel lies in the first half, or so many places into the second.
            let theirs = match old.checked_sub(wide) {
                None => first.get(old),
                Some(place) => second.get(place),
            };
            if let Some(&theirs) = theirs {
                *chance = 0.5 * (*chance + theirs);
            }
        }
    }
}
