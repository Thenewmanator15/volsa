//! What a file says about how its series was acquired.

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use voxa_core::{Family, Plane, ScanFacts};

use super::{first_item, floats, text};

/// The facts of acquisition in one file's header. A single-frame file states them directly;
/// a file of many frames keeps some in a functional group, shared by every frame or repeated
/// for each, in which case the first frame's is taken.
pub(super) fn facts(obj: &InMemDicomObject, orientation: Option<[f64; 6]>) -> ScanFacts {
    let acquired = text(obj, tags::MR_ACQUISITION_TYPE);
    ScanFacts {
        plane: orientation.and_then(Plane::of),
        in_3d: acquired.and_then(|said| match said.as_str() {
            "3D" => Some(true),
            "2D" => Some(false),
            _ => None,
        }),
        family: family(obj),
        repetition_ms: number(obj, tags::REPETITION_TIME).or_else(|| {
            grouped(
                obj,
                tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
                tags::REPETITION_TIME,
            )
        }),
        echo_ms: number(obj, tags::ECHO_TIME)
            .or_else(|| grouped(obj, tags::MR_ECHO_SEQUENCE, tags::EFFECTIVE_ECHO_TIME)),
        inversion_ms: number(obj, tags::INVERSION_TIME)
            .or_else(|| grouped(obj, tags::MR_MODIFIER_SEQUENCE, tags::INVERSION_TIMES)),
        flip_deg: number(obj, tags::FLIP_ANGLE).or_else(|| {
            grouped(
                obj,
                tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
                tags::FLIP_ANGLE,
            )
        }),
        field_t: number(obj, tags::MAGNETIC_FIELD_STRENGTH),
        body_part: text(obj, tags::BODY_PART_EXAMINED).unwrap_or_default(),
    }
}

/// The first number of a value.
pub(super) fn number(obj: &InMemDicomObject, tag: Tag) -> Option<f64> {
    floats(obj, tag).and_then(|values| values.first().copied())
}

/// A number kept in a functional group: the shared one, or else the first frame's.
pub(super) fn grouped(obj: &InMemDicomObject, group: Tag, wanted: Tag) -> Option<f64> {
    let shared = first_item(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    let first_frame = first_item(obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    [shared, first_frame]
        .into_iter()
        .flatten()
        .find_map(|groups| first_item(groups, group).and_then(|item| number(item, wanted)))
}

/// The families of sequence: from the codes a single-frame file lists, or from the separate
/// statements a file of many frames makes.
fn family(obj: &InMemDicomObject) -> Vec<Family> {
    let listed = obj.get(tags::SCANNING_SEQUENCE);
    let codes = listed.and_then(|element| element.to_multi_str().ok());
    let lists = |code: &str| {
        codes
            .as_ref()
            .is_some_and(|codes| codes.iter().any(|said| said.trim() == code))
    };
    let yes = |tag: Tag| text(obj, tag).is_some_and(|said| said == "YES");
    let echo = text(obj, tags::ECHO_PULSE_SEQUENCE).unwrap_or_default();
    let made_by = |way: &str| echo == way || echo == "BOTH";
    let found = [
        (
            lists("IR") || yes(tags::INVERSION_RECOVERY),
            Family::InversionRecovery,
        ),
        (
            lists("EP") || yes(tags::ECHO_PLANAR_PULSE_SEQUENCE),
            Family::EchoPlanar,
        ),
        (lists("SE") || made_by("SPIN"), Family::SpinEcho),
        (lists("GR") || made_by("GRADIENT"), Family::GradientEcho),
    ];
    found
        .into_iter()
        .filter_map(|(is, family)| is.then_some(family))
        .collect()
}
