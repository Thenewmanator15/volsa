//! A hostile file cannot make the reader recurse, allocate or loop without end, and nothing it
//! holds makes it panic.

use proptest::prelude::*;

use super::NetworkError;
use super::tests::{one_node, read_bytes, standard};
use crate::testkit::onnx::{AttrSpec, GraphSpec, conv_network, image_input, network_bytes};

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
