//! The sentences about a kind of scan: made only from what the files say, and withheld where
//! what they say could make them wrong.

use volsa_core::Facing::{Axial, Sagittal};
use volsa_core::{SeriesId, VolumeSet};

use super::super::tests::{mr, with};
use super::super::words::WEIGHTINGS;
use super::*;

/// The sentences about an MR series with this description and nothing else known, joined.
fn said_of(description: &str) -> String {
    about(&mr(description, None, None, "")).join("\n")
}

#[test]
fn every_weighting_and_modality_of_the_vocabulary_has_its_sentences() {
    for (_, said) in WEIGHTINGS {
        assert!(matches!(kind_said(said), Named::One(_)), "{said}");
    }
    for (_, said) in MODALITIES {
        assert!(OTHER_KINDS.iter().any(|kind| kind.said == *said), "{said}");
    }
    // And no kind is kept that the vocabulary cannot name.
    for kind in MR_KINDS {
        let named = WEIGHTINGS.iter().any(|(_, said)| *said == kind.said);
        assert!(named, "{}", kind.said);
    }
}

#[test]
fn a_kind_the_scanner_wrote_is_explained_with_the_words_it_was_read_from() {
    let said = said_of("demo_T2W_TSE");
    assert!(said.starts_with("A T2-weighted picture."), "{said}");
    assert!(said.contains("Watery fluid is bright."), "{said}");
    assert!(
        said.contains("read from the scanner's own description, \"demo T2W TSE\""),
        "{said}"
    );
    assert!(said.ends_with(CLOSING), "{said}");
}

#[test]
fn a_way_of_making_the_picture_is_told_before_its_contrast() {
    assert!(said_of("T2 FLAIR").starts_with("A FLAIR picture"));
    assert!(said_of("FLAIR").contains("Free fluid"));
    // A FLAIR written with T1 is not like T2.
    let t1 = said_of("T1 FLAIR");
    assert!(t1.starts_with("A T1-weighted FLAIR picture"), "{t1}");
    assert!(!t1.contains("T2-weighted"), "{t1}");
    assert!(said_of("T2 STIR").contains("Fat is dark"));
    for both in ["DWI ADC", "ADC DWI"] {
        assert!(said_of(both).starts_with("An ADC map"), "{both}");
    }
}

#[test]
fn nothing_is_said_of_brightness_for_a_kind_that_is_unknown_or_mixed() {
    // A timing names no weighting, and neither does a word outside the vocabulary.
    let mut timed = mr("demo sequence", None, None, "");
    (timed.scan.repetition_ms, timed.scan.echo_ms) = (Some(4000.0), Some(100.0));
    let said = about(&timed).join("\n");
    assert!(said.starts_with(UNKNOWN_MR), "{said}");
    assert!(
        !said.contains("Fat is") && !said.contains("fluid is"),
        "{said}"
    );
    // Two contrasts contradict each other; two ways of making a picture do not go together.
    assert!(said_of("T1 T2").starts_with(UNKNOWN_MR));
    assert!(said_of("FLAIR SWI").starts_with(MIXED_MR));
}

#[test]
fn a_word_that_changes_the_contrast_stops_what_is_bright_being_said() {
    for description in [
        "T1 FS",
        "T1 +C",
        "T2 fat sat",
        "t1 post gad",
        "T1 DIXON water",
    ] {
        let said = said_of(description);
        assert!(!said.contains("Fat is bright"), "{description}: {said}");
        assert!(
            !said.contains("Watery fluid is bright."),
            "{description}: {said}"
        );
        assert!(
            said.contains("which changes what is bright and dark"),
            "{description}: {said}"
        );
        assert!(
            said.contains("weighted picture"),
            "the kind is still told: {said}"
        );
    }
    assert!(said_of("T1 FS").contains("also says \"FS\""));
}

#[test]
fn several_volumes_and_colour_stop_what_is_bright_being_said() {
    let mut diffusion = mr("DWI", None, None, "");
    diffusion.set = Some(VolumeSet {
        of: SeriesId("made-up".to_owned()),
        at: 0,
        count: 2,
        what: "b 0".to_owned(),
    });
    let said = about(&diffusion).join("\n");
    assert!(said.contains(SEVERAL), "{said}");
    assert!(!said.contains("the brighter it is"), "{said}");
    let mut doppler = with("US", "", None, "");
    doppler.colour = true;
    let said = about(&doppler).join("\n");
    assert!(said.contains(COLOURED), "{said}");
    assert!(!said.contains("the brighter it is"), "{said}");
}

#[test]
fn how_fine_the_scan_is_comes_only_from_distances_the_files_gave() {
    let mut scan = mr("T1", Some(Sagittal), None, "");
    scan.spacing_mm = [0.5, 0.5, 4.0];
    (scan.spacing_given, scan.slice_step_given) = (true, true);
    let said = about(&scan).join("\n");
    assert!(
        said.contains("20 sagittal (seen from the side) slices, 4 mm apart."),
        "{said}"
    );
    assert!(said.contains("Each pixel is 0.5 × 0.5 mm."), "{said}");
    assert!(
        said.contains("no sharper than the 4 mm between its slices"),
        "{said}"
    );
    // The stand-in of 1 mm is no distance to say.
    (scan.spacing_given, scan.slice_step_given) = (false, false);
    scan.spacing_mm = [1.0, 1.0, 1.0];
    let said = about(&scan).join("\n");
    assert!(
        said.contains("20 sagittal (seen from the side) slices."),
        "{said}"
    );
    assert!(!said.contains("mm"), "{said}");
}

#[test]
fn other_modalities_are_told_by_what_the_files_name_them() {
    let ct = about(&with("CT", "demo T1 words", Some(Axial), "")).join("\n");
    assert!(ct.starts_with("A CT scan"), "{ct}");
    assert!(ct.contains("bone is bright"), "{ct}");
    assert!(
        !ct.contains("T1-weighted"),
        "an MR word in a CT description: {ct}"
    );
    assert!(ct.contains("20 axial (across the body) slices"), "{ct}");
    let xray = about(&with("DX", "", None, "")).join("\n");
    assert!(xray.starts_with("An X-ray picture"), "{xray}");
    // A modality with no sentences of its own is told only how fine it is.
    let unknown = about(&with("ZZ", "", None, ""));
    assert_eq!(unknown, ["20 slices.", CLOSING]);
}

#[test]
fn a_file_that_is_no_scan_of_the_body_is_told_only_that() {
    assert_eq!(about(&with("SC", "T1", None, "")), [NOT_A_SCAN]);
    assert_eq!(about(&with("SEG", "", None, "")), [NOT_A_SCAN]);
}

#[test]
fn a_description_is_made_printable_before_it_is_repeated() {
    let said = said_of("T1\u{202e}\u{7}evil");
    assert!(
        !said.contains('\u{202e}') && !said.contains('\u{7}'),
        "{said:?}"
    );
}
