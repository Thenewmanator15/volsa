//! Who made a label set, as a table of names keeps it in a `volsa` object: the program, the
//! model, the place it ran, the moment, and sentences for a reader. Nothing here holds a
//! path or the name of a file.
//!
//! Writing is strict and reading is forgiving. What is written is cut to length; what is
//! read is cut the same way, and a part that is missing or of the wrong kind is taken as
//! absent. A `volsa` object never makes a table unreadable.

use serde::Serialize;
use serde_json::{Map, Value};

/// The longest `by`, `model`, `device` and `when` are kept, in characters.
pub const MAX_MADE_LINE: usize = 80;
/// The most notes kept.
pub const MAX_NOTES: usize = 8;
/// The longest a note is kept, in characters.
pub const MAX_NOTE: usize = 300;
/// How many hexadecimal digits a SHA-256 sum has.
const SUM_DIGITS: usize = 64;

#[cfg(test)]
mod added_tests;

/// Who made a label set, and how. Every part is plain text; a part that is not known is
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Made {
    /// The program and its version, for example the name of a program, a space and a number.
    pub by: String,
    /// The model's kind and what ran: which of its networks made the labels.
    pub model: String,
    /// The SHA-256 of the model folder's list file, as 64 lower-case hexadecimal digits;
    /// empty when there is no such sum.
    pub model_sum: String,
    /// Where it ran, in plain words: a graphics card by name, or the processor.
    pub device: String,
    /// The moment it was made, as text a person can read.
    pub when: String,
    /// Sentences for a reader: anything the labels cannot show by themselves.
    pub notes: Vec<String>,
    /// The plugin that made the labels, when one the person added did: then they are not
    /// Volsa's own, and every place they are shown says so first.
    pub added: Option<Added>,
}

/// A plugin the person added, as a label set it made says it: its own words, cut to the
/// lengths its description may hold, and the sum of its description.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Added {
    /// Its identity.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// Who made it.
    pub made_by: String,
    /// Where it came from: text, never a link.
    pub source: String,
    /// Its licence.
    pub licence: String,
    /// Its limits, up to eight sentences.
    pub limits: Vec<String>,
    /// The SHA-256 of its description, as 64 lower-case hexadecimal digits; empty when there
    /// is no such sum.
    pub sum: String,
}

/// The longest each part of [`Added`] is kept, in characters, as a plugin's description may
/// hold it: its identity, name, version, maker, source, licence and each limit.
const ADDED_LENGTHS: [usize; 7] = [64, 60, 20, 80, 200, 200, 300];

/// The `volsa` object of a table, if its value is one. A value of any other kind says nothing.
pub(crate) fn read_made(value: &Value) -> Option<Made> {
    let object = value.as_object()?;
    let line = |key: &str| text(object, key, MAX_MADE_LINE);
    let notes = object.get("notes").and_then(Value::as_array);
    let notes = notes.map_or_else(Vec::new, |notes| {
        let words = notes.iter().filter_map(Value::as_str);
        words
            .take(MAX_NOTES)
            .map(|note| cut(note, MAX_NOTE))
            .collect()
    });
    Some(Made {
        by: line("by"),
        model: line("model"),
        model_sum: object
            .get("model_sum")
            .and_then(Value::as_str)
            .map_or_else(String::new, sum),
        device: line("device"),
        when: line("when"),
        notes,
        added: object.get("added_by_you").and_then(read_added),
    })
}

/// The plugin that made a label set, if the value is a group of fields that says it. A part
/// missing or of the wrong kind is empty; a value of any other kind says nothing.
fn read_added(value: &Value) -> Option<Added> {
    let object = value.as_object()?;
    let [id, name, version, made_by, source, licence, limit] = ADDED_LENGTHS;
    let line = |key: &str, most: usize| text(object, key, most);
    let limits = object.get("limits").and_then(Value::as_array);
    let limits = limits.map_or_else(Vec::new, |limits| {
        let words = limits.iter().filter_map(Value::as_str);
        words
            .take(MAX_NOTES)
            .map(|words| cut(words, limit))
            .collect()
    });
    Some(Added {
        id: line("id", id),
        name: line("name", name),
        version: line("version", version),
        made_by: line("made_by", made_by),
        source: line("source", source),
        licence: line("licence", licence),
        limits,
        sum: object
            .get("sum")
            .and_then(Value::as_str)
            .map_or_else(String::new, sum),
    })
}

/// The `volsa` object as it is written: cut to length, with whatever is empty or not a sum
/// left out.
#[derive(Debug, Serialize)]
pub(crate) struct VolsaObject {
    #[serde(skip_serializing_if = "String::is_empty")]
    by: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    model_sum: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    device: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    when: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    added_by_you: Option<AddedObject>,
}

/// The plugin that made a label set, as a table's `volsa` object keeps it: cut to length.
#[derive(Debug, Serialize)]
pub(crate) struct AddedObject {
    id: String,
    name: String,
    version: String,
    made_by: String,
    source: String,
    licence: String,
    limits: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    sum: String,
}

impl AddedObject {
    /// What a table keeps of the plugin that made its labels.
    fn of(added: &Added) -> Self {
        let [id, name, version, made_by, source, licence, limit] = ADDED_LENGTHS;
        Self {
            id: cut(&added.id, id),
            name: cut(&added.name, name),
            version: cut(&added.version, version),
            made_by: cut(&added.made_by, made_by),
            source: cut(&added.source, source),
            licence: cut(&added.licence, licence),
            limits: added
                .limits
                .iter()
                .take(MAX_NOTES)
                .map(|words| cut(words, limit))
                .collect(),
            sum: sum(&added.sum),
        }
    }
}

impl VolsaObject {
    /// What a table keeps of who made its labels.
    pub(crate) fn of(made: &Made) -> Self {
        Self {
            by: cut(&made.by, MAX_MADE_LINE),
            model: cut(&made.model, MAX_MADE_LINE),
            model_sum: sum(&made.model_sum),
            device: cut(&made.device, MAX_MADE_LINE),
            when: cut(&made.when, MAX_MADE_LINE),
            notes: made
                .notes
                .iter()
                .take(MAX_NOTES)
                .map(|note| cut(note, MAX_NOTE))
                .collect(),
            added_by_you: made.added.as_ref().map(AddedObject::of),
        }
    }
}

/// A text held to a length in characters, never cut inside one.
pub(crate) fn cut(text: &str, most: usize) -> String {
    text.chars().take(most).collect()
}

/// The text under a key of an object, cut; empty when there is none or it is not text.
fn text(object: &Map<String, Value>, key: &str, most: usize) -> String {
    let text = object.get(key).and_then(Value::as_str);
    text.map_or_else(String::new, |text| cut(text, most))
}

/// A sum in lower case if it is 64 hexadecimal digits; empty if it is anything else.
fn sum(text: &str) -> String {
    let is_sum = text.len() == SUM_DIGITS && text.bytes().all(|byte| byte.is_ascii_hexdigit());
    if is_sum {
        text.to_ascii_lowercase()
    } else {
        String::new()
    }
}
