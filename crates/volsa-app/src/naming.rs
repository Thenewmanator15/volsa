//! Plain names for scans, and which of two lists a file belongs in.
//!
//! A plain name tells a person what a scan is in a few ordinary words. A wrong one could
//! lead them to compare or measure the wrong scan, so a name is made only from what the files
//! say: the scanner's own words in its description, when they are in the vocabulary in
//! `words.rs`, and facts the files state, such as the plane of the slices, the part of the body
//! and the modality. A word that is not in the vocabulary never reaches a name, and no
//! weighting is ever worked out from a timing. With nothing recognised, the name is the facts
//! alone. The scanner's own description is always shown beneath a name, unchanged.

pub use printable::{printable, printable_multiline};
use volsa_core::{Facing, SeriesInfo, SeriesKind};
use words::{BODY_PARTS, JOINED, MODALITIES, PLANES, WEIGHTINGS, plane_word};

mod printable;
mod words;

/// The three contrasts a weighting can be made of. A description that writes two different
/// ones of them contradicts itself, so no weighting is named.
const CONTRASTS: [&str; 3] = ["T1", "T2", "PD"];

/// The modalities whose files are pages or screenshots, or radiotherapy images and doses: not
/// pictures of the body to look at, and ones that often have text written into the picture.
const PAGES: [&str; 5] = ["SC", "OT", "DOC", "RTIMAGE", "RTDOSE"];

/// The modalities of objects that carry pixels but are no picture of the body to look at: a
/// segmentation, a radiotherapy dose or image, a presentation state and a key-image note. Each
/// is called by its kind in words.
const NOT_PICTURES: [(&str, &str); 5] = [
    ("SEG", "Segmentation"),
    ("RTDOSE", "RT dose"),
    ("RTIMAGE", "RT image"),
    ("PR", "Presentation state"),
    ("KO", "Key-image note"),
];

/// Which of the two lists a file is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Pictures of the body: volumes, plain stacks, single images, many-frame series, in
    /// grey or in colour, and series Volsa cannot read yet.
    Scans,
    /// Locating images, files whose modality says they are pages, screenshots or other
    /// documents, and objects that carry pixels but are no picture of the body to look at.
    Other,
}

/// The list a series is shown in: [`Group::Other`] for locating images, for a modality of
/// `SC`, `OT` or `DOC`, and for a segmentation, a radiotherapy dose or image, a presentation
/// state or a key-image note (`SEG`, `RTDOSE`, `RTIMAGE`, `PR`, `KO`); [`Group::Scans`] for
/// everything else, a series that cannot be read and a series in colour among them.
pub fn group_of(info: &SeriesInfo) -> Group {
    let shown_elsewhere = is_page(info) || not_a_picture(info).is_some();
    if shown_elsewhere || matches!(info.kind(), SeriesKind::Locating) {
        Group::Other
    } else {
        Group::Scans
    }
}

/// Whether a series' modality says it is pages, screenshots or other documents (`SC`, `OT`,
/// `DOC`), or a radiotherapy image or dose (`RTIMAGE`, `RTDOSE`). Such a picture often carries
/// the patient's name written into it, so it is not drawn small while the patient's details
/// are hidden.
pub fn is_page(info: &SeriesInfo) -> bool {
    PAGES.contains(&modality_of(info).as_str())
}

/// What an object that carries pixels but is no picture of the body is: `None` for anything
/// else.
fn not_a_picture(info: &SeriesInfo) -> Option<&'static str> {
    lookup(&NOT_PICTURES, &modality_of(info))
}

/// What a file of the other files is called in place of a plain name, by what it is: `None`
/// for a scan.
fn other_file_name(info: &SeriesInfo) -> Option<&'static str> {
    match modality_of(info).as_str() {
        "SC" => Some("Scanned page or screenshot"),
        "DOC" => Some("Document"),
        "OT" => Some("Other picture"),
        _ => not_a_picture(info)
            .or_else(|| matches!(info.kind(), SeriesKind::Locating).then_some("Locating images")),
    }
}

/// A scan's name in plain words, made only from what its files say.
///
/// A magnetic resonance scan is named by the weighting its description spells out, with the
/// part of the body, whether it was acquired in 3D and its plane: "Head, 3D T1, sagittal".
/// With no weighting, or with a description that names two different ones of T1, T2 and PD,
/// it is named by the facts alone: "Axial scan, 3D", or "Scan 7" with none known. Any other
/// modality is named by what it is and the part of the body: "Chest X-ray". A file of the other
/// files is called by what it is: "Locating images", "Scanned page or screenshot", "Document",
/// "Other picture", "Segmentation", "RT dose", "RT image", "Presentation state" or "Key-image
/// note". A name holds no control or format character, whatever the file held.
pub fn plain_name(info: &SeriesInfo) -> String {
    if let Some(name) = other_file_name(info) {
        return name.to_owned();
    }
    match modality_of(info).as_str() {
        "MR" => mr_name(info),
        code => other_name(info, code),
    }
}

/// The name of a magnetic resonance scan.
fn mr_name(info: &SeriesInfo) -> String {
    let words = words_of(&info.description);
    let weightings = weightings_in(&words);
    let three_d = info.scan.in_3d == Some(true) || words.iter().any(|word| word == "3D");
    let plane = plane_of(info, &words);
    if weightings.is_empty() {
        return facts_name(info, plane, three_d);
    }
    let says_three_d = weightings
        .iter()
        .any(|weighting| weighting.starts_with("3D "));
    let mut parts: Vec<String> = Vec::new();
    parts.extend(body_of(info));
    parts.push(weightings.join(" "));
    if three_d && !says_three_d {
        parts.push("3D".to_owned());
    }
    parts.extend(plane.map(|facing| plane_word(facing).to_owned()));
    parts.join(", ")
}

/// The name of a scan made by any other modality: the part of the body and what the modality
/// is, then the plane. A modality that is not given leaves only the facts.
///
/// Only a CT description is read for a plane word, as an MR one is. For other modalities the
/// word `COR` may as well be short for coronary, so only the geometry gives a plane.
fn other_name(info: &SeriesInfo, code: &str) -> String {
    let what = lookup(MODALITIES, code).unwrap_or(code);
    let words = if code == "CT" {
        words_of(&info.description)
    } else {
        Vec::new()
    };
    let plane = plane_of(info, &words);
    if what.is_empty() {
        return facts_name(info, plane, info.scan.in_3d == Some(true));
    }
    let mut parts = vec![
        [body_of(info), Some(what.to_owned())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
    ];
    parts.extend(plane.map(|facing| plane_word(facing).to_owned()));
    parts.join(", ")
}

/// The name of a scan when no weighting is known: the plane, the part of the body and
/// whether it is 3D, as far as they are known, or its series number when none is.
fn facts_name(info: &SeriesInfo, plane: Option<Facing>, three_d: bool) -> String {
    let mut name = match (plane, body_of(info)) {
        (Some(facing), Some(body)) => format!("{body}, {} scan", plane_word(facing)),
        (Some(facing), None) => format!("{} scan", sentence_case(plane_word(facing))),
        (None, Some(body)) => format!("{body} scan"),
        (None, None) => format!("Scan {}", info.number),
    };
    if three_d {
        name.push_str(", 3D");
    }
    name
}

/// What the weighting words of a description say, each once, in the order they were written.
///
/// A weighting that another says in full ("T1" within "3D T1") is not said twice. When the
/// words name two different ones of T1, T2 and PD, the description contradicts itself and
/// nothing at all is said.
fn weightings_in(words: &[String]) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for weighting in words.iter().filter_map(|word| lookup(WEIGHTINGS, word)) {
        if !found.contains(&weighting) {
            found.push(weighting);
        }
    }
    let mut contrasts: Vec<&str> = found
        .iter()
        .filter_map(|weighting| contrast_of(weighting))
        .collect();
    contrasts.sort_unstable();
    contrasts.dedup();
    if contrasts.len() > 1 {
        return Vec::new();
    }
    let all = found.clone();
    found.retain(|weighting| {
        !all.iter().any(|other| {
            other
                .strip_suffix(*weighting)
                .is_some_and(|front| front.ends_with(' '))
        })
    });
    found
}

/// The one of T1, T2 and PD that a weighting is made of, if it is.
fn contrast_of(weighting: &str) -> Option<&'static str> {
    CONTRASTS.into_iter().find(|contrast| {
        // A star keeps the contrast: T2* is a T2 weighting, and does not agree with T1.
        let made_of = |word: &str| word.trim_end_matches('*') == *contrast;
        weighting.split(' ').any(made_of)
    })
}

/// The plane of the slices: the geometry of the files first, else the one plane the
/// description writes. A description that writes two different planes gives none.
fn plane_of(info: &SeriesInfo, words: &[String]) -> Option<Facing> {
    if let Some(plane) = info.scan.plane {
        return Some(plane.nearest);
    }
    let mut written = words.iter().filter_map(|word| lookup(PLANES, word));
    let first = written.next()?;
    written.all(|facing| facing == first).then_some(first)
}

/// The part of the body, as the file's own element gives it, in sentence case: "Head". A term
/// that runs words together is spelled out as they are: "Cervical spine". `None` when the
/// element is absent or blank.
fn body_of(info: &SeriesInfo) -> Option<String> {
    let text = tidy(&info.scan.body_part);
    let spelled = lookup(BODY_PARTS, &text.to_ascii_uppercase());
    spelled
        .map(str::to_owned)
        .or_else(|| (!text.is_empty()).then(|| sentence_case(&text)))
}

/// The modality's code, in upper case.
fn modality_of(info: &SeriesInfo) -> String {
    tidy(&info.modality).to_ascii_uppercase()
}

/// The words of a description in upper case, split on anything that is not a letter or a
/// digit. A star that follows a word stays with it, so `T2*` is one word. `MP-RAGE` is joined
/// into one.
fn words_of(description: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for letter in description.chars() {
        if letter.is_alphanumeric() || (letter == '*' && !word.is_empty()) {
            word.push(letter.to_ascii_uppercase());
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    join_split(words)
}

/// The words with each pair that the vocabulary reads as one joined: `MP` then `RAGE`.
fn join_split(words: Vec<String>) -> Vec<String> {
    let mut joined = Vec::with_capacity(words.len());
    let mut rest = words.into_iter().peekable();
    while let Some(word) = rest.next() {
        let second = lookup(JOINED, &word);
        match second {
            Some(second) if rest.peek().is_some_and(|next| next == second) => {
                rest.next();
                joined.push(format!("{word}{second}"));
            }
            _ => joined.push(word),
        }
    }
    joined
}

/// What a table of words says for this word. The words in a table are in upper case.
fn lookup<T: Copy>(table: &[(&str, T)], word: &str) -> Option<T> {
    table
        .iter()
        .find(|(key, _)| *key == word)
        .map(|(_, value)| *value)
}

/// Text from a file with underscores and control characters taken for spaces, format
/// characters left out, and runs of spaces and the ends trimmed.
fn tidy(text: &str) -> String {
    let spaced = printable(&text.replace('_', " "));
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text with a capital first letter and the rest in lower case.
fn sentence_case(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(letters.flat_map(char::to_lowercase))
            .collect()
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod modality_tests;
