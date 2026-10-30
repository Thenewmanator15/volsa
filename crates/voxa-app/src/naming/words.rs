//! The words a plain name may repeat from a scanner's description, and what it says for each.
//!
//! These are plain tables. Each key is a word the scanner itself wrote, in upper case; a
//! description is split into words and compared with the keys ignoring case, so a word that
//! is not a key is never carried into a name. Nothing here is worked out from a timing: a
//! weighting is named only because the scanner wrote its name.

use voxa_core::Facing;

/// Weighting and sequence words, each with what a plain name says for it. Several words may
/// say the same thing, such as `DWI` and `TRACE`.
pub(super) const WEIGHTINGS: &[(&str, &str)] = &[
    ("T1", "T1"),
    ("T1W", "T1"),
    ("T2", "T2"),
    ("T2W", "T2"),
    ("PD", "PD"),
    ("PDW", "PD"),
    ("FLAIR", "FLAIR"),
    ("T2*", "T2*"),
    ("DWI", "Diffusion"),
    ("TRACE", "Diffusion"),
    ("DIFFUSION", "Diffusion"),
    ("ADC", "ADC map"),
    ("SWI", "SWI"),
    ("SWAN", "SWI"),
    ("STIR", "STIR"),
    ("TOF", "Angiography (TOF)"),
    ("MPRAGE", "3D T1"),
    ("BRAVO", "3D T1"),
];

/// Pairs of words that a scanner may write as two, such as `MP-RAGE`, and that are one word
/// of the vocabulary when joined.
pub(super) const JOINED: &[(&str, &str)] = &[("MP", "RAGE")];

/// Words for the plane the slices lie in.
pub(super) const PLANES: &[(&str, Facing)] = &[
    ("AX", Facing::Axial),
    ("AXIAL", Facing::Axial),
    ("TRA", Facing::Axial),
    ("SAG", Facing::Sagittal),
    ("SAGITTAL", Facing::Sagittal),
    ("COR", Facing::Coronal),
    ("CORONAL", Facing::Coronal),
];

/// What a plain name says for each modality other than MR, keyed by the modality's DICOM
/// code. A code not listed is said as it is.
pub(super) const MODALITIES: &[(&str, &str)] = &[
    ("CT", "CT"),
    ("CR", "X-ray"),
    ("DX", "X-ray"),
    ("US", "Ultrasound"),
    ("PT", "PET"),
    ("NM", "Nuclear medicine"),
    ("MG", "Mammogram"),
    ("XA", "X-ray angiography"),
];

/// The plane as a lower-case word inside a name.
pub(super) fn plane_word(facing: Facing) -> &'static str {
    match facing {
        Facing::Axial => "axial",
        Facing::Coronal => "coronal",
        Facing::Sagittal => "sagittal",
    }
}
