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

fn named(code: &str) -> Unit<'_> {
    Unit::Named(code)
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
fn each_ct_window_is_what_the_window_constructor_would_make_of_it() {
    // The table sets its fields directly, which skips the clamp of `WindowLevel::new`.
    for window in CT_WINDOWS {
        let WindowLevel { center, width } = window.wl;
        assert_eq!(
            window.wl,
            WindowLevel::new(center, width),
            "{}",
            window.name
        );
    }
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
fn an_inverted_series_has_no_unit_whatever_its_files_say() {
    // The values of a MONOCHROME1 series are flipped when decoded, so they are not the file's.
    let inverted = |info: SeriesInfo| SeriesInfo {
        inverted: true,
        ..info
    };
    assert_eq!(unit_of(&inverted(ct(None))), Unit::None);
    assert_eq!(unit_of(&inverted(ct(Some("HU")))), Unit::None);
    assert_eq!(unit_of(&inverted(ct(Some("MGML")))), Unit::None);
    assert_eq!(unit_of(&inverted(mr(Some("OD")))), Unit::None);
    assert_eq!(
        unit_of(&ct(None)),
        Unit::Hu,
        "the same series, not inverted"
    );
}

#[test]
fn a_rescale_type_of_blanks_says_nothing() {
    assert_eq!(unit_of(&ct(Some(" "))), Unit::Hu);
    assert_eq!(unit_of(&mr(Some(""))), Unit::None);
}

/// What the readout says of a value that is a number.
fn say(value: f32, unit: &Unit<'_>) -> String {
    readout(value, unit).expect("a number has a readout")
}

#[test]
fn readouts_say_their_unit() {
    assert_eq!(say(-12.4, &Unit::Hu), "-12 HU");
    assert_eq!(say(312.0, &Unit::None), "312 (no unit)");
    assert_eq!(say(0.4, &named("OD")), "0.4 OD");
}

#[test]
fn hu_are_whole_numbers_and_never_minus_nothing() {
    assert_eq!(say(40.6, &Unit::Hu), "41 HU");
    assert_eq!(say(-1000.0, &Unit::Hu), "-1000 HU");
    assert_eq!(say(0.4, &Unit::Hu), "0 HU");
    assert_eq!(say(-0.4, &Unit::Hu), "0 HU");
}

#[test]
fn a_whole_value_without_hu_has_no_decimal_point() {
    assert_eq!(say(312.0, &Unit::None), "312 (no unit)");
    assert_eq!(say(312.0, &named("MGML")), "312 MGML");
    assert_eq!(say(0.0, &Unit::None), "0 (no unit)");
    assert_eq!(say(-0.0, &Unit::None), "0 (no unit)");
}

#[test]
fn a_fraction_has_no_trailing_zeros() {
    assert_eq!(say(0.4, &Unit::None), "0.4 (no unit)");
    assert_eq!(say(2.5, &Unit::None), "2.5 (no unit)");
    assert_eq!(say(-0.4, &named("OD")), "-0.4 OD");
}

#[test]
fn digits_left_of_the_point_are_never_rounded_away() {
    assert_eq!(say(1234.5, &Unit::None), "1235 (no unit)");
    assert_eq!(say(-1234.5, &Unit::None), "-1235 (no unit)");
    assert_eq!(say(100.4, &Unit::None), "100 (no unit)");
    assert_eq!(say(65535.0, &Unit::None), "65535 (no unit)");
}

#[test]
fn a_fraction_has_three_significant_figures() {
    assert_eq!(say(0.04567, &Unit::None), "0.0457 (no unit)");
    assert_eq!(say(5.678, &Unit::None), "5.68 (no unit)");
    assert_eq!(say(12.345, &Unit::None), "12.3 (no unit)");
    assert_eq!(say(-0.001_234, &named("OD")), "-0.00123 OD");
}

#[test]
fn a_value_that_is_not_a_number_has_no_readout_at_all() {
    // Not "NaN HU": a density that is not one is not read out.
    for unit in [Unit::Hu, Unit::None, named("OD")] {
        assert_eq!(readout(f32::NAN, &unit), None);
        assert_eq!(readout(-f32::NAN, &unit), None);
    }
}

#[test]
fn an_infinite_value_is_shown_as_the_file_has_it() {
    assert_eq!(say(f32::INFINITY, &Unit::None), "inf (no unit)");
    assert_eq!(say(f32::NEG_INFINITY, &named("OD")), "-inf OD");
    assert_eq!(say(f32::INFINITY, &Unit::Hu), "inf HU");
}

#[test]
fn a_very_small_value_keeps_three_figures_and_at_most_eight_places() {
    // 0.000001 is the smallest written out, with its six places.
    assert_eq!(say(0.000_001_5, &Unit::None), "0.0000015 (no unit)");
    assert_eq!(say(0.000_005_678, &Unit::None), "0.00000568 (no unit)");
    // Smaller still, and it takes the form with an exponent.
    assert_eq!(say(1.0e-9, &Unit::None), "1e-9 (no unit)");
    assert_eq!(say(-4.567e-7, &named("OD")), "-4.57e-7 OD");
    let tiny = say(f32::MIN_POSITIVE, &Unit::None);
    assert_eq!(tiny, "1.18e-38 (no unit)");
}

#[test]
fn a_very_large_value_takes_the_form_with_an_exponent_too() {
    assert_eq!(say(999_999_999.0, &Unit::None), "1e9 (no unit)");
    assert_eq!(say(123_456.0, &Unit::None), "123456 (no unit)");
    assert_eq!(say(-3.4e38, &named("OD")), "-3.4e38 OD");
    assert_eq!(say(2.0e12, &Unit::Hu), "2e12 HU");
}

#[test]
fn no_readout_runs_to_a_long_string() {
    let values = [
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        1.0e-45,
        1.234_567e-8,
        -9.87654e-6,
        0.123_456_78,
        12_345_678.9,
        999_999_999.0,
        1.0e9,
    ];
    let units = [Unit::Hu, Unit::None, named("MGML")];
    for value in values {
        for unit in &units {
            let text = say(value, unit);
            let number = text.split(' ').next().unwrap_or_default();
            assert!(number.len() <= 12, "{value:e}: {text}");
        }
    }
}
