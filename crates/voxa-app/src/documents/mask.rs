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
//!
//! What is replaced must never change what a report says. So an identifier of fewer than four
//! letters or digits, a name (or way of writing it) of fewer than three, and one of punctuation
//! alone are not looked for, and a number is never taken apart: `10` is not found in `10.5`,
//! `0.10` or `1,10`. A short identifier may therefore show; a figure of the report is never
//! changed. The window says under a report's heading when it replaced anything (see
//! `ui/document.rs`).
//!
//! A mask works on a plain string too: [`Mask::apply`] and [`Mask::words`] need no report, only
//! a mask made by [`Mask::of`] or [`Mask::for_document`], which any text that may name the
//! patient (a file's name or path, say) can be given.

use std::borrow::Cow;

use voxa_dicom::{Index, IndexedDocument};

use crate::naming::printable;
use crate::state::AppState;

/// What stands in for a name or an identifier found written out in a document.
pub const HIDDEN: &str = "[hidden]";
/// What stands in for the value of an item that holds a person's name.
pub const NAME_HIDDEN: &str = "Name hidden (turn on Show patient details to see it)";
/// The fewest letters and digits an identifier must hold to be looked for in a document's
/// words. A shorter one, such as `1` or `P2`, is also how numbers, sizes and series are
/// written, and replacing it would change what the report says.
const SHORTEST_IDENTIFIER: usize = 4;
/// The fewest letters and digits a name, or one way of writing it, must hold. A shorter one
/// is as likely to be a word of the report as the person.
const SHORTEST_NAME: usize = 3;

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
    /// everyone whose studies the export holds; and always the name and identifier the
    /// document's own file gives, which in an export of documents alone are the only place its
    /// patient is named.
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
        let mut people: Vec<(&Option<String>, &Option<String>)> = studies
            .iter()
            .map(|study| (&study.patient_name, &study.patient_id))
            .collect();
        people.push((&document.patient_name, &document.patient_id));
        let mut needles: Vec<Vec<char>> = Vec::new();
        for (name, id) in people {
            let given = |field: &Option<String>| field.as_deref().map(str::trim).map(str::to_owned);
            let names = given(name).into_iter().flat_map(|name| written_as(&name));
            let names = names.filter(|word| worth_looking_for(word, SHORTEST_NAME));
            let id = given(id).filter(|id| worth_looking_for(id, SHORTEST_IDENTIFIER));
            needles.extend(names.chain(id).map(|word| folded(&printable(&word))));
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

    /// Whether [`Mask::words`] of `text` says anything other than `text`: it holds an item that
    /// holds a person's name, or a place where the patient is named.
    pub fn changes(&self, text: &str, is_name: bool) -> bool {
        self.words(text, is_name) != text
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
/// as a word of its own: not as part of a longer run of letters and digits, nor taken from a
/// number written with a point or a comma in it.
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
    // A number is never taken apart: "10" is not found in "10.5", "0.10" or "1,10".
    let digit = |letter: Option<&char>| letter.is_some_and(char::is_ascii_digit);
    let mark = |letter: Option<&char>| letter.is_some_and(|letter| matches!(letter, '.' | ','));
    let goes_on = needle.last().is_some_and(char::is_ascii_digit)
        && mark(original.get(end))
        && digit(original.get(end + 1));
    let came_from = needle.first().is_some_and(char::is_ascii_digit)
        && at
            .checked_sub(1)
            .is_some_and(|before| mark(original.get(before)))
        && at
            .checked_sub(2)
            .is_some_and(|before| digit(original.get(before)));
    !joined_before && !joined_after && !goes_on && !came_from
}

/// Whether `word` holds at least `shortest` letters and digits: enough to be told from the
/// ordinary words and numbers of a report.
fn worth_looking_for(word: &str, shortest: usize) -> bool {
    word.chars()
        .filter(|letter| letter.is_alphanumeric())
        .count()
        >= shortest
}

#[cfg(test)]
mod tests;
