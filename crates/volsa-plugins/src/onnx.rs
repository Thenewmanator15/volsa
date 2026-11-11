//! Reading a network file's structure before ONNX Runtime ever sees it: what it takes and
//! gives, and whether it asks for anything outside standard ONNX.
//!
//! The file is read by the wire format of Protocol Buffers, field by field, its weights passed
//! over unread, so a network of 2 GiB is checked in a few kilobytes of memory. Every operator
//! set, every node in every graph (the graphs inside `If`, `Loop` and `Scan` included, to a
//! depth of 16), and every tensor that keeps its numbers in another file is looked at. A
//! network is refused for an operator outside the standard domain, functions of its own, data
//! kept anywhere but a file its description lists, or versions newer than the runtime runs.
//! The file's own words of itself (its producer, its notes, its metadata) are never read.

use std::fs::File;
use std::path::Path;

use volsa_models::Supported;

use crate::Listed;
use graph::{Main, Walk, standard};
use wire::Wire;

mod graph;
#[cfg(test)]
mod limit_tests;
#[cfg(test)]
pub(crate) mod tests;
mod wire;

/// What a network takes and gives, as its file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFacts {
    /// Its inputs, in the file's order, without the tensors of fixed numbers.
    pub inputs: Vec<Port>,
    /// Its outputs.
    pub outputs: Vec<Port>,
}

/// An input or an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    /// Its name.
    pub name: String,
    /// Its element type.
    pub element: Element,
    /// Its dimensions.
    pub dims: Vec<Dim>,
}

/// An element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// 32-bit floats.
    F32,
    /// Bytes.
    U8,
    /// 32-bit integers.
    I32,
    /// 64-bit integers.
    I64,
    /// Another.
    Other(i32),
}

/// A dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dim {
    /// Fixed.
    Fixed(u64),
    /// Free, by name.
    Named(String),
    /// Free, unnamed.
    Unknown,
}

/// Why a network file is refused. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkError {
    /// It is not a network file, or it is damaged: cut short, a length past its end.
    #[error("The network file is damaged or is not an ONNX network.")]
    NotANetwork,
    /// It asks for operators of a domain other than the standard one: the domain, for the log.
    #[error("The network uses operators outside standard ONNX, which Volsa does not run.")]
    Domain(String),
    /// It holds functions of its own.
    #[error("The network holds functions of its own, which Volsa does not run.")]
    Functions,
    /// A tensor keeps its numbers somewhere other than a part of a listed file.
    #[error("The network keeps data in a file its description does not list.")]
    Outside,
    /// Its IR version or its operators are newer than the runtime runs.
    #[error("The network needs a newer ONNX Runtime than the one Volsa uses.")]
    TooNew,
    /// Its graphs nest more than 16 deep, it has more than 100 000 nodes, a name is too long, or
    /// it holds more ports, dimensions, operator sets or fixed tensors than the reader keeps.
    #[error("The network is nested too deeply or is too large for Volsa to check.")]
    TooDeep,
}

/// Reads the structure of the network file at `path`, whose description lists `listed`, and
/// checks it against what the runtime runs.
///
/// # Errors
///
/// [`NetworkError`] says the first thing found wrong.
pub fn read_network(
    path: &Path,
    listed: &[Listed],
    supported: Supported,
) -> Result<NetworkFacts, NetworkError> {
    let file = File::open(path).map_err(|_| NetworkError::NotANetwork)?;
    let end = file
        .metadata()
        .map_err(|_| NetworkError::NotANetwork)?
        .len();
    let mut wire = Wire::new(file);
    let mut walk = Walk { listed, nodes: 0 };
    let model = graph::model(&mut wire, end, &mut walk)?;
    let main = model.main.ok_or(NetworkError::NotANetwork)?;
    versions(&model.opsets, model.ir_version, supported)?;
    let Main {
        inputs,
        outputs,
        fixed,
    } = main;
    let inputs = inputs
        .into_iter()
        .filter(|port| !fixed.contains(&port.name))
        .collect();
    Ok(NetworkFacts { inputs, outputs })
}

/// The model's versions: an IR version it has and the runtime runs, and operator sets of the
/// standard domain only, at least one, none newer than the runtime runs.
fn versions(
    opsets: &[(Option<String>, u64)],
    ir_version: u64,
    supported: Supported,
) -> Result<(), NetworkError> {
    if ir_version == 0 {
        return Err(NetworkError::NotANetwork);
    }
    if let Some((domain, _)) = opsets
        .iter()
        .find(|(domain, _)| !standard(domain.as_deref()))
    {
        return Err(NetworkError::Domain(domain.clone().unwrap_or_default()));
    }
    if opsets.is_empty() {
        return Err(NetworkError::NotANetwork);
    }
    let newer = |version: u64, most: i64| u64::try_from(most).map_or(true, |most| version > most);
    let too_new = newer(ir_version, supported.ir_version)
        || opsets
            .iter()
            .any(|(_, version)| newer(*version, supported.opset));
    if too_new {
        Err(NetworkError::TooNew)
    } else {
        Ok(())
    }
}
