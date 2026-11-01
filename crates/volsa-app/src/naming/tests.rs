use super::*;
use volsa_core::Facing::{self, Axial, Coronal, Sagittal};
use volsa_core::{Plane, ScanFacts, SeriesProblem};

/// A twenty-slice series of this modality, numbered 7, with only these facts known.
pub(super) fn with(
    modality: &str,
    description: &str,
    plane: Option<Facing>,
    body: &str,
) -> SeriesInfo {
    SeriesInfo {
        number: 7,
        slices: 20,
        modality: modality.to_owned(),
        description: description.to_owned(),
        scan: ScanFacts {
            plane: plane.map(|nearest| Plane {
                nearest,
                oblique: false,
            }),
            body_part: body.to_owned(),
            ..ScanFacts::default()
        },
        ..SeriesInfo::default()
    }
}

pub(super) fn mr(
    description: &str,
    plane: Option<Facing>,
    in_3d: Option<bool>,
    body: &str,
) -> SeriesInfo {
    let mut info = with("MR", description, plane, body);
    info.scan.in_3d = in_3d;
    info
}

/// The name of an axial MR series with this description and nothing else known.
fn axial(description: &str) -> String {
    plain_name(&mr(description, Some(Axial), None, ""))
}

fn locating() -> SeriesInfo {
    SeriesInfo {
        slices: 3,
        problems: vec![SeriesProblem::MixedOrientations],
        ..with("MR", "demo localiser", None, "")
    }
}

fn unreadable_mr() -> SeriesInfo {
    SeriesInfo {
        problems: vec![SeriesProblem::UnsupportedTransferSyntax("demo".to_owned())],
        ..with("MR", "demo t2", Some(Axial), "")
    }
}

fn colour_us() -> SeriesInfo {
    SeriesInfo {
        colour: true,
        ..with("US", "demo clip", None, "")
    }
}

#[test]
fn known_words_become_a_plain_name() {
    assert_eq!(
        plain_name(&mr("sag_mprage_demo", Some(Sagittal), Some(true), "HEAD")),
        "Head, 3D T1, sagittal"
    );
    assert_eq!(
        plain_name(&mr("ax_t2_demo", Some(Axial), Some(false), "")),
        "T2, axial"
    );
}

#[test]
fn conflicting_weightings_name_none() {
    assert_eq!(
        plain_name(&mr("demo t1 t2 compare", Some(Axial), None, "")),
        "Axial scan"
    );
}

#[test]
fn with_nothing_known_the_name_is_facts() {
    assert_eq!(
        plain_name(&mr("", Some(Coronal), Some(true), "")),
        "Coronal scan, 3D"
    );
    assert_eq!(plain_name(&mr("xyz123", None, None, "")), "Scan 7");
}

#[test]
fn the_body_part_comes_only_from_its_element() {
    assert_eq!(
        plain_name(&mr("brain t1 post", Some(Axial), None, "")),
        "T1, axial"
    );
}

#[test]
fn unknown_words_never_reach_the_name() {
    let name = plain_name(&mr("t2 zzqq secretname", Some(Axial), None, ""));
    assert!(!name.to_lowercase().contains("zzqq") && !name.to_lowercase().contains("secretname"));
}

#[test]
fn other_modalities_say_what_they_are() {
    assert_eq!(
        plain_name(&with("CT", "", Some(Axial), "HEAD")),
        "Head CT, axial"
    );
    assert_eq!(plain_name(&with("DX", "", None, "CHEST")), "Chest X-ray");
}

#[test]
fn pages_and_locating_images_are_other_files() {
    assert_eq!(group_of(&with("SC", "", None, "")), Group::Other);
    assert_eq!(group_of(&locating()), Group::Other);
    assert_eq!(group_of(&unreadable_mr()), Group::Scans);
    assert_eq!(group_of(&colour_us()), Group::Scans);
}

#[test]
fn every_scanner_word_has_one_plain_name() {
    let weightings = [
        ("t1", "T1"),
        ("t2", "T2"),
        ("pd", "PD"),
        ("flair", "FLAIR"),
        ("t2*", "T2*"),
        ("dwi", "Diffusion"),
        ("trace", "Diffusion"),
        ("diffusion", "Diffusion"),
        ("adc", "ADC map"),
        ("swi", "SWI"),
        ("swan", "SWI"),
        ("stir", "STIR"),
        ("tof", "Angiography (TOF)"),
        ("mprage", "3D T1"),
        ("mp-rage", "3D T1"),
        ("bravo", "3D T1"),
    ];
    for (written, plain) in weightings {
        assert_eq!(axial(written), format!("{plain}, axial"), "{written}");
    }
    let planes = [
        ("ax", Axial),
        ("axial", Axial),
        ("tra", Axial),
        ("sag", Sagittal),
        ("sagittal", Sagittal),
        ("cor", Coronal),
        ("coronal", Coronal),
    ];
    for (written, facing) in planes {
        let name = plain_name(&mr(&format!("{written} t2"), None, None, ""));
        let word = format!("{facing:?}").to_lowercase();
        assert_eq!(name, format!("T2, {word}"), "{written}");
    }
}

#[test]
fn words_are_matched_whole_without_regard_to_case_or_punctuation() {
    assert_eq!(axial("AX_T2-FLAIR"), "T2 FLAIR, axial");
    assert_eq!(axial("[Sag] (T1)"), "T1, axial", "geometry says axial");
    assert_eq!(axial("t2*_gre"), "T2*, axial");
    assert_eq!(
        axial("t2 * gre"),
        "T2, axial",
        "a lone star is not part of a word"
    );
    // Each of these holds a vocabulary word inside a longer word, which is not that word.
    for longer in [
        "relaxed", "t1x", "flairs", "extra", "corner", "swing", "pdf", "t22",
    ] {
        assert_eq!(axial(longer), "Axial scan", "{longer}");
    }
    // Letters from other alphabets that change case into plain ones are not those letters.
    for lookalike in ["\u{17f}tir", "fla\u{131}r"] {
        assert_eq!(axial(lookalike), "Axial scan", "{lookalike}");
    }
}

#[test]
fn several_weightings_are_joined_in_the_order_written_and_each_said_once() {
    assert_eq!(axial("t2 flair"), "T2 FLAIR, axial");
    assert_eq!(axial("flair t2"), "FLAIR T2, axial");
    assert_eq!(axial("t2 t2 flair flair"), "T2 FLAIR, axial");
    assert_eq!(axial("dwi trace"), "Diffusion, axial");
    assert_eq!(axial("dwi adc"), "Diffusion ADC map, axial");
}

#[test]
fn a_weighting_that_says_3d_is_not_followed_by_3d_again() {
    let three_d = |description: &str| plain_name(&mr(description, Some(Axial), Some(true), ""));
    assert_eq!(three_d("mprage"), "3D T1, axial");
    assert_eq!(three_d("bravo 3d"), "3D T1, axial");
    assert_eq!(
        three_d("t1 mprage"),
        "3D T1, axial",
        "T1 is said once, as part of 3D T1"
    );
    assert_eq!(three_d("t2 flair"), "T2 FLAIR, 3D, axial");
}

#[test]
fn two_of_t1_t2_and_pd_name_no_weighting_at_all() {
    for conflict in [
        "t1 pd",
        "pd t2",
        "t1 t2 flair",
        "t2 mprage",
        "bravo pd stir",
    ] {
        assert_eq!(axial(conflict), "Axial scan", "{conflict}");
    }
    // Naming none also drops the 3D that a sequence name would have implied.
    assert_eq!(axial("mprage t2"), "Axial scan");
    assert_eq!(
        axial("t1 t1"),
        "T1, axial",
        "one weighting twice is not a conflict"
    );
    assert_eq!(axial("t1 flair"), "T1 FLAIR, axial");
}

#[test]
fn the_plane_comes_from_the_geometry_before_the_description() {
    let named = |description, plane| plain_name(&mr(description, plane, None, ""));
    assert_eq!(named("ax t2", Some(Sagittal)), "T2, sagittal");
    assert_eq!(named("ax t2", None), "T2, axial");
    assert_eq!(named("t2", None), "T2");
    assert_eq!(
        named("ax sag t2", None),
        "T2",
        "two planes written is no plane"
    );
}

#[test]
fn a_tilted_plane_is_named_by_its_nearest() {
    let oblique = Plane {
        nearest: Coronal,
        oblique: true,
    };
    let mut info = mr("t2", None, None, "");
    info.scan.plane = Some(oblique);
    assert_eq!(plain_name(&info), "T2, coronal");
    info.description.clear();
    assert_eq!(plain_name(&info), "Coronal scan");
}

#[test]
fn three_d_comes_from_the_files_or_the_word() {
    let named = |description, in_3d| plain_name(&mr(description, Some(Axial), in_3d, ""));
    assert_eq!(named("t2", Some(true)), "T2, 3D, axial");
    assert_eq!(named("t2 3d", None), "T2, 3D, axial");
    assert_eq!(named("t2", Some(false)), "T2, axial");
    assert_eq!(named("3D", None), "Axial scan, 3D");
}

#[test]
fn without_a_weighting_the_name_is_the_facts_that_are_known() {
    let named = |plane, in_3d, body| plain_name(&mr("zzqq", plane, in_3d, body));
    assert_eq!(
        named(Some(Axial), Some(true), "HEAD"),
        "Head, axial scan, 3D"
    );
    assert_eq!(named(Some(Axial), None, "HEAD"), "Head, axial scan");
    assert_eq!(named(Some(Sagittal), None, ""), "Sagittal scan");
    assert_eq!(named(None, Some(true), "HEAD"), "Head scan, 3D");
    assert_eq!(named(None, None, "HEAD"), "Head scan");
    assert_eq!(named(None, Some(true), ""), "Scan 7, 3D");
    assert_eq!(named(None, Some(false), ""), "Scan 7");
}

#[test]
fn the_body_part_is_said_in_plain_letters() {
    let named = |body| plain_name(&mr("t2", Some(Axial), None, body));
    assert_eq!(named("HEAD"), "Head, T2, axial");
    assert_eq!(named("head"), "Head, T2, axial");
    assert_eq!(named("HEAD_NECK"), "Head neck, T2, axial");
    assert_eq!(named("  L-SPINE \t"), "L-spine, T2, axial");
    assert_eq!(
        named("KNEE\nHIP\u{1b}"),
        "Knee hip, T2, axial",
        "no control character"
    );
    assert_eq!(named("   "), "T2, axial", "blank is unknown");
    assert_eq!(named("_"), "T2, axial", "a lone underscore is unknown");
}

#[test]
fn timings_never_name_a_weighting() {
    use volsa_core::Family;
    // Times that look like a T1 weighting, a T2 one and a proton-density one, with no word
    // from the scanner saying so: each is named by its facts alone.
    for (repetition, echo) in [(500.0, 10.0), (4000.0, 100.0), (3000.0, 15.0)] {
        let mut info = mr("", Some(Axial), None, "HEAD");
        info.scan.repetition_ms = Some(repetition);
        info.scan.echo_ms = Some(echo);
        info.scan.flip_deg = Some(90.0);
        info.scan.family = vec![Family::SpinEcho];
        assert_eq!(plain_name(&info), "Head, axial scan");
    }
    // And times that look like T2 do not outweigh the scanner's own word.
    let mut info = mr("t1", Some(Axial), None, "");
    info.scan.repetition_ms = Some(4000.0);
    info.scan.echo_ms = Some(100.0);
    assert_eq!(plain_name(&info), "T1, axial");
}

#[test]
fn a_body_part_written_as_one_code_is_spelled_out() {
    // The defined terms of Body Part Examined that run words together would read as a typing
    // mistake ("Cspine") if only their case were changed; each is said as it is spelled.
    let named = |body| plain_name(&mr("t2", Some(Axial), None, body));
    let spelled = [
        ("CSPINE", "Cervical spine"),
        ("TSPINE", "Thoracic spine"),
        ("LSPINE", "Lumbar spine"),
        ("SSPINE", "Sacrum"),
        ("CTSPINE", "Cervico-thoracic spine"),
        ("TLSPINE", "Thoraco-lumbar spine"),
        ("LSSPINE", "Lumbo-sacral spine"),
        ("HEADNECK", "Head and neck"),
        ("CHESTABDOMEN", "Chest and abdomen"),
        ("ABDOMENPELVIS", "Abdomen and pelvis"),
        ("CHESTABDPELVIS", "Chest, abdomen and pelvis"),
        ("WHOLEBODY", "Entire body"),
    ];
    for (written, said) in spelled {
        assert_eq!(named(written), format!("{said}, T2, axial"), "{written}");
    }
    // Whatever the case or the spaces around it, and in a name without a weighting.
    assert_eq!(named(" cSpine "), "Cervical spine, T2, axial");
    let facts = plain_name(&mr("zzqq", Some(Axial), None, "CSPINE"));
    assert_eq!(facts, "Cervical spine, axial scan");
}

#[test]
fn an_unknown_body_part_is_as_written() {
    let named = |body| plain_name(&mr("t2", Some(Axial), None, body));
    assert_eq!(named("ZSPINE"), "Zspine, T2, axial");
    assert_eq!(named("CSPINE EXTRA"), "Cspine extra, T2, axial");
    assert_eq!(named("HEAD_NECK"), "Head neck, T2, axial");
}

#[test]
fn a_star_weighting_is_a_t2_weighting_when_two_are_written() {
    // T1 and T2* are two contrasts, as T1 and T2 are: the description contradicts itself.
    assert_eq!(axial("t1 t2*"), "Axial scan");
    assert_eq!(axial("t2* flair t1"), "Axial scan");
    // T2 and T2* are one contrast, and are both said.
    assert_eq!(axial("t2 t2*"), "T2 T2*, axial");
    assert_eq!(axial("t2*"), "T2*, axial");
}
