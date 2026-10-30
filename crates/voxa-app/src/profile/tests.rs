use super::*;

fn with_modality(modality: &str) -> SeriesInfo {
    SeriesInfo {
        modality: modality.into(),
        ..SeriesInfo::default()
    }
}

fn scan(modality: &str, rescale_type: Option<&str>) -> SeriesInfo {
    SeriesInfo {
        rescale_type: rescale_type.map(str::to_owned),
        ..with_modality(modality)
    }
}

fn ct(rescale_type: Option<&str>) -> SeriesInfo {
    scan("CT", rescale_type)
}

fn mr(rescale_type: Option<&str>) -> SeriesInfo {
    scan("MR", rescale_type)
}

fn named(code: &str) -> Unit {
    Unit::Named(code.into())
}

#[test]
fn the_profile_comes_from_the_modality() {
    assert_eq!(Profile::of(&with_modality("CT")), Profile::Ct);
    assert_eq!(Profile::of(&with_modality("mr ")), Profile::Mr);
    for other in ["US", "CR", "PT", "SC", ""] {
        assert_eq!(Profile::of(&with_modality(other)), Profile::Basic);
    }
}

#[test]
fn ct_windows_are_the_common_teaching_values() {
    let got: Vec<_> = CT_WINDOWS
        .iter()
        .map(|w| (w.name, w.wl.width, w.wl.center))
        .collect();
    assert_eq!(
        got,
        [
            ("Brain", 80.0, 40.0),
            ("Soft tissue", 400.0, 40.0),
            ("Lung", 1500.0, -600.0),
            ("Bone", 1800.0, 400.0)
        ]
    );
}

#[test]
fn hu_only_for_ct_whose_rescale_says_hu_or_nothing() {
    assert_eq!(unit_of(&ct(None)), Unit::Hu);
    assert_eq!(unit_of(&ct(Some("HU"))), Unit::Hu);
    assert_eq!(unit_of(&ct(Some("US"))), Unit::None);
    assert_eq!(unit_of(&ct(Some("MGML"))), named("MGML"));
    assert_eq!(unit_of(&mr(None)), Unit::None);
}

#[test]
fn another_profile_never_gets_hu_by_saying_nothing() {
    assert_eq!(unit_of(&with_modality("")), Unit::None);
    assert_eq!(unit_of(&with_modality("PT")), Unit::None);
    assert_eq!(unit_of(&mr(Some("US"))), Unit::None);
    assert_eq!(unit_of(&scan("SC", Some("US"))), Unit::None);
}

#[test]
fn a_file_that_names_its_unit_has_it_shown_whatever_the_modality() {
    assert_eq!(unit_of(&mr(Some("OD"))), named("OD"));
    assert_eq!(unit_of(&scan("PT", Some("MGML"))), named("MGML"));
}

#[test]
fn hu_in_a_file_that_is_not_ct_is_only_a_name() {
    assert_eq!(unit_of(&mr(Some("HU"))), named("HU"));
    assert_eq!(unit_of(&scan("", Some("HU"))), named("HU"));
}

#[test]
fn a_rescale_type_of_blanks_says_nothing() {
    assert_eq!(unit_of(&ct(Some(" "))), Unit::Hu);
    assert_eq!(unit_of(&mr(Some(""))), Unit::None);
}

#[test]
fn readouts_say_their_unit() {
    assert_eq!(readout(-12.4, &Unit::Hu), "-12 HU");
    assert_eq!(readout(312.0, &Unit::None), "312 (no unit)");
    assert_eq!(readout(0.4, &named("OD")), "0.4 OD");
}

#[test]
fn hu_are_whole_numbers_and_never_minus_nothing() {
    assert_eq!(readout(40.6, &Unit::Hu), "41 HU");
    assert_eq!(readout(-1000.0, &Unit::Hu), "-1000 HU");
    assert_eq!(readout(0.4, &Unit::Hu), "0 HU");
    assert_eq!(readout(-0.4, &Unit::Hu), "0 HU");
}

#[test]
fn a_whole_value_without_hu_has_no_decimal_point() {
    assert_eq!(readout(312.0, &Unit::None), "312 (no unit)");
    assert_eq!(readout(312.0, &named("MGML")), "312 MGML");
    assert_eq!(readout(0.0, &Unit::None), "0 (no unit)");
    assert_eq!(readout(-0.0, &Unit::None), "0 (no unit)");
}

#[test]
fn a_fraction_has_no_trailing_zeros() {
    assert_eq!(readout(0.4, &Unit::None), "0.4 (no unit)");
    assert_eq!(readout(2.5, &Unit::None), "2.5 (no unit)");
    assert_eq!(readout(-0.4, &named("OD")), "-0.4 OD");
}

#[test]
fn digits_left_of_the_point_are_never_rounded_away() {
    assert_eq!(readout(1234.5, &Unit::None), "1235 (no unit)");
    assert_eq!(readout(-1234.5, &Unit::None), "-1235 (no unit)");
    assert_eq!(readout(100.4, &Unit::None), "100 (no unit)");
    assert_eq!(readout(65535.0, &Unit::None), "65535 (no unit)");
}

#[test]
fn a_fraction_has_three_significant_figures() {
    assert_eq!(readout(0.04567, &Unit::None), "0.0457 (no unit)");
    assert_eq!(readout(5.678, &Unit::None), "5.68 (no unit)");
    assert_eq!(readout(12.345, &Unit::None), "12.3 (no unit)");
    assert_eq!(readout(-0.001_234, &named("OD")), "-0.00123 OD");
}

#[test]
fn a_value_that_is_not_a_number_is_shown_as_it_is_and_nothing_stops() {
    assert_eq!(readout(f32::NAN, &Unit::None), "NaN (no unit)");
    assert_eq!(readout(f32::INFINITY, &Unit::None), "inf (no unit)");
    let tiny = readout(f32::MIN_POSITIVE, &Unit::None);
    assert!(
        tiny.starts_with("0.0") && tiny.ends_with("118 (no unit)"),
        "{tiny}"
    );
}
