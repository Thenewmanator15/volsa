//! A profile for each scan, chosen from its modality alone: what the View tab offers and what
//! the value under the pointer says. A profile changes nothing in how a scan is read or drawn.
//!
//! A value is shown in Hounsfield units only for a CT scan whose files say so or say nothing
//! of their unit. Anything else is shown with the unit its files name, or with none. A derived
//! or secondary CT can hold values that are not Hounsfield units, and a density read from them
//! would be false.
//!
//! The value under the pointer is worked out as the pointer moves, so nothing here allocates
//! to find a profile or a unit.

use voxa_core::{SeriesInfo, WindowLevel};

/// The kind of scan, which decides the brightness settings offered for it and the unit its
/// values are shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Any scan that is not CT or MR: brightness is automatic and values have no unit.
    Basic,
    /// A magnetic resonance scan.
    Mr,
    /// A computed tomography scan.
    Ct,
}

impl Profile {
    /// The profile of a scan, from its modality alone, ignoring case and padding: `CT` is CT,
    /// `MR` is MR, and every other modality, or none, is basic.
    pub fn of(info: &SeriesInfo) -> Self {
        let modality = info.modality.trim();
        if modality.eq_ignore_ascii_case("CT") {
            Self::Ct
        } else if modality.eq_ignore_ascii_case("MR") {
            Self::Mr
        } else {
            Self::Basic
        }
    }
}

/// A brightness setting for CT with a name a person knows it by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamedWindow {
    /// What the setting is for, as its button says.
    pub name: &'static str,
    /// The window and level, in Hounsfield units.
    pub wl: WindowLevel,
}

/// A named window from its level and width, in the order `WindowLevel::new` takes them. That
/// function is not `const`, so the fields are set directly; the values given are within the
/// limits it would enforce, which a test holds each of them to.
const fn window(name: &'static str, center: f32, width: f32) -> NamedWindow {
    NamedWindow {
        name,
        wl: WindowLevel { center, width },
    }
}

/// The windows offered for CT, in the order they are shown: the common teaching values for
/// brain (width 80, level 40), soft tissue (400, 40), lung (1500, −600) and bone (1800, 400).
pub const CT_WINDOWS: [NamedWindow; 4] = [
    window("Brain", 40.0, 80.0),
    window("Soft tissue", 40.0, 400.0),
    window("Lung", -600.0, 1500.0),
    window("Bone", 400.0, 1800.0),
];

/// What a scan's values are measured in, as far as its files say. It borrows the unit's code
/// from the scan's facts, so finding it costs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit<'a> {
    /// Hounsfield units: a CT scan whose files say so or say nothing.
    Hu,
    /// No unit: the files do not name one, or say it is unspecified.
    None,
    /// A unit the files name, by its DICOM code, such as `MGML`.
    Named(&'a str),
}

/// The unit of a scan's values.
///
/// A CT scan whose Rescale Type is `HU`, or is not given, is in [`Unit::Hu`]. A scan with no
/// Rescale Type, or `US` (DICOM's "unspecified"), is in [`Unit::None`]. Any other Rescale Type
/// is named. So a CT scan that says it is not in Hounsfield units is not shown as if it were,
/// and nor is a scan of another kind that merely says `HU`. A scan stored as `MONOCHROME1` is
/// in no unit at all: its values are flipped when decoded, so they are not the file's own.
pub fn unit_of(info: &SeriesInfo) -> Unit<'_> {
    if info.inverted {
        return Unit::None;
    }
    let code = info
        .rescale_type
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty());
    match (Profile::of(info), code) {
        (Profile::Ct, None | Some("HU")) => Unit::Hu,
        (_, None | Some("US")) => Unit::None,
        (_, Some(code)) => Unit::Named(code),
    }
}

/// The least a value may be, in size, to be written out in full rather than with an exponent.
/// A whole number beyond it would run to many digits, and a float cannot hold them anyway.
const LARGEST_WRITTEN_OUT: f32 = 1.0e9;
/// The most places after the decimal point a value is written with.
const MOST_PLACES: usize = 8;

/// A value as the readout under the pointer says it, such as `-12 HU`, `312 (no unit)` or
/// `0.4 OD`. `None` for a value that is not a number: there is nothing to read out.
///
/// Hounsfield units are shown as whole numbers. Any other value is shown to three significant
/// figures when it has a fraction and without trailing zeros, but never loses a digit left of
/// the decimal point: 0.04567 is `0.0457`, and 1234.5 is `1235`. A value that would need more
/// than eight places, or a billion or more, is written with an exponent, such as `1.18e-38`,
/// so that a readout is never a long string.
pub fn readout(value: f32, unit: &Unit<'_>) -> Option<String> {
    if value.is_nan() {
        return None;
    }
    Some(match unit {
        Unit::Hu => format!("{} HU", hounsfield(value)),
        Unit::None => format!("{} (no unit)", plain(value)),
        Unit::Named(code) => format!("{} {code}", plain(value)),
    })
}

/// A value of Hounsfield units: a whole number.
fn hounsfield(value: f32) -> String {
    if value.abs() >= LARGEST_WRITTEN_OUT {
        exponent(value)
    } else {
        whole(value)
    }
}

/// A value rounded to a whole number, with no sign on a zero.
fn whole(value: f32) -> String {
    let rounded = value.round();
    if rounded == 0.0 {
        "0".to_owned()
    } else {
        rounded.to_string()
    }
}

/// A value to three significant figures, but to a whole number from 100 up, without trailing
/// zeros.
fn plain(value: f32) -> String {
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return "0".to_owned();
    }
    if magnitude >= LARGEST_WRITTEN_OUT {
        return exponent(value);
    }
    let decimals = decimals_for(magnitude);
    if decimals == 0 {
        // `round` takes a half away from zero, where the formatter would take it to the even
        // number, and 1234.5 is 1235.
        return whole(value);
    }
    if decimals > MOST_PLACES {
        return exponent(value);
    }
    let text = format!("{value:.decimals$}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A value to three significant figures with an exponent, without trailing zeros: `1.18e-38`,
/// `-4.57e-7`, `1e9`. An infinite value is shown as it is written, `inf`.
fn exponent(value: f32) -> String {
    let text = format!("{value:.2e}");
    match text.split_once('e') {
        Some((figures, power)) => {
            let figures = figures.trim_end_matches('0').trim_end_matches('.');
            format!("{figures}e{power}")
        }
        None => text,
    }
}

/// How many places after the decimal point give a value of this size three significant
/// figures: none from 100, one from 10, two from 1, and one more for each tenth below that.
fn decimals_for(magnitude: f32) -> usize {
    if magnitude >= 100.0 {
        return 0;
    }
    if magnitude >= 10.0 {
        return 1;
    }
    let mut decimals = 2;
    let mut bound = 1.0_f32;
    // The bound shrinks to nothing in the end, so this stops for any value above zero.
    while magnitude < bound {
        bound /= 10.0;
        decimals += 1;
    }
    decimals
}

#[cfg(test)]
mod tests;
