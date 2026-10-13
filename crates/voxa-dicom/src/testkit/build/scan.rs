//! Writing what a made-up file says about how it was acquired.

use dicom_core::{PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{EnhancedFrame, ScanTags, decimal, element, one, sequence};

/// The elements at the top of a file. A single-frame file lists its sequence as codes and
/// states its timings directly; a file of many frames says each thing separately and keeps
/// its timings in the shared functional groups (see [`put_shared`]).
pub(super) fn put(obj: &mut InMemDicomObject, scan: &ScanTags, many_frames: bool) {
    let said = [
        (tags::MR_ACQUISITION_TYPE, &scan.acquired),
        (tags::BODY_PART_EXAMINED, &scan.body_part),
    ];
    for (tag, value) in said {
        if !value.is_empty() {
            obj.put(element(tag, VR::CS, value.as_str()));
        }
    }
    if let Some(field) = scan.field_t {
        obj.put(decimal(tags::MAGNETIC_FIELD_STRENGTH, &[field]));
    }
    let counted = [
        (tags::TEMPORAL_POSITION_IDENTIFIER, scan.temporal),
        (tags::ACQUISITION_NUMBER, scan.acquisition),
    ];
    for (tag, value) in counted {
        if let Some(value) = value {
            obj.put(element(tag, VR::IS, value.to_string().as_str()));
        }
    }
    let codes: Vec<String> = scan.sequence.split('\\').map(str::to_owned).collect();
    if many_frames {
        put_statements(obj, &codes);
        return;
    }
    if !scan.sequence.is_empty() {
        obj.put(element(
            tags::SCANNING_SEQUENCE,
            VR::CS,
            PrimitiveValue::Strs(codes.into()),
        ));
    }
    let timed = [
        (tags::REPETITION_TIME, scan.repetition_ms),
        (tags::ECHO_TIME, scan.echo_ms),
        (tags::INVERSION_TIME, scan.inversion_ms),
        (tags::FLIP_ANGLE, scan.flip_deg),
        (tags::TRIGGER_TIME, scan.trigger_ms),
    ];
    for (tag, value) in timed {
        if let Some(value) = value {
            obj.put(decimal(tag, &[value]));
        }
    }
    if let Some(b_value) = scan.b_value {
        obj.put(element(
            tags::DIFFUSION_B_VALUE,
            VR::FD,
            PrimitiveValue::from(b_value),
        ));
    }
}

/// The sequence as a file of many frames states it: one element for each question.
fn put_statements(obj: &mut InMemDicomObject, codes: &[String]) {
    let lists = |code: &str| codes.iter().any(|said| said == code);
    let echo = match (lists("SE"), lists("GR")) {
        (true, true) => Some("BOTH"),
        (true, false) => Some("SPIN"),
        (false, true) => Some("GRADIENT"),
        (false, false) => None,
    };
    if let Some(echo) = echo {
        obj.put(element(tags::ECHO_PULSE_SEQUENCE, VR::CS, echo));
    }
    if lists("IR") {
        obj.put(element(tags::INVERSION_RECOVERY, VR::CS, "YES"));
    }
    if lists("EP") {
        obj.put(element(tags::ECHO_PLANAR_PULSE_SEQUENCE, VR::CS, "YES"));
    }
}

/// The timings of a file of many frames, in the functional groups every frame shares.
pub(super) fn put_shared(shared: &mut InMemDicomObject, scan: &ScanTags) {
    let mut timing = InMemDicomObject::new_empty();
    if let Some(repetition) = scan.repetition_ms {
        timing.put(decimal(tags::REPETITION_TIME, &[repetition]));
    }
    if let Some(flip) = scan.flip_deg {
        timing.put(decimal(tags::FLIP_ANGLE, &[flip]));
    }
    if scan.repetition_ms.is_some() || scan.flip_deg.is_some() {
        let group = tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE;
        shared.put(sequence(group, vec![timing]));
    }
    if let Some(echo) = scan.echo_ms {
        let item = one(element(
            tags::EFFECTIVE_ECHO_TIME,
            VR::FD,
            PrimitiveValue::from(echo),
        ));
        shared.put(sequence(tags::MR_ECHO_SEQUENCE, vec![item]));
    }
    if let Some(inversion) = scan.inversion_ms {
        let item = one(element(
            tags::INVERSION_TIMES,
            VR::FD,
            PrimitiveValue::from(inversion),
        ));
        shared.put(sequence(tags::MR_MODIFIER_SEQUENCE, vec![item]));
    }
}

/// What one frame says that sets it apart from others at its position, in its own groups.
pub(super) fn put_frame(groups: &mut InMemDicomObject, frame: &EnhancedFrame) {
    if let Some(b_value) = frame.b_value {
        let item = one(element(
            tags::DIFFUSION_B_VALUE,
            VR::FD,
            PrimitiveValue::from(b_value),
        ));
        groups.put(sequence(tags::MR_DIFFUSION_SEQUENCE, vec![item]));
    }
    if let Some(echo) = frame.echo_ms {
        let item = one(element(
            tags::EFFECTIVE_ECHO_TIME,
            VR::FD,
            PrimitiveValue::from(echo),
        ));
        groups.put(sequence(tags::MR_ECHO_SEQUENCE, vec![item]));
    }
    if let Some(temporal) = frame.temporal {
        let index = u32::try_from(temporal).unwrap_or(0);
        let item = one(element(
            tags::TEMPORAL_POSITION_INDEX,
            VR::UL,
            PrimitiveValue::from(index),
        ));
        groups.put(sequence(tags::FRAME_CONTENT_SEQUENCE, vec![item]));
    }
}
