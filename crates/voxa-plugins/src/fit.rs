//! A network against its description: the image inputs it takes, the size of each and the
//! names of the sides Voxa sets, and the answer it gives, of the kind and size the description
//! says. What this cannot know until the network runs (a side of an answer the runtime works
//! out) is checked again on each answer.

use crate::{
    Description, Dim, Element, Fit, Gives, Kind, Layout, Moves, NetworkFacts, Output, PluginError,
    Port,
};

#[cfg(test)]
mod tests;

/// The sentences a network that does not fit is refused with.
const INPUTS: &str = "The network takes a number of inputs other than its description says.";
const INPUT_KIND: &str = "The network's input is not of 32-bit numbers.";
const INPUT_RANK: &str = "The network's input does not have the five sides its description says.";
const INPUT_SIZE: &str = "The network's input is not the size its description gives.";
const INPUT_UNSET: &str = "The network's input has a side Voxa cannot set.";
const SEVERAL: &str =
    "The network gives more than one answer and the description does not say which to read.";
const NO_SUCH: &str = "The network has no answer of the name its description gives.";
const ANSWER_KIND: &str = "The network's answer is not of the kind its description says.";
const ANSWER_SIZE: &str = "The network's answer is not the size its description gives.";

/// How a network is wired to its description: what Voxa gives it and what it reads back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    /// The names of its image inputs, in its own order: one, or two for an alignment, the
    /// fixed scan's first.
    pub inputs: Vec<String>,
    /// The name of the output read.
    pub output: String,
    /// Each side of the inputs left free, by its name, with the size Voxa sets it to.
    pub free: Vec<(String, usize)>,
}

/// Checks a network's ports against its description, and says how to wire it.
///
/// # Errors
///
/// [`PluginError::Fit`] with the first thing found wrong.
pub fn fit(description: &Description, facts: &NetworkFacts) -> Result<Wiring, PluginError> {
    let inputs = if description.kind == Kind::Alignment {
        2
    } else {
        1
    };
    if facts.inputs.len() != inputs {
        return Err(PluginError::Fit(INPUTS));
    }
    let size = match description.input.fit {
        Fit::Whole(size) | Fit::Blocks { size, .. } => size,
    };
    let layout = description.input.layout;
    let mut free = Vec::new();
    for input in &facts.inputs {
        check_input(input, layout, size, &mut free)?;
    }
    let output = chosen(facts, from(&description.output))?;
    check_output(output, &description.output, layout, size)?;
    Ok(Wiring {
        inputs: facts.inputs.iter().map(|port| port.name.clone()).collect(),
        output: output.name.clone(),
        free,
    })
}

/// The five sides a tensor of `channels` channels has with this layout and spatial size.
fn sides(layout: Layout, size: [usize; 3], channels: usize) -> [usize; 5] {
    let [a, b, c] = size;
    match layout {
        Layout::ChannelsFirst => [1, channels, a, b, c],
        Layout::ChannelsLast => [1, a, b, c, channels],
    }
}

/// An image input: 32-bit numbers, five sides, each fixed to the size wanted or left free
/// under a name that Voxa then sets, a name never given two sizes.
fn check_input(
    port: &Port,
    layout: Layout,
    size: [usize; 3],
    free: &mut Vec<(String, usize)>,
) -> Result<(), PluginError> {
    if port.element != Element::F32 {
        return Err(PluginError::Fit(INPUT_KIND));
    }
    if port.dims.len() != 5 {
        return Err(PluginError::Fit(INPUT_RANK));
    }
    for (dim, wanted) in port.dims.iter().zip(sides(layout, size, 1)) {
        match dim {
            Dim::Fixed(fixed) if usize::try_from(*fixed) == Ok(wanted) => {}
            Dim::Fixed(_) => return Err(PluginError::Fit(INPUT_SIZE)),
            Dim::Named(name) => match free.iter().find(|(known, _)| known == name) {
                Some((_, set)) if *set != wanted => return Err(PluginError::Fit(INPUT_SIZE)),
                Some(_) => {}
                None => free.push((name.clone(), wanted)),
            },
            Dim::Unknown => return Err(PluginError::Fit(INPUT_UNSET)),
        }
    }
    Ok(())
}

/// The name of the output the description says to read, if it says.
fn from(output: &Output) -> Option<&str> {
    match output {
        Output::Labels(labels) => labels.from.as_deref(),
        Output::Number(number) => number.from.as_deref(),
        Output::ValueMap(map) => map.from.as_deref(),
        Output::Alignment(alignment) => alignment.from.as_deref(),
    }
}

/// The output read: the one named, or the only one.
fn chosen<'a>(facts: &'a NetworkFacts, from: Option<&str>) -> Result<&'a Port, PluginError> {
    let outputs = &facts.outputs;
    match from {
        Some(name) => outputs
            .iter()
            .find(|port| port.name == name)
            .ok_or(PluginError::Fit(NO_SUCH)),
        None => match outputs.as_slice() {
            [only] => Ok(only),
            [] => Err(PluginError::Fit(ANSWER_KIND)),
            _ => Err(PluginError::Fit(SEVERAL)),
        },
    }
}

/// The answer, of the kind and size the description's output says.
fn check_output(
    port: &Port,
    output: &Output,
    layout: Layout,
    size: [usize; 3],
) -> Result<(), PluginError> {
    let floats = || {
        if port.element == Element::F32 {
            Ok(())
        } else {
            Err(PluginError::Fit(ANSWER_KIND))
        }
    };
    match output {
        Output::Labels(labels) => match &labels.gives {
            Gives::Scores { channels } => {
                floats()?;
                shaped(port, &sides(layout, size, channels.len()))
            }
            Gives::Chance { .. } => {
                floats()?;
                shaped(port, &sides(layout, size, 1))
            }
            Gives::Labels => {
                let whole = matches!(port.element, Element::U8 | Element::I32 | Element::I64);
                if !whole {
                    return Err(PluginError::Fit(ANSWER_KIND));
                }
                let [a, b, c] = size;
                if port.dims.len() == 4 {
                    shaped(port, &[1, a, b, c])
                } else {
                    shaped(port, &sides(layout, size, 1))
                }
            }
        },
        Output::Number(_) => floats(),
        Output::ValueMap(_) => {
            floats()?;
            shaped(port, &sides(layout, size, 1))
        }
        Output::Alignment(alignment) => {
            floats()?;
            match alignment.gives {
                Moves::Field { .. } => shaped(port, &sides(Layout::ChannelsFirst, size, 3)),
                Moves::Matrix => Ok(()),
            }
        }
    }
}

/// An answer with as many sides as `wanted`, each fixed side the size wanted. A side left free
/// is worked out by the runtime, and checked on each answer.
fn shaped(port: &Port, wanted: &[usize]) -> Result<(), PluginError> {
    if port.dims.len() != wanted.len() {
        return Err(PluginError::Fit(ANSWER_SIZE));
    }
    let fits = port.dims.iter().zip(wanted).all(|(dim, wanted)| match dim {
        Dim::Fixed(fixed) => usize::try_from(*fixed) == Ok(*wanted),
        Dim::Named(_) | Dim::Unknown => true,
    });
    if fits {
        Ok(())
    } else {
        Err(PluginError::Fit(ANSWER_SIZE))
    }
}
