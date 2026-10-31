//! A block's answer finished to labels as the description says: the channel of the highest
//! score, a chance above its threshold, or labels as given. The answer is checked first, as
//! `SynthSeg`'s are: of the kind and size expected, and every number finite.

use std::collections::HashSet;

use voxa_models::{Numbers, Tensor};

use crate::{Gives, LabelOutput, Layout};

#[cfg(test)]
mod tests;

/// Why an answer could not be finished. Each sentence is fit to be shown.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize,
)]
pub enum FinishError {
    /// The answer is not of the size or kind its description says.
    #[error("The plugin gave an answer of the wrong size or kind.")]
    WrongAnswer,
    /// A number of the answer is not finite.
    #[error("The plugin gave an answer with a number that is not finite.")]
    NotFinite,
    /// A label of the answer is neither 0 nor named by the description.
    #[error("The plugin gave labels its description does not name.")]
    Unnamed,
    /// No voxel of the scan was labelled.
    #[error("The plugin labelled nothing in this scan.")]
    Nothing,
    /// There was not the memory to finish the answer.
    #[error("There was not enough memory to finish the plugin's answer.")]
    NoRoom,
}

/// The labels of a block of `block` voxels from the network's answer, in the tensor's order
/// (the last axis fastest).
///
/// # Errors
///
/// [`FinishError`] when the answer is not what the description says.
pub fn labels_of(
    answer: Tensor,
    layout: Layout,
    block: [usize; 3],
    output: &LabelOutput,
) -> Result<Vec<u16>, FinishError> {
    let voxels = block
        .iter()
        .try_fold(1_usize, |count, side| count.checked_mul(*side))
        .ok_or(FinishError::WrongAnswer)?;
    let mut labels = room(voxels)?;
    match &output.gives {
        Gives::Scores { channels } => {
            let scores = floats(answer, &shape(layout, block, channels.len()))?;
            let label = |channel: usize| channels.get(channel).copied().unwrap_or(0);
            match layout {
                Layout::ChannelsFirst => {
                    for voxel in 0..voxels {
                        let at = |channel: usize| scores.get(channel * voxels + voxel).copied();
                        labels.push(label(highest((0..channels.len()).map_while(at))));
                    }
                }
                Layout::ChannelsLast => {
                    for voxel in scores.chunks_exact(channels.len().max(1)) {
                        labels.push(label(highest(voxel.iter().copied())));
                    }
                }
            }
        }
        Gives::Chance { label, threshold } => {
            let chances = floats(answer, &shape(layout, block, 1))?;
            let marked = chances
                .iter()
                .map(|chance| if chance > threshold { *label } else { 0 });
            labels.extend(marked);
        }
        Gives::Labels => given(answer, layout, block, output, &mut labels)?,
    }
    Ok(labels)
}

/// The shape of an answer of `channels` channels over a block, with this layout.
fn shape(layout: Layout, block: [usize; 3], channels: usize) -> Vec<usize> {
    let [a, b, c] = block;
    match layout {
        Layout::ChannelsFirst => vec![1, channels, a, b, c],
        Layout::ChannelsLast => vec![1, a, b, c, channels],
    }
}

/// The place of the highest of some numbers, the first when several are equal.
fn highest(numbers: impl Iterator<Item = f32>) -> usize {
    let mut best = (0, f32::NEG_INFINITY);
    for (place, number) in numbers.enumerate() {
        if number > best.1 {
            best = (place, number);
        }
    }
    best.0
}

/// The numbers of an answer that should be floats of this shape, every one finite.
fn floats(answer: Tensor, shape: &[usize]) -> Result<Vec<f32>, FinishError> {
    let Numbers::F32(numbers) = answer.numbers else {
        return Err(FinishError::WrongAnswer);
    };
    if answer.shape != shape || Some(numbers.len()) != holds(shape) {
        return Err(FinishError::WrongAnswer);
    }
    if !numbers.iter().all(|number| number.is_finite()) {
        return Err(FinishError::NotFinite);
    }
    Ok(numbers)
}

/// Labels as the network gives them: whole numbers, one for each voxel, each 0 or named.
fn given(
    answer: Tensor,
    layout: Layout,
    block: [usize; 3],
    output: &LabelOutput,
    labels: &mut Vec<u16>,
) -> Result<(), FinishError> {
    let [a, b, c] = block;
    let four = answer.shape == [1, a, b, c];
    if !four && answer.shape != shape(layout, block, 1) {
        return Err(FinishError::WrongAnswer);
    }
    let wide: Vec<i64> = match answer.numbers {
        Numbers::U8(numbers) => numbers.into_iter().map(i64::from).collect(),
        Numbers::I32(numbers) => numbers.into_iter().map(i64::from).collect(),
        Numbers::I64(numbers) => numbers,
        Numbers::F32(_) => return Err(FinishError::WrongAnswer),
    };
    if Some(wide.len()) != holds(&answer.shape) {
        return Err(FinishError::WrongAnswer);
    }
    let named: HashSet<u16> = output.names.iter().map(|name| name.id).collect();
    for label in wide {
        let label = u16::try_from(label).map_err(|_| FinishError::Unnamed)?;
        if label != 0 && !named.contains(&label) {
            return Err(FinishError::Unnamed);
        }
        labels.push(label);
    }
    Ok(())
}

/// How many numbers a shape holds.
fn holds(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1_usize, |count, side| count.checked_mul(*side))
}

/// A list with room for so many labels, asked for in a way that can be refused.
pub(crate) fn room(count: usize) -> Result<Vec<u16>, FinishError> {
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(count)
        .map_err(|_| FinishError::NoRoom)?;
    Ok(labels)
}
