//! What a plugin is run on: the wrong modality refused, a contrast the description contradicts
//! warned of, and a contrast it does not say noted.

use super::{Decision, contrasts_named, decide, spacing_note};
use crate::{Accepts, Contrast, Contrasts};

/// A plugin that accepts MR scans of these contrasts.
fn mr(contrasts: Contrasts) -> Accepts {
    Accepts {
        modality: vec!["MR".to_owned()],
        contrast: contrasts,
        body_part: String::new(),
        thickest_mm: None,
    }
}

/// An MR plugin for T1 only.
fn t1() -> Accepts {
    mr(Contrasts::Only(vec![Contrast::T1]))
}

#[test]
fn a_ct_scan_is_refused_by_an_mr_plugin_in_words() {
    let refused = decide("CT", "Head CT", &t1());
    let words = "This plugin is for MR scans; this scan is CT.".to_owned();
    assert_eq!(refused, Decision::Refuse(words));
    let both = Accepts {
        modality: vec!["MR".to_owned(), "PT".to_owned()],
        ..t1()
    };
    let words = "This plugin is for MR or PT scans; this scan is CT.".to_owned();
    assert_eq!(decide(" ct ", "", &both), Decision::Refuse(words));
}

#[test]
fn a_scan_with_no_modality_is_refused() {
    let words = "This plugin is for MR scans; this scan does not say what kind it is.".to_owned();
    assert_eq!(decide("  ", "T1 MPRAGE", &t1()), Decision::Refuse(words));
}

#[test]
fn t2_flair_matches_a_flair_plugin_and_a_t2_plugin() {
    let flair = mr(Contrasts::Only(vec![Contrast::Flair]));
    let note = "The scan's description says FLAIR, as this plugin expects.".to_owned();
    assert_eq!(
        decide("MR", "AX T2 FLAIR", &flair),
        Decision::Run { note: Some(note) }
    );
    let t2 = mr(Contrasts::Only(vec![Contrast::T2]));
    let note = "The scan's description says T2, as this plugin expects.".to_owned();
    assert_eq!(
        decide("MR", "AX T2 FLAIR", &t2),
        Decision::Run { note: Some(note) }
    );
}

#[test]
fn mprage_matches_a_t1_plugin() {
    let note = "The scan's description says 3D T1, as this plugin expects.".to_owned();
    assert_eq!(
        decide("MR", "sag MP-RAGE", &t1()),
        Decision::Run { note: Some(note) }
    );
    assert_eq!(contrasts_named("T1 MPRAGE"), [Contrast::T1In3d]);
}

#[test]
fn a_t2_scan_warns_a_t1_plugin_with_the_exact_words() {
    let warned = decide("MR", "Ax T2 TSE", &t1());
    let expected = Decision::Warn {
        warning: "This plugin expects T1; this scan's description says T2.".to_owned(),
        stamp: "Run after a warning: this plugin expects T1; this scan's description says T2."
            .to_owned(),
    };
    assert_eq!(warned, expected);
}

#[test]
fn diffusion_warns_a_t1_plugin() {
    let Decision::Warn { warning, .. } = decide("MR", "DWI b1000 ADC", &t1()) else {
        panic!("a warning");
    };
    assert_eq!(
        warning,
        "This plugin expects T1; this scan's description says Diffusion and ADC map."
    );
}

#[test]
fn t1_and_t2_together_are_not_said() {
    let unsaid = "Contrast not confirmed: the scan's description does not say.".to_owned();
    assert_eq!(
        decide("MR", "T1 T2 combo", &t1()),
        Decision::Run { note: Some(unsaid) }
    );
}

#[test]
fn no_contrast_word_is_not_said() {
    let unsaid = "Contrast not confirmed: the scan's description does not say.".to_owned();
    assert_eq!(
        decide("MR", "Series 7", &t1()),
        Decision::Run { note: Some(unsaid) }
    );
}

#[test]
fn any_contrast_runs_without_a_note() {
    let any = mr(Contrasts::Any);
    assert_eq!(
        decide("MR", "Ax T2 TSE", &any),
        Decision::Run { note: None }
    );
}

#[test]
fn a_thick_scan_gets_the_spacing_note() {
    let note = "The voxels of this scan are 5 mm apart; this plugin states it was made for up to \
                1.5 mm.";
    assert_eq!(
        spacing_note([0.5, 0.5, 5.0], Some(1.5)).as_deref(),
        Some(note)
    );
    assert_eq!(spacing_note([1.0, 1.0, 1.2], Some(1.5)), None);
    assert_eq!(spacing_note([1.0, 1.0, 6.0], None), None);
}
