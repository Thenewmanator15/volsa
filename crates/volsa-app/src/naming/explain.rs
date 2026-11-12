//! What kind of scan a series is, in a few plain sentences: what the kind is, what is bright and
//! dark in it, what it is often used for, and how fine its slices are.
//!
//! It is held to the rule of the plain names. The kind is read only from the scanner's own
//! words, when they are in the vocabulary in `words.rs`, and from the modality the files state;
//! nothing is worked out from a timing. What is bright and dark is said only while nothing the
//! files say could change it: a description that also writes a word for fat suppression, a dye
//! or a worked-out picture, a series of several volumes, and a picture in colour are each told
//! their kind and not what is bright in them. Every sentence describes a kind of scan and none
//! describes the person's own.

use volsa_core::{Facing, SeriesInfo};

use super::words::{MODALITIES, plane_word};
use super::{
    contrast_of, lookup, modality_of, other_file_name, plane_of, printable, tidy, weightings_in,
    words_of,
};

mod kinds;

pub use kinds::Kind;
use kinds::{CHANGERS, MR_KINDS, OTHER_KINDS, T1_FLAIR};

/// The heading over the sentences.
pub const ABOUT_HEADING: &str = "About this kind of scan";
/// Said last, whatever the scan.
pub const CLOSING: &str = "This describes the kind of scan, not what yours shows.";
/// Said of a file that is no scan of the body: a page, a locating image, a segmentation.
pub const NOT_A_SCAN: &str = "Not a scan of the body, so there is nothing to say of its kind.";
/// Said of a magnetic resonance scan whose description names no kind in the vocabulary.
pub const UNKNOWN_MR: &str = "A magnetic resonance scan. Its description names no kind that Volsa \
                              knows, so what is bright and dark is not said.";
/// Said of a magnetic resonance scan whose description names kinds that do not go together.
pub const MIXED_MR: &str = "A magnetic resonance scan. Its description names more than one kind, \
                            so what is bright and dark is not said.";
/// Said in place of what is bright and dark for a series of several volumes.
pub const SEVERAL: &str = "It holds several volumes that differ in what is bright and dark, so \
                           that is not said.";
/// Said in place of what is bright and dark for a picture stored in colour.
pub const COLOURED: &str = "It is stored in colour, so what is bright and dark is not said.";

/// The weightings that are a contrast alone; any other names a way of making the picture that
/// decides what is bright and dark more than the contrast does.
const PLAIN: [&str; 4] = ["T1", "T2", "PD", "3D T1"];

/// What a description's weightings come to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    /// One kind that can be explained.
    One(&'static Kind),
    /// Kinds that do not go together.
    Mixed,
    /// No kind at all.
    Nothing,
}

/// The kind in the table that a plain name says as `said`.
fn kind_said(said: &str) -> Named {
    let found = MR_KINDS.iter().find(|kind| kind.said == said);
    found.map_or(Named::Nothing, Named::One)
}

/// The one kind the weightings of a description come to. A way of making the picture is told
/// before the contrast it was written with: "T2 FLAIR" is a FLAIR. A FLAIR written with T1 is a
/// T1 FLAIR, and diffusion written with ADC is the ADC map.
fn named(weightings: &[&'static str]) -> Named {
    let ways: Vec<&str> = weightings
        .iter()
        .copied()
        .filter(|weighting| !PLAIN.contains(weighting))
        .collect();
    let with_t1 = weightings
        .iter()
        .any(|weighting| contrast_of(weighting) == Some("T1"));
    match (ways.as_slice(), weightings) {
        ([], []) => Named::Nothing,
        ([], [only]) => kind_said(only),
        (["FLAIR"], _) if with_t1 => Named::One(&T1_FLAIR),
        ([way], _) => kind_said(way),
        (["Diffusion", "ADC map"] | ["ADC map", "Diffusion"], _) => kind_said("ADC map"),
        _ => Named::Mixed,
    }
}

/// The word of a description that changes what is bright and dark, if it writes one.
fn changer_in(words: &[String]) -> Option<&str> {
    let found = words.iter().find(|word| CHANGERS.contains(&word.as_str()));
    found.map(String::as_str)
}

/// A number of millimetres to a hundredth, without trailing zeros.
fn mm(length: f64) -> f64 {
    (length * 100.0).round() / 100.0
}

/// What a plane is, in the words of looking at a body.
fn plane_means(facing: Facing) -> &'static str {
    match facing {
        Facing::Axial => "across the body",
        Facing::Coronal => "seen from the front",
        Facing::Sagittal => "seen from the side",
    }
}

/// How fine the scan is, from what its files give: the slices, their plane and the distance
/// between them, the size of a pixel, and what that means from the other two sides. A distance
/// the files did not give is not said.
fn fineness(info: &SeriesInfo, plane: Option<Facing>) -> Vec<String> {
    let mut lines = Vec::new();
    let [across, down, apart] = info.spacing_mm.map(mm);
    let stacked = info.slices > 1 && info.slice_step_given;
    let lying = plane.map(|facing| format!(" {} ({})", plane_word(facing), plane_means(facing)));
    let lying = lying.unwrap_or_default();
    lines.push(match (info.slices, stacked) {
        (1, _) => format!("One{lying} slice."),
        (count, true) => format!("{count}{lying} slices, {apart} mm apart."),
        (count, false) => format!("{count}{lying} slices."),
    });
    if info.spacing_given {
        lines.push(format!("Each pixel is {across} × {down} mm."));
    }
    if stacked && info.set.is_none() {
        lines.push(format!(
            "Seen from the other two sides, it is no sharper than the {apart} mm between its \
             slices."
        ));
    }
    lines
}

/// What is said of what is bright and dark in a scan of this kind, or why it is not said.
fn brightness(info: &SeriesInfo, kind: &Kind, changer: Option<&str>) -> Option<String> {
    if kind.shows.is_empty() {
        return None;
    }
    if info.colour {
        return Some(COLOURED.to_owned());
    }
    if info.set.is_some() {
        return Some(SEVERAL.to_owned());
    }
    Some(match changer {
        Some(word) => format!(
            "Its description also says \"{word}\", which changes what is bright and dark, so that \
             is not said."
        ),
        None => kind.shows.to_owned(),
    })
}

/// The sentences of a kind: what it is, what is bright and dark, and what it is used for.
fn of_kind(info: &SeriesInfo, kind: &Kind, changer: Option<&str>) -> Vec<String> {
    let shows = brightness(info, kind, changer);
    let used = (!kind.used.is_empty()).then(|| kind.used.to_owned());
    [Some(kind.what.to_owned()), shows, used]
        .into_iter()
        .flatten()
        .collect()
}

/// A few plain sentences about the kind of scan a series is, made only from what its files say,
/// ending with [`CLOSING`]. A file that is no scan of the body is told only that
/// ([`NOT_A_SCAN`]).
pub fn about(info: &SeriesInfo) -> Vec<String> {
    if other_file_name(info).is_some() {
        return vec![NOT_A_SCAN.to_owned()];
    }
    let code = modality_of(info);
    // A description is read for its words where a plain name reads it: for MR and for CT.
    let words = if matches!(code.as_str(), "MR" | "CT") {
        words_of(&info.description)
    } else {
        Vec::new()
    };
    let mut lines = Vec::new();
    if code == "MR" {
        match named(&weightings_in(&words)) {
            Named::One(kind) => {
                lines.extend(of_kind(info, kind, changer_in(&words)));
                let description = tidy(&printable(&info.description));
                lines.push(format!(
                    "The kind is read from the scanner's own description, \"{description}\"."
                ));
            }
            Named::Mixed => lines.push(MIXED_MR.to_owned()),
            Named::Nothing => lines.push(UNKNOWN_MR.to_owned()),
        }
    } else if let Some(said) = lookup(MODALITIES, &code) {
        let kind = OTHER_KINDS.iter().find(|kind| kind.said == said);
        lines.extend(kind.into_iter().flat_map(|kind| of_kind(info, kind, None)));
    }
    lines.extend(fineness(info, plane_of(info, &words)));
    lines.push(CLOSING.to_owned());
    lines
}

/// Every sentence kept in a constant or a table, for the check that none suggests a finding.
#[cfg(test)]
pub fn fixed_texts() -> Vec<&'static str> {
    let own = [
        ABOUT_HEADING,
        CLOSING,
        NOT_A_SCAN,
        UNKNOWN_MR,
        MIXED_MR,
        SEVERAL,
        COLOURED,
    ];
    let kinds = MR_KINDS.iter().chain(OTHER_KINDS).chain([&T1_FLAIR]);
    let of_kinds = kinds.flat_map(|kind| [kind.what, kind.shows, kind.used]);
    own.into_iter().chain(of_kinds).collect()
}

#[cfg(test)]
mod tests;
