//! What a made-up export says of its patient, its study and its scans, for the tests that look
//! for any of it in what Voxa saves or writes.

use voxa_dicom::Index;

/// Every piece of text the export gives that could identify the patient or the scan, each name
/// part by itself: the patient's name, identifier, the study's date and description, and the
/// series' descriptions. None is empty.
pub fn said_of(index: &Index) -> Vec<String> {
    let mut said: Vec<String> = Vec::new();
    for study in &index.studies {
        let name = study.patient_name.iter().flat_map(|name| name.split('^'));
        said.extend(name.map(str::to_owned));
        said.extend(study.patient_id.clone());
        said.extend(study.date.clone());
        said.push(study.description.clone());
    }
    let series = index.series.iter();
    said.extend(series.map(|series| series.info.description.clone()));
    said.retain(|text| !text.is_empty());
    assert!(
        said.len() > 4,
        "the made-up export says something: {said:?}"
    );
    said
}
