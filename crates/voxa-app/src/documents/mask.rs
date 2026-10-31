//! Leaving the patient's name and identifier out of what a document says while the patient's
//! details are hidden.
//!
//! A report is shown as it was written (see `ui/document.rs`), and what it was written with can
//! include the person's name: an item that holds a name, a sentence that names them, a title
//! that does. While the details are hidden, an item that holds a name shows [`NAME_HIDDEN`] in
//! place of the name, and each place where the study's patient name or identifier is written
//! out, and a DICOM name's parts joined by a space in either order, is replaced by [`HIDDEN`].
//! A name written another way inside free text, such as with an initial, a title, a different
//! spelling or only a surname, cannot be found reliably and may show.

use std::borrow::Cow;

use voxa_dicom::{Index, IndexedDocument};

use crate::state::AppState;

/// What stands in for a name or an identifier found written out in a document.
pub const HIDDEN: &str = "[hidden]";
/// What stands in for the value of an item that holds a person's name.
pub const NAME_HIDDEN: &str = "Name hidden (turn on Show patient details to see it)";

/// What is left out of what a document says, and how.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mask {
    /// Whether the patient's details are hidden at all.
    hiding: bool,
    /// What is replaced by [`HIDDEN`], each in lower case, longest first.
    needles: Vec<Vec<char>>,
}

impl Mask {
    /// A mask that leaves everything as written: the patient's details are shown.
    pub fn off() -> Self {
        Self::default()
    }

    /// The mask for the words of `document`, given what the export says of whose it is: while
    /// `show_patient` is false, the name and identifier of the patient of the study it says it
    /// belongs to; or, when that study holds no scans and so says nothing of whose it is, of
    /// everyone whose studies the export holds.
    pub fn of(index: &Index, document: &IndexedDocument, show_patient: bool) -> Self {
        if show_patient {
            return Self::off();
        }
        let own = index
            .studies
            .iter()
            .filter(|study| study.id.0 == document.study_uid);
        let mut studies: Vec<_> = own.collect();
        if studies.is_empty() {
            studies = index.studies.iter().collect();
        }
        let mut needles: Vec<Vec<char>> = Vec::new();
        for study in studies {
            let given = |field: &Option<String>| field.as_deref().map(str::trim).map(str::to_owned);
            let words = given(&study.patient_name)
                .into_iter()
                .flat_map(|name| written_as(&name))
                .chain(given(&study.patient_id));
            needles.extend(
                words
                    .filter(|word| !word.is_empty())
                    .map(|word| folded(&word)),
            );
        }
        needles.sort_by_key(|needle| std::cmp::Reverse(needle.len()));
        needles.dedup();
        Self {
            hiding: true,
            needles,
        }
    }

    /// The mask for the words of `document` in `state`.
    pub fn for_document(state: &AppState, document: &IndexedDocument) -> Self {
        match &state.index {
            Some(index) => Self::of(index, document, state.show_patient),
            None => Self::default(),
        }
    }

    /// What a line of a document says: for the line of an item that holds a person's name, its
    /// own name and then [`NAME_HIDDEN`]; for any other, `text` itself; each place the patient
    /// is named replaced by [`HIDDEN`].
    pub fn words<'a>(&self, text: &'a str, is_name: bool) -> Cow<'a, str> {
        if self.hiding && is_name {
            let hidden = name_hidden(text);
            return Cow::Owned(self.apply(&hidden).into_owned());
        }
        self.apply(text)
    }

    /// `text` with each place the patient is named replaced by [`HIDDEN`]; `text` itself, not
    /// copied, where there is none.
    pub fn apply<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.needles.is_empty() {
            return Cow::Borrowed(text);
        }
        let original: Vec<char> = text.chars().collect();
        let lower: Vec<char> = original.iter().map(|letter| fold(*letter)).collect();
        let mut out = String::new();
        let (mut at, mut copied, mut found) = (0, 0, false);
        while at < lower.len() {
            let matched = self
                .needles
                .iter()
                .find(|needle| is_at(&original, &lower, at, needle));
            let Some(needle) = matched else {
                at += 1;
                continue;
            };
            found = true;
            out.extend(original.get(copied..at).unwrap_or_default());
            out.push_str(HIDDEN);
            at += needle.len();
            copied = at;
        }
        if !found {
            return Cow::Borrowed(text);
        }
        out.extend(original.get(copied..).unwrap_or_default());
        Cow::Owned(out)
    }
}

/// The ways a patient's name is written out: as the file has it (`Family^Given`), and a name of
/// several parts with its parts joined by a space, in order and in the other order.
fn written_as(name: &str) -> Vec<String> {
    let parts: Vec<&str> = name
        .split('^')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let mut written = vec![name.to_owned()];
    if parts.len() > 1 {
        written.push(parts.join(" "));
        let backwards: Vec<&str> = parts.iter().rev().copied().collect();
        written.push(backwards.join(" "));
    }
    written
}

/// An item that holds a person's name, as `name: value`, with the value left out. Whatever
/// comes after the first colon and space is the value; a line with none is wholly the value,
/// and one with nothing after its colon has nothing to hide.
fn name_hidden(text: &str) -> String {
    match text.split_once(": ") {
        Some((concept, _)) => format!("{concept}: {NAME_HIDDEN}"),
        None if text.ends_with(':') => text.to_owned(),
        None => NAME_HIDDEN.to_owned(),
    }
}

/// A letter in lower case; one letter for one, so that places in the folded text are places in
/// the text itself.
fn fold(letter: char) -> char {
    letter.to_lowercase().next().unwrap_or(letter)
}

fn folded(text: &str) -> Vec<char> {
    text.chars().map(fold).collect()
}

/// Whether `needle` is written at `at` in the text (`original`, and `lower`, its folded form)
/// as a word of its own: not as part of a longer run of letters and digits.
fn is_at(original: &[char], lower: &[char], at: usize, needle: &[char]) -> bool {
    let end = at + needle.len();
    if lower.get(at..end) != Some(needle) {
        return false;
    }
    let inside = |letter: Option<&char>| letter.is_some_and(|letter| letter.is_alphanumeric());
    let before = at.checked_sub(1).and_then(|before| original.get(before));
    let starts_a_word = needle.first().is_some_and(|first| first.is_alphanumeric());
    let ends_a_word = needle.last().is_some_and(|last| last.is_alphanumeric());
    let joined_before = starts_a_word && inside(before);
    let joined_after = ends_a_word && inside(original.get(end));
    !joined_before && !joined_after
}

#[cfg(test)]
mod tests;
