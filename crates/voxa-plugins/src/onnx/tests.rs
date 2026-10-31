//! A network file is read for its ports, and refused for anything outside standard ONNX.

use std::fs;

use voxa_models::SUPPORTED;

use super::{Dim, Element, NetworkError, NetworkFacts, read_network};
use crate::Listed;
use crate::testkit::onnx::{
    AttrSpec, DimSpec, GraphSpec, InitSpec, NetSpec, NodeSpec, conv_network, floats, image_input,
    network_bytes,
};
use crate::testkit::sum;

/// What reading these bytes as a network file says, with these other files listed.
pub(super) fn read_bytes(
    bytes: &[u8],
    others: &[(&str, &[u8])],
) -> Result<NetworkFacts, NetworkError> {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("net.onnx");
    fs::write(&path, bytes).expect("written");
    let mut listed = vec![Listed {
        name: "net.onnx".to_owned(),
        bytes: bytes.len() as u64,
        sha256: sum(bytes),
    }];
    for (name, other) in others {
        fs::write(folder.path().join(name), other).expect("written");
        listed.push(Listed {
            name: (*name).to_owned(),
            bytes: other.len() as u64,
            sha256: sum(other),
        });
    }
    read_network(&path, &listed, SUPPORTED)
}

/// A network of one node of `op` in `domain`, with an image in and out.
pub(crate) fn one_node(op: &str, domain: &str, opsets: Vec<(String, i64)>) -> NetSpec {
    let node = NodeSpec {
        op: op.to_owned(),
        domain: domain.to_owned(),
        inputs: vec!["image".to_owned()],
        outputs: vec!["out".to_owned()],
        ..NodeSpec::default()
    };
    NetSpec {
        ir_version: 8,
        opsets,
        graph: GraphSpec {
            nodes: vec![node],
            inputs: vec![image_input()],
            outputs: vec![floats("out", vec![DimSpec::Fixed(1)])],
            initializers: Vec::new(),
        },
        functions: 0,
    }
}

/// The standard operators, version 17.
pub(super) fn standard() -> Vec<(String, i64)> {
    vec![(String::new(), 17)]
}

#[test]
fn a_conv_network_gives_its_ports() {
    let bytes = conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5]);
    let facts = read_bytes(&bytes, &[]).expect("read");
    assert_eq!(facts.inputs.len(), 1, "the weights are not inputs");
    let input = &facts.inputs[0];
    assert_eq!(input.name, "image");
    assert_eq!(input.element, Element::F32);
    let named = |name: &str| Dim::Named(name.to_owned());
    let dims = vec![
        Dim::Fixed(1),
        Dim::Fixed(1),
        named("x"),
        named("y"),
        named("z"),
    ];
    assert_eq!(input.dims, dims);
    assert_eq!(facts.outputs.len(), 1);
    assert_eq!(facts.outputs[0].name, "scores");
    assert_eq!(facts.outputs[0].dims[1], Dim::Fixed(2));
}

#[test]
fn a_node_in_the_ai_onnx_domain_by_name_is_standard() {
    let spec = one_node("Relu", "ai.onnx", vec![("ai.onnx".to_owned(), 17)]);
    assert!(read_bytes(&network_bytes(&spec), &[]).is_ok());
}

#[test]
fn a_node_in_a_custom_domain_is_refused() {
    let spec = one_node("Mystery", "com.example", standard());
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert!(matches!(refused, NetworkError::Domain(_)), "{refused:?}");
    let words = "The network uses operators outside standard ONNX, which Voxa does not run.";
    assert_eq!(refused.to_string(), words);
}

#[test]
fn a_custom_node_inside_an_if_branch_is_refused() {
    let inner = one_node("Mystery", "com.example", standard()).graph;
    let mut spec = one_node("If", "", standard());
    spec.graph.nodes[0].attributes =
        vec![AttrSpec::Graph("then_branch".to_owned(), Box::new(inner))];
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert!(matches!(refused, NetworkError::Domain(_)), "{refused:?}");
}

#[test]
fn a_com_microsoft_or_ml_opset_is_refused() {
    for domain in ["com.microsoft", "ai.onnx.ml"] {
        let mut opsets = standard();
        opsets.push((domain.to_owned(), 1));
        let spec = one_node("Relu", "", opsets);
        let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
        assert!(
            matches!(refused, NetworkError::Domain(_)),
            "{domain}: {refused:?}"
        );
    }
}

#[test]
fn local_functions_are_refused() {
    let mut spec = one_node("Relu", "", standard());
    spec.functions = 1;
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::Functions);
}

#[test]
fn an_opset_or_ir_version_newer_than_supported_is_refused() {
    let spec = one_node("Relu", "", vec![(String::new(), SUPPORTED.opset + 1)]);
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::TooNew);
    let mut spec = one_node("Relu", "", standard());
    spec.ir_version = SUPPORTED.ir_version + 1;
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::TooNew);
}

/// A network whose one tensor of fixed numbers lies in another file, at this place.
fn with_external(location: &str, offset: &str, length: &str) -> Vec<u8> {
    let mut spec = one_node("Relu", "", standard());
    spec.graph.initializers = vec![InitSpec {
        name: "w".to_owned(),
        dims: vec![4],
        floats: Vec::new(),
        external: Some((location.to_owned(), offset.to_owned(), length.to_owned())),
    }];
    network_bytes(&spec)
}

#[test]
fn external_data_in_a_listed_file_is_taken() {
    let weights = [0_u8; 32];
    let bytes = with_external("weights.bin", "16", "16");
    assert!(read_bytes(&bytes, &[("weights.bin", &weights)]).is_ok());
}

#[test]
fn external_data_named_with_dots_or_unlisted_or_past_the_end_is_refused() {
    let weights = [0_u8; 32];
    let cases = [
        with_external("../weights.bin", "0", "16"),
        with_external("other.bin", "0", "16"),
        with_external("weights.bin", "16", "17"),
        with_external("weights.bin", "33", "0"),
        with_external("weights.bin", "x", "16"),
    ];
    for bytes in cases {
        let refused = read_bytes(&bytes, &[("weights.bin", &weights)]).expect_err("refused");
        assert_eq!(refused, NetworkError::Outside);
    }
}

#[test]
fn a_tensor_attribute_with_external_data_is_checked_too() {
    let mut spec = one_node("Constant", "", standard());
    let tensor = InitSpec {
        name: "value".to_owned(),
        dims: vec![4],
        floats: Vec::new(),
        external: Some(("elsewhere.bin".to_owned(), "0".to_owned(), "16".to_owned())),
    };
    spec.graph.nodes[0].attributes = vec![AttrSpec::Tensor("value".to_owned(), tensor)];
    let refused = read_bytes(&network_bytes(&spec), &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::Outside);
}

#[test]
fn an_empty_or_graphless_file_is_not_a_network() {
    assert_eq!(
        read_bytes(&[], &[]).expect_err("refused"),
        NetworkError::NotANetwork
    );
    let mut no_graph = Vec::new();
    no_graph.extend_from_slice(&[0x08, 0x08]);
    let refused = read_bytes(&no_graph, &[]).expect_err("refused");
    assert_eq!(refused, NetworkError::NotANetwork);
}
