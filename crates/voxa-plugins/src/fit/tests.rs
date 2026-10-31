//! A network is wired to its description only when its ports are what the description says.

use serde_json::json;

use super::{Wiring, fit};
use crate::describe::tests::{example, read};
use crate::{Description, Dim, Element, NetworkFacts, PluginError, Port};

/// A port of these dimensions: a number is fixed, a name is free.
fn port(name: &str, element: Element, dims: &[&str]) -> Port {
    let dim = |text: &&str| match text.parse::<u64>() {
        Ok(size) => Dim::Fixed(size),
        Err(_) if *text == "?" => Dim::Unknown,
        Err(_) => Dim::Named((*text).to_owned()),
    };
    Port {
        name: name.to_owned(),
        element,
        dims: dims.iter().map(dim).collect(),
    }
}

/// A network of one image input and these outputs.
fn network(input: Port, outputs: Vec<Port>) -> NetworkFacts {
    NetworkFacts {
        inputs: vec![input],
        outputs,
    }
}

/// The example's description.
fn described() -> Description {
    read(&example()).expect("the example reads")
}

/// The sentence a refusal is said with.
fn said(refused: Result<Wiring, PluginError>) -> String {
    refused.expect_err("refused").to_string()
}

#[test]
fn a_whole_box_network_with_named_sides_is_wired_with_them() {
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let output = port("scores", Element::F32, &["1", "2", "x", "y", "z"]);
    let wiring = fit(&described(), &network(input, vec![output])).expect("wired");
    assert_eq!(wiring.inputs, ["image"]);
    assert_eq!(wiring.output, "scores");
    let free = [
        ("x".to_owned(), 256),
        ("y".to_owned(), 256),
        ("z".to_owned(), 256),
    ];
    assert_eq!(wiring.free, free);
}

#[test]
fn a_free_batch_and_a_side_named_twice_are_set_once() {
    let input = port(
        "image",
        Element::F32,
        &["batch", "1", "side", "side", "side"],
    );
    let output = port(
        "scores",
        Element::F32,
        &["batch", "2", "side", "side", "side"],
    );
    let wiring = fit(&described(), &network(input, vec![output])).expect("wired");
    assert_eq!(
        wiring.free,
        [("batch".to_owned(), 1), ("side".to_owned(), 256)]
    );
}

#[test]
fn a_name_given_two_sizes_is_refused() {
    let mut value = example();
    value["input"]["fit"]["whole"] = json!([256, 256, 128]);
    let description = read(&value).expect("reads");
    let input = port("image", Element::F32, &["1", "1", "side", "side", "side"]);
    let output = port("scores", Element::F32, &["1", "2", "side", "side", "side"]);
    let words = "The network's input is not the size its description gives.";
    assert_eq!(
        said(fit(&description, &network(input, vec![output]))),
        words
    );
}

#[test]
fn a_channels_last_network_is_read_so() {
    let mut value = example();
    value["input"]["layout"] = json!("channels-last");
    let description = read(&value).expect("reads");
    let input = port("image", Element::F32, &["1", "256", "256", "256", "1"]);
    let output = port("scores", Element::F32, &["1", "256", "256", "256", "2"]);
    assert!(fit(&description, &network(input.clone(), vec![output])).is_ok());
    let first = port("scores", Element::F32, &["1", "2", "256", "256", "256"]);
    let words = "The network's answer is not the size its description gives.";
    assert_eq!(said(fit(&description, &network(input, vec![first]))), words);
}

#[test]
fn a_fixed_side_that_differs_is_refused() {
    let input = port("image", Element::F32, &["1", "1", "128", "128", "128"]);
    let output = port("scores", Element::F32, &["1", "2", "128", "128", "128"]);
    let words = "The network's input is not the size its description gives.";
    assert_eq!(
        said(fit(&described(), &network(input, vec![output]))),
        words
    );
}

#[test]
fn an_input_of_the_wrong_kind_or_shape_is_refused() {
    let output = port("scores", Element::F32, &["1", "2", "x", "y", "z"]);
    let bytes = port("image", Element::U8, &["1", "1", "x", "y", "z"]);
    let words = "The network's input is not of 32-bit numbers.";
    assert_eq!(
        said(fit(&described(), &network(bytes, vec![output.clone()]))),
        words
    );
    let four = port("image", Element::F32, &["1", "x", "y", "z"]);
    let words = "The network's input does not have the five sides its description says.";
    assert_eq!(
        said(fit(&described(), &network(four, vec![output.clone()]))),
        words
    );
    let unknown = port("image", Element::F32, &["1", "1", "?", "y", "z"]);
    let words = "The network's input has a side Voxa cannot set.";
    assert_eq!(
        said(fit(&described(), &network(unknown, vec![output]))),
        words
    );
}

#[test]
fn a_network_of_two_inputs_is_refused_for_a_label_map() {
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let output = port("scores", Element::F32, &["1", "2", "x", "y", "z"]);
    let mut facts = network(input.clone(), vec![output]);
    facts.inputs.push(Port {
        name: "second".to_owned(),
        ..input
    });
    let words = "The network takes a number of inputs other than its description says.";
    assert_eq!(said(fit(&described(), &facts)), words);
}

#[test]
fn several_outputs_need_from_and_from_must_exist() {
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let scores = port("scores", Element::F32, &["1", "2", "x", "y", "z"]);
    let extra = port("deep", Element::F32, &["1", "2", "a", "b", "c"]);
    let facts = network(input, vec![extra, scores]);
    let words = "The network gives more than one answer and the description does not say which \
                 to read.";
    assert_eq!(said(fit(&described(), &facts)), words);
    let mut value = example();
    value["output"]["from"] = json!("scores");
    let wiring = fit(&read(&value).expect("reads"), &facts).expect("wired");
    assert_eq!(wiring.output, "scores");
    value["output"]["from"] = json!("missing");
    let words = "The network has no answer of the name its description gives.";
    assert_eq!(said(fit(&read(&value).expect("reads"), &facts)), words);
}

#[test]
fn a_scores_output_with_the_wrong_channel_count_is_refused() {
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let output = port("scores", Element::F32, &["1", "3", "x", "y", "z"]);
    let words = "The network's answer is not the size its description gives.";
    assert_eq!(
        said(fit(&described(), &network(input, vec![output]))),
        words
    );
}

#[test]
fn a_labels_output_of_sixty_four_bit_integers_is_taken() {
    let mut value = example();
    value["output"]["gives"] = json!("labels");
    value["output"]
        .as_object_mut()
        .expect("object")
        .remove("channels");
    let description = read(&value).expect("reads");
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let output = port("labels", Element::I64, &["1", "x", "y", "z"]);
    assert!(fit(&description, &network(input.clone(), vec![output])).is_ok());
    let floats = port("labels", Element::F32, &["1", "x", "y", "z"]);
    let words = "The network's answer is not of the kind its description says.";
    assert_eq!(
        said(fit(&description, &network(input, vec![floats]))),
        words
    );
}

#[test]
fn a_chance_output_has_one_channel() {
    let mut value = example();
    value["output"]["gives"] = json!("chance");
    value["output"]["label"] = json!(1);
    value["output"]["threshold"] = json!(0.5);
    value["output"]
        .as_object_mut()
        .expect("object")
        .remove("channels");
    let description = read(&value).expect("reads");
    let input = port("image", Element::F32, &["1", "1", "x", "y", "z"]);
    let one = port("chance", Element::F32, &["1", "1", "x", "y", "z"]);
    assert!(fit(&description, &network(input.clone(), vec![one])).is_ok());
    let two = port("chance", Element::F32, &["1", "2", "x", "y", "z"]);
    let words = "The network's answer is not the size its description gives.";
    assert_eq!(said(fit(&description, &network(input, vec![two]))), words);
}
