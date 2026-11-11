//! A block's answer, finished to labels as its description says.

use volsa_models::{Numbers, Tensor};

use super::{FinishError, labels_of};
use crate::{Gives, LabelOutput, Layout, Name, Side};

/// An output that gives `gives`, naming the labels 1 and 2.
fn output(gives: Gives) -> LabelOutput {
    let name = |id: u16| Name {
        id,
        name: format!("part {id}"),
        side: Side::Midline,
        group: String::new(),
        colour: None,
    };
    LabelOutput {
        gives,
        from: None,
        keep_largest_piece: false,
        names: vec![name(1), name(2)],
    }
}

/// Scores of three channels, for the labels 0, 1 and 2.
fn scores() -> LabelOutput {
    output(Gives::Scores {
        channels: vec![0, 1, 2],
    })
}

/// A tensor of floats.
fn floats(shape: Vec<usize>, numbers: Vec<f32>) -> Tensor {
    Tensor {
        shape,
        numbers: Numbers::F32(numbers),
    }
}

#[test]
fn scores_take_the_largest_and_the_first_on_ties() {
    // Three voxels; the third is a tie between the first two channels, and the first wins.
    let answer = floats(
        vec![1, 3, 1, 1, 3],
        vec![0.9, 0.2, 0.5, 0.1, 0.7, 0.5, 0.0, 0.1, 0.0],
    );
    let labels = labels_of(answer, Layout::ChannelsFirst, [1, 1, 3], &scores()).expect("labels");
    assert_eq!(labels, [0, 1, 0]);
}

#[test]
fn channels_last_scores_are_read_so() {
    let answer = floats(
        vec![1, 1, 1, 3, 3],
        vec![0.9, 0.1, 0.0, 0.2, 0.7, 0.1, 0.0, 0.1, 0.8],
    );
    let labels = labels_of(answer, Layout::ChannelsLast, [1, 1, 3], &scores()).expect("labels");
    assert_eq!(labels, [0, 1, 2]);
}

#[test]
fn a_chance_above_the_threshold_is_the_label() {
    let chance = output(Gives::Chance {
        label: 2,
        threshold: 0.5,
    });
    let answer = floats(vec![1, 1, 1, 1, 3], vec![0.2, 0.5, 0.9]);
    let labels = labels_of(answer, Layout::ChannelsFirst, [1, 1, 3], &chance).expect("labels");
    assert_eq!(labels, [0, 0, 2]);
}

#[test]
fn sixty_four_bit_labels_are_taken_and_an_unnamed_one_is_refused() {
    let given = output(Gives::Labels);
    let answer = |numbers: Numbers| Tensor {
        shape: vec![1, 1, 1, 3],
        numbers,
    };
    let labels = labels_of(
        answer(Numbers::I64(vec![0, 2, 1])),
        Layout::ChannelsFirst,
        [1, 1, 3],
        &given,
    );
    assert_eq!(labels.expect("labels"), [0, 2, 1]);
    let labels = labels_of(
        answer(Numbers::I32(vec![1, 0, 0])),
        Layout::ChannelsFirst,
        [1, 1, 3],
        &given,
    );
    assert_eq!(labels.expect("labels"), [1, 0, 0]);
    let labels = labels_of(
        answer(Numbers::U8(vec![0, 0, 2])),
        Layout::ChannelsFirst,
        [1, 1, 3],
        &given,
    );
    assert_eq!(labels.expect("labels"), [0, 0, 2]);
    for unnamed in [vec![0, 7, 0], vec![-1, 0, 0], vec![70_000, 0, 0]] {
        let refused = labels_of(
            answer(Numbers::I64(unnamed)),
            Layout::ChannelsFirst,
            [1, 1, 3],
            &given,
        );
        let refused = refused.expect_err("refused");
        assert_eq!(refused, FinishError::Unnamed);
        assert_eq!(
            refused.to_string(),
            "The plugin gave labels its description does not name."
        );
    }
}

#[test]
fn a_nan_or_a_wrong_shape_or_kind_stops_with_a_sentence() {
    let nan = floats(vec![1, 3, 1, 1, 1], vec![0.1, f32::NAN, 0.2]);
    let refused = labels_of(nan, Layout::ChannelsFirst, [1, 1, 1], &scores()).expect_err("nan");
    assert_eq!(refused, FinishError::NotFinite);
    assert_eq!(
        refused.to_string(),
        "The plugin gave an answer with a number that is not finite."
    );
    let short = floats(vec![1, 3, 1, 1, 2], vec![0.0; 6]);
    let refused = labels_of(short, Layout::ChannelsFirst, [1, 1, 3], &scores()).expect_err("size");
    assert_eq!(refused, FinishError::WrongAnswer);
    assert_eq!(
        refused.to_string(),
        "The plugin gave an answer of the wrong size or kind."
    );
    let lying = floats(vec![1, 3, 1, 1, 3], vec![0.0; 8]);
    let refused = labels_of(lying, Layout::ChannelsFirst, [1, 1, 3], &scores()).expect_err("count");
    assert_eq!(refused, FinishError::WrongAnswer);
    let bytes = Tensor {
        shape: vec![1, 3, 1, 1, 1],
        numbers: Numbers::U8(vec![0, 1, 0]),
    };
    let refused = labels_of(bytes, Layout::ChannelsFirst, [1, 1, 1], &scores()).expect_err("kind");
    assert_eq!(refused, FinishError::WrongAnswer);
}
