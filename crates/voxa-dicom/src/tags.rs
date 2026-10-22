//! Listing the elements of one file for the tag browser.

use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry};
use dicom_core::value::Value;
use dicom_core::{DataElement, VR};
use dicom_dictionary_std::StandardDataDictionary;
use voxa_core::VoxaError;

use crate::decode::{guarded, open};
use crate::index::SliceRef;
use crate::source::SeriesSource;

/// The most characters of a value that are shown.
const MAX_VALUE_CHARS: usize = 256;

/// One element of a file, as text.
#[derive(Clone, PartialEq, Eq)]
pub struct TagRow {
    /// The element's number, as `(0010,0010)`.
    pub tag: String,
    /// Its name in the DICOM standard, as `PatientName`; empty for an element the standard
    /// does not define.
    pub keyword: String,
    /// Its value representation, as `PN`.
    pub vr: String,
    /// Its value. Several values are joined by a backslash; a long value is cut at 256
    /// characters; binary and nested values show their size instead of their contents.
    pub value: String,
}

/// List every element of the file holding `slice`, the file header first, then the data set,
/// each in tag order. The rows include the patient's details: see [`is_patient_identifying`].
///
/// # Errors
///
/// [`VoxaError::NotDicom`] when the entry is not a DICOM file,
/// [`VoxaError::UnsupportedTransferSyntax`] or [`VoxaError::Decode`] when it cannot be read
/// through, and the source's own error when the entry cannot be read from it.
pub fn list_tags(
    source: &mut dyn SeriesSource,
    slice: &SliceRef,
) -> Result<Vec<TagRow>, VoxaError> {
    let bytes = source.read(&slice.entry)?;
    guarded(|| {
        let file = open(&bytes)?;
        let mut rows: Vec<TagRow> = file
            .meta()
            .to_element_iter()
            .map(|element| row(&element))
            .collect();
        rows.extend(file.iter().map(row));
        Ok(rows)
    })
}

impl std::fmt::Debug for TagRow {
    /// Prints the row with the value of an element that could identify the patient replaced, so
    /// that rows can appear in a log or a failed test without naming anyone.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let private = is_patient_identifying(&self.tag);
        let value = if private {
            "<hidden>"
        } else {
            self.value.as_str()
        };
        f.debug_struct("TagRow")
            .field("tag", &self.tag)
            .field("keyword", &self.keyword)
            .field("vr", &self.vr)
            .field("value", &value)
            .finish()
    }
}

/// Elements outside the patient's group that could lead to the patient: the people who
/// ordered, made and read the scan, the place and the machine it was made on, and the numbers
/// of the order and of the visit it belongs to.
const LEADS_TO_THE_PATIENT: [&str; 22] = [
    "(0008,0050)", // AccessionNumber
    "(0008,0080)", // InstitutionName
    "(0008,0081)", // InstitutionAddress
    "(0008,0090)", // ReferringPhysicianName
    "(0008,0092)", // ReferringPhysicianAddress
    "(0008,0094)", // ReferringPhysicianTelephoneNumbers
    "(0008,1010)", // StationName
    "(0008,1040)", // InstitutionalDepartmentName
    "(0008,1048)", // PhysiciansOfRecord
    "(0008,1050)", // PerformingPhysicianName
    "(0008,1060)", // NameOfPhysiciansReadingStudy
    "(0008,1070)", // OperatorsName
    "(0018,1000)", // DeviceSerialNumber
    "(0032,1032)", // RequestingPhysician
    "(0038,0010)", // AdmissionID
    "(0040,0006)", // ScheduledPerformingPhysicianName
    "(0040,0009)", // ScheduledProcedureStepID
    "(0040,0242)", // PerformedStationName
    "(0040,0243)", // PerformedLocation
    "(0040,0253)", // PerformedProcedureStepID
    "(0040,1001)", // RequestedProcedureID
    "(0040,2016)", // PlacerOrderNumberImagingServiceRequest
];

/// Whether the value of a tag written as `(gggg,eeee)` is kept from view while the patient's
/// details are: every element of the patient's group, 0010; the elements elsewhere that could
/// lead to the patient (the accession number, the institution, the physicians and operators,
/// the station and the device's serial number, and the numbers of the order and the visit);
/// and every private element, which is any in a group of odd number, since what a maker keeps
/// there is not said.
pub fn is_patient_identifying(tag: &str) -> bool {
    let group = tag
        .strip_prefix('(')
        .and_then(|rest| rest.get(..4))
        .and_then(|digits| u16::from_str_radix(digits, 16).ok());
    let private = group.is_some_and(|group| group % 2 == 1);
    tag.starts_with("(0010,") || private || LEADS_TO_THE_PATIENT.contains(&tag)
}

fn row<I, P>(element: &DataElement<I, P>) -> TagRow {
    let tag = element.header().tag;
    let keyword = StandardDataDictionary
        .by_tag(tag)
        .map(|entry| entry.alias().to_owned());
    TagRow {
        tag: tag.to_string(),
        keyword: keyword.unwrap_or_default(),
        vr: element.vr().to_string().to_owned(),
        value: shown(element.value(), element.vr()),
    }
}

fn shown<I, P>(value: &Value<I, P>, vr: VR) -> String {
    let binary = matches!(
        vr,
        VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN
    );
    match value {
        Value::Sequence(sequence) => counted(sequence.items().len(), "item"),
        Value::PixelSequence(sequence) => counted(sequence.fragments().len(), "fragment"),
        Value::Primitive(primitive) if binary => counted(primitive.calculate_byte_len(), "byte"),
        Value::Primitive(primitive) => shortened(primitive.to_str().trim_end_matches(['\0', ' '])),
    }
}

/// A size in place of contents, as `<24 items>`.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("<1 {noun}>")
    } else {
        format!("<{count} {noun}s>")
    }
}

/// The text, cut at [`MAX_VALUE_CHARS`] characters with a mark that more follows.
fn shortened(text: &str) -> String {
    let mut shown: String = text.chars().take(MAX_VALUE_CHARS).collect();
    if text.chars().nth(MAX_VALUE_CHARS).is_some() {
        shown.push('…');
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SliceRef;
    use crate::testkit::{
        EnhancedFrame, EnhancedOptions, MemorySource, SliceOptions, classic_slice, enhanced_series,
    };
    use voxa_core::VoxaError;

    const NAME: &str = "slice.dcm";

    fn rows_of(bytes: Vec<u8>) -> Vec<TagRow> {
        let mut source = MemorySource::new(vec![(NAME.to_owned(), bytes)]);
        let slice = SliceRef {
            entry: NAME.to_owned(),
            frame: None,
        };
        list_tags(&mut source, &slice).expect("lists")
    }

    fn row<'a>(rows: &'a [TagRow], tag: &str) -> &'a TagRow {
        rows.iter()
            .find(|row| row.tag == tag)
            .unwrap_or_else(|| panic!("no row for {tag}"))
    }

    #[test]
    fn list_tags_gives_each_element_its_number_name_type_and_value() {
        let rows = rows_of(classic_slice(&SliceOptions::default()));
        let name = row(&rows, "(0010,0010)");
        assert_eq!(
            (name.keyword.as_str(), name.vr.as_str(), name.value.as_str()),
            ("PatientName", "PN", "Phantom^Test")
        );
        let modality = row(&rows, "(0008,0060)");
        assert_eq!(
            (
                modality.keyword.as_str(),
                modality.vr.as_str(),
                modality.value.as_str()
            ),
            ("Modality", "CS", "MR")
        );
        assert_eq!(
            row(&rows, "(0028,0010)").value,
            "4",
            "numbers are written out"
        );
    }

    #[test]
    fn list_tags_is_in_tag_order_starting_with_the_file_header() {
        let rows = rows_of(classic_slice(&SliceOptions::default()));
        let tags: Vec<&str> = rows.iter().map(|row| row.tag.as_str()).collect();
        let mut sorted = tags.clone();
        sorted.sort_unstable();
        assert_eq!(tags, sorted);
        let syntax = row(&rows, "(0002,0010)");
        assert_eq!(syntax.keyword, "TransferSyntaxUID");
        assert_eq!(syntax.value, "1.2.840.10008.1.2.1");
    }

    #[test]
    fn list_tags_joins_several_values_with_a_backslash() {
        let rows = rows_of(classic_slice(&SliceOptions::default()));
        assert_eq!(row(&rows, "(0020,0032)").value, "-100\\-120\\0");
    }

    #[test]
    fn list_tags_gives_the_size_of_binary_and_nested_values_not_their_contents() {
        let rows = rows_of(classic_slice(&SliceOptions::default()));
        assert_eq!(row(&rows, "(7FE0,0010)").value, "<32 bytes>");

        let frames = (0..24)
            .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i)]))
            .collect();
        let enhanced = EnhancedOptions {
            common: SliceOptions::default(),
            frames,
        };
        let rows = rows_of(enhanced_series(&enhanced));
        assert_eq!(row(&rows, "(5200,9230)").value, "<24 items>");
        assert_eq!(row(&rows, "(5200,9229)").value, "<1 item>");
    }

    #[test]
    fn list_tags_cuts_a_long_value_at_256_characters() {
        let long = SliceOptions {
            series_description: "é".repeat(300),
            ..SliceOptions::default()
        };
        let rows = rows_of(classic_slice(&long));
        let shown = &row(&rows, "(0008,103E)").value;
        assert_eq!(
            shown.chars().count(),
            257,
            "256 characters and a mark that more follows"
        );
        assert!(shown.starts_with("ééé") && shown.ends_with('…'));
    }

    #[test]
    fn list_tags_of_something_that_is_not_dicom_says_so() {
        let mut source = MemorySource::new(vec![(NAME.to_owned(), b"%PDF-1.7".to_vec())]);
        let slice = SliceRef {
            entry: NAME.to_owned(),
            frame: None,
        };
        let result = list_tags(&mut source, &slice);
        assert!(matches!(result, Err(VoxaError::NotDicom)), "{result:?}");
    }

    #[test]
    fn printing_rows_does_not_show_what_describes_the_patient() {
        let rows = rows_of(classic_slice(&SliceOptions::default()));
        let printed = format!("{rows:?}");
        assert!(
            !printed.contains("Phantom") && !printed.contains("PHANTOM-001"),
            "{printed}"
        );
        assert!(printed.contains("Modality") && printed.contains("\"MR\""));
        assert!(printed.contains("PatientName") && printed.contains("<hidden>"));
    }

    #[test]
    fn patient_tags_are_known_by_their_group() {
        assert!(is_patient_identifying("(0010,0010)"));
        assert!(is_patient_identifying("(0010,0020)"));
        assert!(is_patient_identifying("(0010,21B0)"));
        assert!(!is_patient_identifying("(0008,0060)"));
        assert!(!is_patient_identifying("(0020,0010)"));
        assert!(!is_patient_identifying("0010"));
    }

    #[test]
    fn what_could_lead_to_the_patient_is_known_too() {
        let leads = [
            "(0008,0050)", // the accession number
            "(0008,0080)", // the institution's name
            "(0008,0081)", // and its address
            "(0008,0090)", // the referring physician
            "(0008,1050)", // the performing physician
            "(0008,1070)", // the operators
            "(0008,1010)", // the station's name
            "(0018,1000)", // the device's serial number
            "(0009,0010)", // a private creator, in an odd group
            "(0019,100C)", // a private element
            "(0029,1010)",
            "(7FE1,0010)",
        ];
        for tag in leads {
            assert!(is_patient_identifying(tag), "{tag}");
        }
        let plain = [
            "(0002,0010)", // the transfer syntax
            "(0008,0020)", // the study's date, shown either way
            "(0008,0060)", // the modality
            "(0008,1030)", // the study's description
            "(0018,0087)", // the field strength
            "(0028,0010)", // the rows
            "(7FE0,0010)", // the pixel data
        ];
        for tag in plain {
            assert!(!is_patient_identifying(tag), "{tag}");
        }
        assert!(!is_patient_identifying("(00G9,0010)"), "not a number");
    }

    #[test]
    fn printing_rows_does_not_show_the_people_the_place_or_private_values() {
        let file = classic_slice(&SliceOptions::default());
        let mut obj = crate::decode::open(&file).expect("parses");
        let made_up = [
            (dicom_core::Tag(0x0008, 0x0080), "Made-up Infirmary"),
            (dicom_core::Tag(0x0008, 0x1070), "Nobody^Operator"),
            (dicom_core::Tag(0x0009, 0x0010), "MADE UP CREATOR"),
            (dicom_core::Tag(0x0009, 0x1001), "made-up private words"),
        ];
        for (tag, value) in made_up {
            obj.put(DataElement::new(tag, VR::LO, value));
        }
        let mut bytes = Vec::new();
        obj.write_all(&mut bytes).expect("writes to memory");
        let printed = format!("{:?}", rows_of(bytes));
        for (_, value) in made_up {
            assert!(!printed.contains(value), "{value} in {printed}");
        }
        assert!(printed.contains("InstitutionName") && printed.contains("\"MR\""));
    }
}
