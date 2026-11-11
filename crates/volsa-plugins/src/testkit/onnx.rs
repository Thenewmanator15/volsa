//! Small ONNX files written byte by byte, so that tests need no network made elsewhere: a
//! network of one 1 x 1 x 1 convolution that ONNX Runtime can run, and networks of any shape
//! for the checks of the file's structure.
//!
//! The numbers are the fields of `onnx.proto`.

/// One dimension of a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimSpec {
    /// A fixed size.
    Fixed(i64),
    /// A size left free, by its name.
    Named(String),
}

/// An input or an output of a graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortSpec {
    /// Its name.
    pub name: String,
    /// Its element type, by ONNX's number: 1 for 32-bit floats, 2 for bytes, 7 for 64-bit
    /// integers.
    pub element: i32,
    /// Its dimensions.
    pub dims: Vec<DimSpec>,
}

/// A tensor of fixed numbers in a graph.
#[derive(Debug, Clone, PartialEq)]
pub struct InitSpec {
    /// Its name.
    pub name: String,
    /// Its dimensions.
    pub dims: Vec<i64>,
    /// Its numbers, when they are in the file.
    pub floats: Vec<f32>,
    /// Where its numbers are when they are in another file: its name, the offset and the
    /// length, as text the way ONNX keeps them.
    pub external: Option<(String, String, String)>,
}

/// An attribute of a node.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrSpec {
    /// A list of integers.
    Ints(String, Vec<i64>),
    /// A graph.
    Graph(String, Box<GraphSpec>),
    /// A tensor.
    Tensor(String, InitSpec),
}

/// A node of a graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeSpec {
    /// Its operator.
    pub op: String,
    /// Its domain; empty for the standard one.
    pub domain: String,
    /// The names of its inputs.
    pub inputs: Vec<String>,
    /// The names of its outputs.
    pub outputs: Vec<String>,
    /// Its attributes.
    pub attributes: Vec<AttrSpec>,
}

/// A graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphSpec {
    /// Its nodes.
    pub nodes: Vec<NodeSpec>,
    /// Its inputs.
    pub inputs: Vec<PortSpec>,
    /// Its outputs.
    pub outputs: Vec<PortSpec>,
    /// Its tensors of fixed numbers.
    pub initializers: Vec<InitSpec>,
}

/// A whole network file.
#[derive(Debug, Clone, PartialEq)]
pub struct NetSpec {
    /// Its IR version.
    pub ir_version: i64,
    /// The operator sets it asks for: a domain and a version each.
    pub opsets: Vec<(String, i64)>,
    /// Its graph.
    pub graph: GraphSpec,
    /// How many functions of its own it holds.
    pub functions: usize,
}

/// A port of 32-bit floats with these dimensions.
pub fn floats(name: &str, dims: Vec<DimSpec>) -> PortSpec {
    PortSpec {
        name: name.to_owned(),
        element: 1,
        dims,
    }
}

/// `[1, 1, x, y, z]`, the three sides left free by those names.
pub fn image_input() -> PortSpec {
    let named = |name: &str| DimSpec::Named(name.to_owned());
    let dims = vec![
        DimSpec::Fixed(1),
        DimSpec::Fixed(1),
        named("x"),
        named("y"),
        named("z"),
    ];
    floats("image", dims)
}

/// A network of one 1 x 1 x 1 convolution: each output channel `c` is
/// `weights[c] * input + bias[c]`. Its input is `input`, `[1, 1, d0, d1, d2]`; its output
/// `scores`, `[1, channels, d0, d1, d2]`. Operators of version 17, IR version 8.
pub fn conv_network(input: &PortSpec, weights: &[f32], bias: &[f32]) -> Vec<u8> {
    let channels = i64::try_from(weights.len()).unwrap_or(i64::MAX);
    let mut output_dims = input.dims.clone();
    if let Some(dim) = output_dims.get_mut(1) {
        *dim = DimSpec::Fixed(channels);
    }
    let conv = NodeSpec {
        op: "Conv".to_owned(),
        inputs: vec![input.name.clone(), "w".to_owned(), "b".to_owned()],
        outputs: vec!["scores".to_owned()],
        attributes: vec![AttrSpec::Ints("kernel_shape".to_owned(), vec![1, 1, 1])],
        ..NodeSpec::default()
    };
    let tensor = |name: &str, dims: Vec<i64>, floats: &[f32]| InitSpec {
        name: name.to_owned(),
        dims,
        floats: floats.to_vec(),
        external: None,
    };
    let graph = GraphSpec {
        nodes: vec![conv],
        inputs: vec![input.clone()],
        outputs: vec![floats("scores", output_dims)],
        initializers: vec![
            tensor("w", vec![channels, 1, 1, 1, 1], weights),
            tensor("b", vec![channels], bias),
        ],
    };
    network_bytes(&NetSpec {
        ir_version: 8,
        opsets: vec![(String::new(), 17)],
        graph,
        functions: 0,
    })
}

/// The bytes of a network file.
pub fn network_bytes(spec: &NetSpec) -> Vec<u8> {
    let mut model = Vec::new();
    varint_field(&mut model, 1, spec.ir_version.cast_unsigned());
    bytes_field(&mut model, 2, b"volsa tests");
    bytes_field(&mut model, 7, &graph(&spec.graph));
    for (domain, version) in &spec.opsets {
        let mut opset = Vec::new();
        bytes_field(&mut opset, 1, domain.as_bytes());
        varint_field(&mut opset, 2, version.cast_unsigned());
        bytes_field(&mut model, 8, &opset);
    }
    for _ in 0..spec.functions {
        let mut function = Vec::new();
        bytes_field(&mut function, 1, b"own");
        bytes_field(&mut model, 25, &function);
    }
    model
}

/// The bytes of a graph.
pub fn graph(spec: &GraphSpec) -> Vec<u8> {
    let mut graph = Vec::new();
    for node in &spec.nodes {
        bytes_field(&mut graph, 1, &node_bytes(node));
    }
    bytes_field(&mut graph, 2, b"graph");
    for tensor in &spec.initializers {
        bytes_field(&mut graph, 5, &tensor_bytes(tensor));
    }
    for port in &spec.inputs {
        bytes_field(&mut graph, 11, &port_bytes(port));
    }
    for port in &spec.outputs {
        bytes_field(&mut graph, 12, &port_bytes(port));
    }
    graph
}

/// The bytes of a node.
fn node_bytes(spec: &NodeSpec) -> Vec<u8> {
    let mut node = Vec::new();
    for input in &spec.inputs {
        bytes_field(&mut node, 1, input.as_bytes());
    }
    for output in &spec.outputs {
        bytes_field(&mut node, 2, output.as_bytes());
    }
    bytes_field(&mut node, 4, spec.op.as_bytes());
    for attribute in &spec.attributes {
        bytes_field(&mut node, 5, &attribute_bytes(attribute));
    }
    if !spec.domain.is_empty() {
        bytes_field(&mut node, 7, spec.domain.as_bytes());
    }
    node
}

/// The bytes of an attribute.
fn attribute_bytes(spec: &AttrSpec) -> Vec<u8> {
    let mut attribute = Vec::new();
    match spec {
        AttrSpec::Ints(name, ints) => {
            bytes_field(&mut attribute, 1, name.as_bytes());
            for int in ints {
                varint_field(&mut attribute, 8, int.cast_unsigned());
            }
            varint_field(&mut attribute, 20, 7);
        }
        AttrSpec::Graph(name, graph_spec) => {
            bytes_field(&mut attribute, 1, name.as_bytes());
            bytes_field(&mut attribute, 6, &graph(graph_spec));
            varint_field(&mut attribute, 20, 5);
        }
        AttrSpec::Tensor(name, tensor) => {
            bytes_field(&mut attribute, 1, name.as_bytes());
            bytes_field(&mut attribute, 5, &tensor_bytes(tensor));
            varint_field(&mut attribute, 20, 4);
        }
    }
    attribute
}

/// The bytes of a tensor of floats.
fn tensor_bytes(spec: &InitSpec) -> Vec<u8> {
    let mut tensor = Vec::new();
    for dim in &spec.dims {
        varint_field(&mut tensor, 1, dim.cast_unsigned());
    }
    varint_field(&mut tensor, 2, 1);
    bytes_field(&mut tensor, 8, spec.name.as_bytes());
    if let Some((location, offset, length)) = &spec.external {
        for (key, value) in [
            ("location", location),
            ("offset", offset),
            ("length", length),
        ] {
            let mut entry = Vec::new();
            bytes_field(&mut entry, 1, key.as_bytes());
            bytes_field(&mut entry, 2, value.as_bytes());
            bytes_field(&mut tensor, 13, &entry);
        }
        varint_field(&mut tensor, 14, 1);
    } else {
        let raw: Vec<u8> = spec
            .floats
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        bytes_field(&mut tensor, 9, &raw);
    }
    tensor
}

/// The bytes of a port.
fn port_bytes(spec: &PortSpec) -> Vec<u8> {
    let mut shape = Vec::new();
    for dim in &spec.dims {
        let mut dimension = Vec::new();
        match dim {
            DimSpec::Fixed(size) => varint_field(&mut dimension, 1, size.cast_unsigned()),
            DimSpec::Named(name) => bytes_field(&mut dimension, 2, name.as_bytes()),
        }
        bytes_field(&mut shape, 1, &dimension);
    }
    let mut tensor = Vec::new();
    varint_field(&mut tensor, 1, u64::from(spec.element.cast_unsigned()));
    bytes_field(&mut tensor, 2, &shape);
    let mut kind = Vec::new();
    bytes_field(&mut kind, 1, &tensor);
    let mut port = Vec::new();
    bytes_field(&mut port, 1, spec.name.as_bytes());
    bytes_field(&mut port, 2, &kind);
    port
}

/// A field of the varint kind.
fn varint_field(out: &mut Vec<u8>, field: u32, value: u64) {
    varint(out, u64::from(field) << 3);
    varint(out, value);
}

/// A field of the length-delimited kind.
fn bytes_field(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    varint(out, (u64::from(field) << 3) | 2);
    varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// A number written seven bits at a time, lowest first.
fn varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}
