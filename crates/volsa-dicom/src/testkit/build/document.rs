//! Writing made-up reports and PDFs.

use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{Element, element, sequence, write_labelled};
use crate::testkit::{DocumentOptions, EXPLICIT_VR_LITTLE_ENDIAN, ReportItem};

const BASIC_TEXT_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.11";
const ENCAPSULATED_PDF_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.1";

/// One structured report, holding `content` under a title.
pub fn structured_report(opts: &DocumentOptions, content: &[ReportItem]) -> Vec<u8> {
    let class = opts.sop_class.as_deref().unwrap_or(BASIC_TEXT_SR_STORAGE);
    let mut obj = common_elements(opts, class, "SR");
    obj.put(element(tags::VALUE_TYPE, VR::CS, "CONTAINER"));
    obj.put(element(tags::CONTINUITY_OF_CONTENT, VR::CS, "SEPARATE"));
    if !opts.title.is_empty() {
        let name = vec![code(&opts.title)];
        obj.put(sequence(tags::CONCEPT_NAME_CODE_SEQUENCE, name));
    }
    if !content.is_empty() {
        obj.put(content_sequence(content));
    }
    write_labelled(obj, EXPLICIT_VR_LITTLE_ENDIAN)
}

/// One DICOM file holding `pdf`, padded to an even length as the standard says.
pub fn encapsulated_pdf(opts: &DocumentOptions, pdf: &[u8]) -> Vec<u8> {
    let class = opts
        .sop_class
        .as_deref()
        .unwrap_or(ENCAPSULATED_PDF_STORAGE);
    let mut obj = common_elements(opts, class, "DOC");
    obj.put(element(tags::CONVERSION_TYPE, VR::CS, "WSD"));
    if !opts.title.is_empty() {
        obj.put(element(tags::DOCUMENT_TITLE, VR::ST, opts.title.as_str()));
    }
    let mime = tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT;
    obj.put(element(mime, VR::LO, "application/pdf"));
    // The length of the document without the pad, as the standard has a file give it.
    let length = u32::try_from(pdf.len()).expect("a made-up page is short");
    let length = PrimitiveValue::from(length);
    obj.put(DataElement::new(
        tags::ENCAPSULATED_DOCUMENT_LENGTH,
        VR::UL,
        length,
    ));
    let mut padded = pdf.to_vec();
    if padded.len() % 2 == 1 {
        padded.push(0);
    }
    let document = PrimitiveValue::U8(padded.into());
    obj.put(DataElement::new(
        tags::ENCAPSULATED_DOCUMENT,
        VR::OB,
        document,
    ));
    write_labelled(obj, EXPLICIT_VR_LITTLE_ENDIAN)
}

/// The elements every made-up document has, none of them about an image.
pub(super) fn common_elements(
    opts: &DocumentOptions,
    sop_class: &str,
    modality: &str,
) -> InMemDicomObject {
    let sop_instance = format!("{}.{}", opts.series_uid, opts.instance_number);
    let instance_number = opts.instance_number.to_string();
    let text = [
        (tags::SOP_CLASS_UID, VR::UI, sop_class),
        (tags::SOP_INSTANCE_UID, VR::UI, sop_instance.as_str()),
        (tags::MODALITY, VR::CS, modality),
        (tags::PATIENT_NAME, VR::PN, opts.patient_name.as_str()),
        (tags::PATIENT_ID, VR::LO, opts.patient_id.as_str()),
        (tags::STUDY_INSTANCE_UID, VR::UI, opts.study_uid.as_str()),
        (tags::SERIES_INSTANCE_UID, VR::UI, opts.series_uid.as_str()),
        (tags::SERIES_NUMBER, VR::IS, "900"),
        (tags::INSTANCE_NUMBER, VR::IS, instance_number.as_str()),
    ];
    let mut obj = InMemDicomObject::new_empty();
    for (tag, vr, value) in text {
        obj.put(element(tag, vr, value));
    }
    if let Some(date) = &opts.study_date {
        obj.put(element(tags::STUDY_DATE, VR::DA, date.as_str()));
    }
    if let Some(date) = &opts.content_date {
        obj.put(element(tags::CONTENT_DATE, VR::DA, date.as_str()));
    }
    if !opts.series_description.is_empty() {
        let said = opts.series_description.as_str();
        obj.put(element(tags::SERIES_DESCRIPTION, VR::LO, said));
    }
    obj
}

/// A code, as an item of a sequence, whose meaning is `meaning`.
fn code(meaning: &str) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    item.put(element(tags::CODE_VALUE, VR::SH, "MADE-UP"));
    item.put(element(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "99MADEUP"));
    item.put(element(tags::CODE_MEANING, VR::LO, meaning));
    item
}

fn content_sequence(items: &[ReportItem]) -> Element {
    sequence(
        tags::CONTENT_SEQUENCE,
        items.iter().map(content_item).collect(),
    )
}

/// One item of a report's content.
fn content_item(item: &ReportItem) -> InMemDicomObject {
    let mut obj = InMemDicomObject::new_empty();
    obj.put(element(tags::RELATIONSHIP_TYPE, VR::CS, "CONTAINS"));
    match item {
        ReportItem::Container { concept, items } => {
            named(&mut obj, "CONTAINER", concept);
            obj.put(element(tags::CONTINUITY_OF_CONTENT, VR::CS, "SEPARATE"));
            if !items.is_empty() {
                obj.put(content_sequence(items));
            }
        }
        ReportItem::Plain {
            value_type,
            concept,
            value,
        } => {
            named(&mut obj, value_type, concept);
            let (tag, vr) = match value_type.as_str() {
                "TEXT" => (tags::TEXT_VALUE, VR::UT),
                "DATE" => (tags::DATE, VR::DA),
                "TIME" => (tags::TIME, VR::TM),
                "DATETIME" => (tags::DATE_TIME, VR::DT),
                "PNAME" => (tags::PERSON_NAME, VR::PN),
                _ => (tags::UID, VR::UI),
            };
            obj.put(element(tag, vr, value.as_str()));
        }
        ReportItem::Number {
            concept,
            value,
            units,
        } => {
            named(&mut obj, "NUM", concept);
            let mut measured = InMemDicomObject::new_empty();
            let unit = sequence(tags::MEASUREMENT_UNITS_CODE_SEQUENCE, vec![code(units)]);
            measured.put(unit);
            measured.put(element(tags::NUMERIC_VALUE, VR::DS, value.as_str()));
            obj.put(sequence(tags::MEASURED_VALUE_SEQUENCE, vec![measured]));
        }
        ReportItem::Code { concept, meaning } => {
            named(&mut obj, "CODE", concept);
            let value = sequence(tags::CONCEPT_CODE_SEQUENCE, vec![code(meaning)]);
            obj.put(value);
        }
        ReportItem::Unshown {
            value_type,
            concept,
        } => named(&mut obj, value_type, concept),
    }
    obj
}

/// Say what kind of item `obj` is and what it is of.
fn named(obj: &mut InMemDicomObject, value_type: &str, concept: &str) {
    obj.put(element(tags::VALUE_TYPE, VR::CS, value_type));
    obj.put(sequence(
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        vec![code(concept)],
    ));
}
