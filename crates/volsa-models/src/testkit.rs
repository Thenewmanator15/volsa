//! Builders of made-up model folders and a stand-in network, for tests in this crate and the
//! crates above it.
//!
//! No real network is read, written or run here. A folder made by [`model_folder`] holds six
//! small files of made-up bytes and a `model.json` that lists them with their true sizes and
//! sums. A [`Canned`] is a network that answers whatever a test tells it to.
//!
//! This is test support code: a mistake in a builder should stop the test at once, so unlike
//! the rest of the crate it is allowed to panic.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::model::{KIND_SYNTHSEG, MODEL_FORMAT, ModelSide, Role, Structure, hex, is_plain_name};
use crate::network::{Network, RunError};
use crate::tensor::Tensor;

/// Everything `model.json` holds that Volsa reads. The default describes a folder that reads
/// without complaint; change what a test is about.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderOptions {
    /// The list's `format`.
    pub format: u32,
    /// The list's `kind`.
    pub kind: String,
    /// The `name` the list gives for each network. A file is written only for a name that is
    /// a plain file name, so a test of a bad name cannot write outside its folder.
    pub names: HashMap<Role, String>,
    /// The `structures`, with their classes.
    pub structures: Vec<Structure>,
    /// The `flip`.
    pub flip: Vec<usize>,
    /// The `stage_classes`.
    pub stage_classes: usize,
    /// The `areas`; the classes are not written, areas having none.
    pub areas: Vec<Structure>,
    /// The `cortex` labels.
    pub cortex: [u32; 2],
    /// The `score_class`.
    pub score_class: Vec<usize>,
    /// The `score_groups`.
    pub score_groups: Vec<String>,
    /// The `box`.
    pub box_side: usize,
}

impl Default for FolderOptions {
    fn default() -> Self {
        Self {
            format: MODEL_FORMAT,
            kind: KIND_SYNTHSEG.to_owned(),
            names: Role::ALL
                .into_iter()
                .map(|role| (role, format!("{}.onnx", key(role))))
                .collect(),
            structures: (0..33_u32)
                .map(|place| Structure {
                    label: if place == 0 { 0 } else { place + 1 },
                    name: format!("structure {place}"),
                    side: side(place),
                    class: place % 5,
                })
                .collect(),
            // Odd places swap with the even place above them; place 0 stays.
            flip: (0..33)
                .map(|place| match place {
                    0 => 0,
                    odd if odd % 2 == 1 => odd + 1,
                    even => even - 1,
                })
                .collect(),
            stage_classes: 5,
            areas: (0..69_u32)
                .map(|place| Structure {
                    label: if place == 0 { 0 } else { 1000 + place },
                    name: format!("area {place}"),
                    side: side(place),
                    class: 0,
                })
                .collect(),
            cortex: [3, 20],
            score_class: (0..33).map(|place| place % 9).collect(),
            score_groups: (1..=8).map(|group| format!("group {group}")).collect(),
            box_side: 224,
        }
    }
}

/// Midline for the background, then left and right in turn.
fn side(place: u32) -> ModelSide {
    match place {
        0 => ModelSide::Midline,
        odd if odd % 2 == 1 => ModelSide::Left,
        _ => ModelSide::Right,
    }
}

/// The key the list uses for a network.
fn key(role: Role) -> &'static str {
    match role {
        Role::Standard => "standard",
        Role::Robust1 => "robust_1",
        Role::Robust2 => "robust_2",
        Role::Robust3 => "robust_3",
        Role::Cortex => "cortex",
        Role::Scores => "scores",
    }
}

/// How the list writes a side.
fn side_word(side: ModelSide) -> &'static str {
    match side {
        ModelSide::Left => "left",
        ModelSide::Right => "right",
        ModelSide::Midline => "midline",
    }
}

/// A structure as the list writes it; an area has no class.
fn part(part: &Structure, with_class: bool) -> Value {
    let mut entry = json!({
        "label": part.label,
        "name": part.name,
        "side": side_word(part.side),
    });
    if let (true, Some(object)) = (with_class, entry.as_object_mut()) {
        object.insert("class".to_owned(), json!(part.class));
    }
    entry
}

/// Makes a folder holding `model.json` and a made-up file for each network, listed with its
/// true size and sum. It is deleted when the returned value is dropped.
pub fn model_folder(options: &FolderOptions) -> TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let mut files = Map::new();
    for role in Role::ALL {
        let name = options.names.get(&role).cloned().unwrap_or_default();
        let bytes = format!("made-up network for {role}. ")
            .repeat(4)
            .into_bytes();
        if is_plain_name(&name) {
            fs::write(folder.path().join(&name), &bytes).expect("a made-up network");
        }
        files.insert(
            key(role).to_owned(),
            json!({
                "name": name,
                "bytes": bytes.len(),
                "sha256": hex(&Sha256::digest(&bytes)),
            }),
        );
    }
    let list = json!({
        "format": options.format,
        "kind": options.kind,
        "files": files,
        "structures": options.structures.iter().map(|s| part(s, true)).collect::<Vec<_>>(),
        "flip": options.flip,
        "stage_classes": options.stage_classes,
        "areas": options.areas.iter().map(|a| part(a, false)).collect::<Vec<_>>(),
        "cortex": options.cortex,
        "score_class": options.score_class,
        "score_groups": options.score_groups,
        "box": options.box_side,
        "made": { "converter": 1, "opset": 17 },
        "checked": {},
    });
    let text = serde_json::to_vec_pretty(&list).expect("a list");
    fs::write(folder.path().join("model.json"), text).expect("a list file");
    folder
}

/// What answers for a [`Canned`]: it is shown the inputs of a run and gives the run's outcome.
type Answer = Box<dyn FnMut(&[Tensor]) -> Result<Vec<Tensor>, RunError> + Send>;

/// A stand-in for a network: it answers from a closure and remembers what it was given, so
/// that what is built on a [`Network`] can be tested with no runtime and no network file.
pub struct Canned {
    answer: Answer,
    given: Given,
}

/// What a [`Canned`] has been given: the inputs of every run so far. Every copy sees the
/// same runs, so one taken before the network is handed on can be asked afterwards.
#[derive(Debug, Clone, Default)]
pub struct Given(Arc<Mutex<Vec<Vec<Tensor>>>>);

impl Given {
    /// The inputs of each run so far, the first run first.
    pub fn runs(&self) -> Vec<Vec<Tensor>> {
        self.0.lock().expect("no run panicked").clone()
    }
}

impl Canned {
    /// A network whose every run is answered by `answer`, which is shown the inputs.
    pub fn new(
        answer: impl FnMut(&[Tensor]) -> Result<Vec<Tensor>, RunError> + Send + 'static,
    ) -> Self {
        Self {
            answer: Box::new(answer),
            given: Given::default(),
        }
    }

    /// What this network has been given, and will be given from now on.
    pub fn given(&self) -> Given {
        self.given.clone()
    }
}

impl Network for Canned {
    fn run(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError> {
        let answer = (self.answer)(&inputs);
        self.given.0.lock().expect("no run panicked").push(inputs);
        answer
    }
}
