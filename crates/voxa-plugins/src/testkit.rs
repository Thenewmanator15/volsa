//! Builders of made-up plugin folders, for this crate's tests and for the crates above it.
//!
//! This is test support code: a mistake in a builder should stop the test at once, so unlike
//! the rest of the crate it is allowed to panic.
#![allow(
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::indexing_slicing
)]

use std::fs;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

pub mod helper;
pub mod onnx;

/// The SHA-256 of some bytes, in lower-case hex.
pub fn sum(bytes: &[u8]) -> String {
    crate::folder::hex(&Sha256::digest(bytes))
}

/// A folder holding `files` and a `plugin.json` of `description`. When the description has
/// no `files`, the list is made from the files given, with their true sizes and sums. A file
/// is written only when its name is a plain file name, so a test of a bad name cannot write
/// outside its folder.
pub fn plugin_folder(description: &Value, files: &[(&str, &[u8])]) -> TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let mut description = description.clone();
    if description.get("files").is_none() {
        let listed: Vec<Value> = files
            .iter()
            .map(
                |(name, bytes)| json!({ "name": name, "bytes": bytes.len(), "sha256": sum(bytes) }),
            )
            .collect();
        description["files"] = Value::Array(listed);
    }
    for (name, bytes) in files {
        if voxa_models::is_plain_name(name) {
            fs::write(folder.path().join(name), bytes).expect("a file written");
        }
    }
    let text = serde_json::to_vec_pretty(&description).expect("JSON");
    fs::write(folder.path().join("plugin.json"), text).expect("plugin.json written");
    folder
}
