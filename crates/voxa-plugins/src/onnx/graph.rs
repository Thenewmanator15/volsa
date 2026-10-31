//! The messages of `onnx.proto` that the check reads: the model, its operator sets, its graph
//! and every graph inside a node's attributes, each node's domain, where each tensor's numbers
//! are kept, and the main graph's inputs and outputs. Everything else is passed over.

use std::collections::HashSet;
use std::io::{Read, Seek};

use super::wire::{LENGTH, VARINT, Wire};
use super::{Dim, Element, NetworkError, Port};
use crate::Listed;

/// The deepest a graph may lie inside the attributes of nodes: the main graph is 0.
const MOST_DEPTH: usize = 16;
/// The most nodes, in every graph together.
const MOST_NODES: usize = 100_000;
/// The longest a name or a domain is read, in bytes.
const MOST_TEXT: u64 = 1 << 16;

/// What the walk has met so far, and what it checks against.
pub(super) struct Walk<'a> {
    /// The files the description lists: the only places a tensor's numbers may be kept.
    pub(super) listed: &'a [Listed],
    /// How many nodes have been met.
    pub(super) nodes: usize,
}

/// What the model says of itself.
#[derive(Default)]
pub(super) struct Model {
    /// Its IR version.
    pub(super) ir_version: u64,
    /// The operator sets it asks for, each a domain and a version.
    pub(super) opsets: Vec<(Option<String>, u64)>,
    /// Its main graph's ports, once its graph has been read.
    pub(super) main: Option<Main>,
}

/// The main graph's ports.
#[derive(Default)]
pub(super) struct Main {
    /// Its inputs, those that are tensors of fixed numbers among them.
    pub(super) inputs: Vec<Port>,
    /// Its outputs.
    pub(super) outputs: Vec<Port>,
    /// The names of its tensors of fixed numbers.
    pub(super) fixed: HashSet<String>,
}

/// Whether a domain is that of the standard operators.
pub(super) fn standard(domain: Option<&str>) -> bool {
    matches!(domain, Some("" | "ai.onnx"))
}

/// Reads the model, which runs to `end`.
pub(super) fn model<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    walk: &mut Walk,
) -> Result<Model, NetworkError> {
    let mut model = Model::default();
    while wire.at() < end {
        match wire.tag(end)? {
            (1, VARINT) => model.ir_version = wire.varint(end)?,
            (7, LENGTH) => {
                let ends = wire.length(end)?;
                let mut main = Main::default();
                graph(wire, ends, 0, walk, Some(&mut main))?;
                model.main = Some(main);
            }
            (8, LENGTH) => {
                let ends = wire.length(end)?;
                model.opsets.push(opset(wire, ends)?);
            }
            (25, _) => return Err(NetworkError::Functions),
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(model)
}

/// An operator set: its domain (`None` when too long to be a standard one) and version.
fn opset<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
) -> Result<(Option<String>, u64), NetworkError> {
    let (mut domain, mut version) = (Some(String::new()), 0);
    while wire.at() < end {
        match wire.tag(end)? {
            (1, LENGTH) => domain = wire.text(end, MOST_TEXT)?,
            (2, VARINT) => version = wire.varint(end)?,
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok((domain, version))
}

/// Reads a graph lying `depth` deep. The main graph's ports and fixed tensors are kept in
/// `main`; a subgraph's are not.
fn graph<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    depth: usize,
    walk: &mut Walk,
    mut main: Option<&mut Main>,
) -> Result<(), NetworkError> {
    if depth > MOST_DEPTH {
        return Err(NetworkError::TooDeep);
    }
    while wire.at() < end {
        let (field, kind) = wire.tag(end)?;
        if kind != LENGTH {
            wire.skip(kind, end)?;
            continue;
        }
        let ends = wire.length(end)?;
        match (field, main.as_deref_mut()) {
            (1, _) => node(wire, ends, depth, walk)?,
            (5, main) => {
                let name = tensor(wire, ends, walk)?;
                if let (Some(main), Some(name)) = (main, name) {
                    main.fixed.insert(name);
                }
            }
            (15, _) => sparse(wire, ends, walk)?,
            (11, Some(main)) => main.inputs.push(port(wire, ends)?),
            (12, Some(main)) => main.outputs.push(port(wire, ends)?),
            _ => wire.skip_to(ends)?,
        }
    }
    Ok(())
}

/// Reads a node: its domain must be the standard one, and its attributes are read for the
/// graphs and tensors they may hold.
fn node<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    depth: usize,
    walk: &mut Walk,
) -> Result<(), NetworkError> {
    walk.nodes += 1;
    if walk.nodes > MOST_NODES {
        return Err(NetworkError::TooDeep);
    }
    while wire.at() < end {
        match wire.tag(end)? {
            (5, LENGTH) => {
                let ends = wire.length(end)?;
                attribute(wire, ends, depth, walk)?;
            }
            (7, LENGTH) => {
                let domain = wire.text(end, MOST_TEXT)?;
                if !standard(domain.as_deref()) {
                    return Err(NetworkError::Domain(domain.unwrap_or_default()));
                }
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(())
}

/// Reads an attribute for the graphs and tensors it may hold.
fn attribute<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    depth: usize,
    walk: &mut Walk,
) -> Result<(), NetworkError> {
    while wire.at() < end {
        let (field, kind) = wire.tag(end)?;
        if kind != LENGTH {
            wire.skip(kind, end)?;
            continue;
        }
        let ends = wire.length(end)?;
        match field {
            5 | 10 => tensor(wire, ends, walk).map(|_| ())?,
            6 | 11 => graph(wire, ends, depth + 1, walk, None)?,
            22 | 23 => sparse(wire, ends, walk)?,
            _ => wire.skip_to(ends)?,
        }
    }
    Ok(())
}

/// Reads a sparse tensor for its two tensors.
fn sparse<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    walk: &mut Walk,
) -> Result<(), NetworkError> {
    while wire.at() < end {
        match wire.tag(end)? {
            (1 | 2, LENGTH) => {
                let ends = wire.length(end)?;
                tensor(wire, ends, walk)?;
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(())
}

/// Reads a tensor: its name, and where its numbers are kept, which must be the file itself or
/// a part of a file the description lists. Its numbers are passed over.
fn tensor<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    walk: &Walk,
) -> Result<Option<String>, NetworkError> {
    let (mut name, mut external, mut entries) = (None, false, External::default());
    while wire.at() < end {
        match wire.tag(end)? {
            (8, LENGTH) => name = wire.text(end, MOST_TEXT)?,
            (13, LENGTH) => {
                let ends = wire.length(end)?;
                entries.read(wire, ends)?;
            }
            (14, VARINT) => external = wire.varint(end)? == 1,
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    if external && !entries.within(walk.listed) {
        return Err(NetworkError::Outside);
    }
    Ok(name)
}

/// Where a tensor kept outside the file says its numbers are.
#[derive(Default)]
struct External {
    location: Option<String>,
    offset: Option<String>,
    length: Option<String>,
}

impl External {
    /// Reads one entry: a key and its value.
    fn read<R: Read + Seek>(&mut self, wire: &mut Wire<R>, end: u64) -> Result<(), NetworkError> {
        let (mut key, mut value) = (None, None);
        while wire.at() < end {
            match wire.tag(end)? {
                (1, LENGTH) => key = wire.text(end, MOST_TEXT)?,
                (2, LENGTH) => value = wire.text(end, MOST_TEXT)?,
                (_, kind) => wire.skip(kind, end)?,
            }
        }
        match key.as_deref() {
            Some("location") => self.location = value,
            Some("offset") => self.offset = value,
            Some("length") => self.length = value,
            _ => {}
        }
        Ok(())
    }

    /// Whether the numbers lie inside a listed file: its name plain and listed, and the offset
    /// and the length within it.
    fn within(&self, listed: &[Listed]) -> bool {
        let Some(location) = self.location.as_deref() else {
            return false;
        };
        let Some(file) = listed.iter().find(|file| file.name == location) else {
            return false;
        };
        let number = |text: &Option<String>| text.as_deref().map(str::parse::<u64>);
        let offset = match number(&self.offset) {
            None => 0,
            Some(Ok(offset)) => offset,
            Some(Err(_)) => return false,
        };
        let length = match number(&self.length) {
            None => file.bytes.checked_sub(offset),
            Some(Ok(length)) => Some(length),
            Some(Err(_)) => return false,
        };
        voxa_models::is_plain_name(location)
            && length
                .and_then(|length| offset.checked_add(length))
                .is_some_and(|ends| ends <= file.bytes)
    }
}

/// Reads a port: its name, its element type and its dimensions.
fn port<R: Read + Seek>(wire: &mut Wire<R>, end: u64) -> Result<Port, NetworkError> {
    let mut port = Port {
        name: String::new(),
        element: Element::Other(0),
        dims: Vec::new(),
    };
    while wire.at() < end {
        match wire.tag(end)? {
            (1, LENGTH) => {
                port.name = wire.text(end, MOST_TEXT)?.ok_or(NetworkError::TooDeep)?;
            }
            (2, LENGTH) => {
                let ends = wire.length(end)?;
                each(wire, ends, 1, |wire, ends| {
                    tensor_type(wire, ends, &mut port)
                })?;
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(port)
}

/// Reads a tensor's type: its element type and its shape's dimensions.
fn tensor_type<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    port: &mut Port,
) -> Result<(), NetworkError> {
    while wire.at() < end {
        match wire.tag(end)? {
            (1, VARINT) => {
                let code = i32::try_from(wire.varint(end)?).unwrap_or(-1);
                port.element = match code {
                    1 => Element::F32,
                    2 => Element::U8,
                    6 => Element::I32,
                    7 => Element::I64,
                    other => Element::Other(other),
                };
            }
            (2, LENGTH) => {
                let ends = wire.length(end)?;
                each(wire, ends, 1, |wire, ends| {
                    port.dims.push(dimension(wire, ends)?);
                    Ok(())
                })?;
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(())
}

/// Reads one dimension.
fn dimension<R: Read + Seek>(wire: &mut Wire<R>, end: u64) -> Result<Dim, NetworkError> {
    let mut dim = Dim::Unknown;
    while wire.at() < end {
        match wire.tag(end)? {
            (1, VARINT) => {
                let size = wire.varint(end)?;
                dim = if size.cast_signed() < 0 {
                    Dim::Unknown
                } else {
                    Dim::Fixed(size)
                };
            }
            (2, LENGTH) => {
                dim = wire.text(end, MOST_TEXT)?.map_or(Dim::Unknown, Dim::Named);
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(dim)
}

/// Calls `read` on every field numbered `wanted` of the length kind in a message that runs to
/// `end`, passing over the rest.
fn each<R: Read + Seek>(
    wire: &mut Wire<R>,
    end: u64,
    wanted: u64,
    mut read: impl FnMut(&mut Wire<R>, u64) -> Result<(), NetworkError>,
) -> Result<(), NetworkError> {
    while wire.at() < end {
        match wire.tag(end)? {
            (field, LENGTH) if field == wanted => {
                let ends = wire.length(end)?;
                read(wire, ends)?;
            }
            (_, kind) => wire.skip(kind, end)?,
        }
    }
    Ok(())
}
