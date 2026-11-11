//! A hostile file cannot make the reader recurse, allocate or loop without end, and nothing it
//! holds makes it panic.

use proptest::prelude::*;

use super::NetworkError;
use super::tests::{one_node, read_bytes, standard};
use crate::testkit::onnx::{
    AttrSpec, DimSpec, GraphSpec, conv_network, floats, image_input, network_bytes,
};

/// A network whose graph holds subgraphs `depth` deep, each in an `If`.
fn nested(depth: usize) -> Vec<u8> {
    let mut graph: GraphSpec = one_node("Relu", "", standard()).graph;
    for _ in 0..depth {
        let mut outer = one_node("If", "", standard()).graph;
        outer.nodes[0].attributes =
            vec![AttrSpec::Graph("then_branch".to_owned(), Box::new(graph))];
        graph = outer;
    }
    let mut spec = one_node("Relu", "", standard());
    spec.graph = graph;
    network_bytes(&spec)
}

#[test]
fn subgraphs_sixteen_deep_are_read_and_seventeen_are_refused() {
    assert!(read_bytes(&nested(16), &[]).is_ok());
    let refused = read_bytes(&nested(17), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::TooDeep);
}

#[test]
fn a_length_past_the_end_is_not_a_network() {
    // A graph field that says it is a gibibyte long, in a file of a few bytes.
    let bytes = [0x3a, 0x80, 0x80, 0x80, 0x80, 0x04, 0x00];
    let refused = read_bytes(&bytes, &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::NotANetwork);
}

#[test]
fn more_nodes_than_the_reader_counts_are_refused() {
    let mut spec = one_node("Relu", "", standard());
    let node = spec.graph.nodes[0].clone();
    spec.graph.nodes = vec![node; 100_001];
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::TooDeep);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn a_file_cut_short_anywhere_is_refused_without_a_panic(cut in 0_usize..400) {
        let bytes = conv_network(&image_input(), &[1.0, 2.0], &[0.0, 0.0]);
        let cut = cut.min(bytes.len().saturating_sub(1));
        prop_assert!(read_bytes(&bytes[..cut], &[]).is_err());
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = read_bytes(&bytes, &[]);
    }
}

/// Protobuf's wire format, by hand, for what the test kit does not write.
fn varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let low = u8::try_from(value & 0x7f).unwrap_or(0);
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

fn length_field(out: &mut Vec<u8>, number: u64, bytes: &[u8]) {
    varint(out, number << 3 | 2);
    varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// A model whose one tensor names `elsewhere.bin` for its numbers, with its location field
/// written as `location` when there is one.
fn kept_elsewhere(location: Option<u64>) -> Vec<u8> {
    let mut entry = Vec::new();
    length_field(&mut entry, 1, b"location");
    length_field(&mut entry, 2, b"elsewhere.bin");
    let mut tensor = Vec::new();
    length_field(&mut tensor, 8, b"w");
    length_field(&mut tensor, 13, &entry);
    if let Some(location) = location {
        varint(&mut tensor, 14 << 3);
        varint(&mut tensor, location);
    }
    let mut graph = Vec::new();
    length_field(&mut graph, 5, &tensor);
    let mut opset = Vec::new();
    length_field(&mut opset, 1, b"");
    varint(&mut opset, 2 << 3);
    varint(&mut opset, 17);
    let mut model = Vec::new();
    varint(&mut model, 1 << 3);
    varint(&mut model, 8);
    length_field(&mut model, 7, &graph);
    length_field(&mut model, 8, &opset);
    model
}

#[test]
fn a_tensor_kept_elsewhere_is_checked_however_its_location_is_written() {
    // ONNX Runtime reads the location as 32 bits: 2^32 + 1 is "external" to it.
    for location in [Some(1), Some((1 << 32) + 1), None] {
        let refused = read_bytes(&kept_elsewhere(location), &[]).expect_err("refused");
        assert_eq!(refused, NetworkError::Outside, "{location:?}");
    }
}

#[test]
fn more_ports_dimensions_or_operator_sets_than_the_reader_keeps_are_refused() {
    let mut ports = one_node("Relu", "", standard());
    let port = ports.graph.inputs[0].clone();
    ports.graph.inputs = vec![port; 1_025];
    let mut dims = one_node("Relu", "", standard());
    dims.graph.inputs[0] = floats("x", vec![DimSpec::Fixed(1); 17]);
    let mut opsets = one_node("Relu", "", standard());
    opsets.opsets = vec![(String::new(), 17); 65];
    for (what, spec) in [
        ("ports", ports),
        ("dimensions", dims),
        ("operator sets", opsets),
    ] {
        let refused = read_bytes(&network_bytes(&spec), &[]).expect_err(what);
        assert_eq!(refused, NetworkError::TooDeep, "{what}");
    }
}
