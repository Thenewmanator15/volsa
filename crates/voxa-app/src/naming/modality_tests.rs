//! Names of the scans that are not MR, the vocabulary's own consistency and the two groups.

use super::tests::{mr, with};
use super::*;
use voxa_core::Facing::Axial;
use voxa_core::SeriesProblem;

#[test]
fn each_other_modality_has_its_own_words() {
    let named = |modality| plain_name(&with(modality, "", None, "CHEST"));
    assert_eq!(named("CT"), "Chest CT");
    assert_eq!(named("CR"), "Chest X-ray");
    assert_eq!(named("DX"), "Chest X-ray");
    assert_eq!(named("US"), "Chest Ultrasound");
    assert_eq!(named("PT"), "Chest PET");
    assert_eq!(named("NM"), "Chest Nuclear medicine");
    assert_eq!(named("MG"), "Chest Mammogram");
    assert_eq!(named("XA"), "Chest X-ray angiography");
    assert_eq!(named(" ct "), "Chest CT", "padding and case do not matter");
    assert_eq!(
        named("rf"),
        "Chest RF",
        "an unknown modality is said by its code"
    );
    assert_eq!(plain_name(&with("US", "", None, "")), "Ultrasound");
    assert_eq!(plain_name(&with("PT", "", Some(Axial), "")), "PET, axial");
}

#[test]
fn with_no_modality_the_name_is_facts() {
    assert_eq!(plain_name(&with("", "", None, "")), "Scan 7");
    assert_eq!(
        plain_name(&with("", "", Some(Axial), "HEAD")),
        "Head, axial scan"
    );
}

#[test]
fn weighting_and_three_d_words_mean_nothing_outside_mr() {
    for modality in ["CT", "US", "DX", "PT"] {
        let mut info = with(modality, "t2 flair mprage 3d", Some(Axial), "");
        info.scan.in_3d = Some(true);
        let name = plain_name(&info);
        assert!(
            !name.contains("T2") && !name.contains("3D"),
            "{modality}: {name}"
        );
    }
}

#[test]
fn a_plane_word_is_read_from_a_description_only_where_slices_are_cut() {
    assert_eq!(plain_name(&with("CT", "cor", None, "")), "CT, coronal");
    // Where "cor" can as well be short for coronary, only the files' geometry gives a plane.
    assert_eq!(
        plain_name(&with("XA", "cor lao", None, "")),
        "X-ray angiography"
    );
    assert_eq!(plain_name(&with("US", "ax", None, "")), "Ultrasound");
    assert_eq!(
        plain_name(&with("XA", "cor", Some(Axial), "")),
        "X-ray angiography, axial"
    );
}

#[test]
fn nothing_the_scanner_wrote_outside_the_vocabulary_changes_the_name() {
    let plain = plain_name(&mr("t2 flair", Some(Axial), Some(true), "HEAD"));
    for extra in [
        "zzqq",
        "secretname",
        "John Smith 1970",
        "post gad",
        "knee",
        "brain",
    ] {
        let name = plain_name(&mr(
            &format!("t2 {extra} flair"),
            Some(Axial),
            Some(true),
            "HEAD",
        ));
        assert_eq!(name, plain, "{extra}");
    }
    let other = plain_name(&with("CT", "", Some(Axial), "HEAD"));
    assert_eq!(
        plain_name(&with("CT", "zzqq secretname", Some(Axial), "HEAD")),
        other
    );
}

#[test]
fn the_vocabulary_is_written_the_way_it_is_matched() {
    let tables = [
        words::WEIGHTINGS
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>(),
        words::PLANES.iter().map(|(key, _)| *key).collect(),
        words::MODALITIES.iter().map(|(key, _)| *key).collect(),
    ];
    for keys in tables {
        assert!(!keys.is_empty(), "a table with nothing in it");
        for (place, key) in keys.iter().enumerate() {
            assert_eq!(
                *key,
                key.to_ascii_uppercase(),
                "{key} is matched in upper case"
            );
            assert!(
                key.chars()
                    .all(|letter| letter.is_ascii_alphanumeric() || letter == '*'),
                "{key} could never be one word of a description"
            );
            assert!(
                !keys.iter().skip(place + 1).any(|other| other == key),
                "{key} twice"
            );
        }
    }
}

#[test]
fn only_files_that_are_not_pictures_of_the_body_are_other_files() {
    for code in ["SC", "OT", "DOC", " sc ", "ot", "Doc"] {
        assert_eq!(group_of(&with(code, "", None, "")), Group::Other, "{code}");
    }
    for code in ["MR", "CT", "US", "CR", "XA", "SR", "", "PT"] {
        assert_eq!(group_of(&with(code, "", None, "")), Group::Scans, "{code}");
    }
    let single = SeriesInfo {
        slices: 1,
        ..with("DX", "", None, "")
    };
    let stack = SeriesInfo {
        problems: vec![SeriesProblem::UnevenSpacing],
        ..with("MR", "", None, "")
    };
    assert_eq!(group_of(&single), Group::Scans);
    assert_eq!(group_of(&stack), Group::Scans);
    assert_eq!(group_of(&with("MR", "", None, "")), Group::Scans);
}

#[test]
fn other_files_are_called_by_what_they_are_not_by_a_code() {
    assert_eq!(
        plain_name(&with("SC", "made-up page", None, "")),
        "Scanned page or screenshot"
    );
    assert_eq!(plain_name(&with(" doc ", "", None, "")), "Document");
    assert_eq!(
        plain_name(&with("OT", "made-up map", Some(Axial), "HEAD")),
        "Other picture"
    );
    let locating = SeriesInfo {
        slices: 3,
        problems: vec![SeriesProblem::MixedOrientations],
        ..with("MR", "t2 localiser", None, "HEAD")
    };
    assert_eq!(plain_name(&locating), "Locating images");
    // Pages are told apart from the other files: only they may have a name in the picture.
    assert!(is_page(&with("sc", "", None, "")));
    assert!(!is_page(&locating));
    assert!(!is_page(&with("MR", "", None, "")));
}

#[test]
fn a_name_holds_no_character_that_turns_or_hides_text() {
    // The part of the body and an unknown modality's code reach a name as the file wrote them.
    let turned = with("C\u{202E}T", "", None, "HE\u{200B}AD\nNECK");
    let name = plain_name(&turned);
    assert_eq!(name, "Head neck CT");
    let odd = plain_name(&with("X\u{2066}Q", "", None, "\u{202B}KNEE"));
    assert_eq!(odd, "Knee XQ");
    for name in [name, odd] {
        assert!(
            !name
                .chars()
                .any(|c| c.is_control() || printable::is_format(c)),
            "{name:?}"
        );
    }
}
