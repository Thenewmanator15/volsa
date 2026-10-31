//! Whose scans an export holds. An export can hold more than one person's studies, and a
//! person who takes two people's scans for one person's could compare them, so the rail says
//! how many people there are and heads each one's studies.

use voxa_dicom::Index;

use crate::naming::printable;

/// One person whose studies are in an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// What their studies are headed with: `Person 1`, `Person 2`… while the patient's details
    /// are hidden, and their name and identifier while they are shown.
    pub label: String,
    /// Their studies, by their places in the index's list of studies, in its order.
    pub studies: Vec<usize>,
}

/// The people whose studies are in `index`, in the order their first study comes in it.
///
/// Two studies are one person's when they give the same patient identifier and the same name,
/// as written, but for the spaces around them. Studies that differ in either are taken as two
/// people's: telling one person's scans apart as two people's does no harm, the other way
/// round could. Each is labelled with their name and identifier while `show_patient` is true,
/// and with their number otherwise.
pub fn people_of(index: &Index, show_patient: bool) -> Vec<Person> {
    let mut keys: Vec<(Option<&str>, Option<&str>)> = Vec::new();
    let mut people: Vec<Person> = Vec::new();
    for (at, study) in index.studies.iter().enumerate() {
        let key = (
            given(study.patient_id.as_ref()),
            given(study.patient_name.as_ref()),
        );
        if let Some(place) = keys.iter().position(|known| *known == key) {
            if let Some(person) = people.get_mut(place) {
                person.studies.push(at);
            }
            continue;
        }
        keys.push(key);
        people.push(Person {
            label: label(key, keys.len(), show_patient),
            studies: vec![at],
        });
    }
    people
}

/// A field of a study as it was written, without the spaces around it; `None` when there is
/// nothing else.
fn given(field: Option<&String>) -> Option<&str> {
    field
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
}

/// What the `number`th person's studies are headed with, whose identifier and name are `key`.
fn label((id, name): (Option<&str>, Option<&str>), number: usize, show_patient: bool) -> String {
    let known: Vec<String> = [name, id].into_iter().flatten().map(printable).collect();
    if show_patient && !known.is_empty() {
        known.join(" · ")
    } else {
        format!("Person {number}")
    }
}

#[cfg(test)]
mod tests {
    use voxa_core::{StudyId, StudyInfo};

    use super::*;

    /// A made-up study of someone with this identifier and name.
    fn study(id: Option<&str>, name: Option<&str>) -> StudyInfo {
        StudyInfo {
            id: StudyId(format!("1.2.9.{}", id.unwrap_or("none"))),
            date: None,
            description: String::new(),
            patient_name: name.map(str::to_owned),
            patient_id: id.map(str::to_owned),
            series: Vec::new(),
        }
    }

    fn index_of(studies: Vec<StudyInfo>) -> Index {
        Index {
            studies,
            ..Index::default()
        }
    }

    #[test]
    fn studies_are_one_persons_when_their_identifier_and_name_agree() {
        let index = index_of(vec![
            study(Some("MADE-UP-1"), Some("Made^Up")),
            study(Some("MADE-UP-2"), Some("Other^Made")),
            study(Some(" MADE-UP-1 "), Some("Made^Up ")),
        ]);
        let people = people_of(&index, false);
        let labels: Vec<&str> = people.iter().map(|person| person.label.as_str()).collect();
        assert_eq!(labels, ["Person 1", "Person 2"]);
        assert_eq!(people[0].studies, [0, 2]);
        assert_eq!(people[1].studies, [1]);
        // Shown, each is called by their name and identifier.
        let named = people_of(&index, true);
        assert_eq!(named[0].label, "Made^Up · MADE-UP-1");
        assert_eq!(named[1].label, "Other^Made · MADE-UP-2");
    }

    #[test]
    fn studies_that_differ_in_identifier_or_name_are_two_peoples() {
        let index = index_of(vec![
            study(Some("MADE-UP-1"), Some("Made^Up")),
            study(Some("MADE-UP-1"), Some("Someone^Else")),
            study(None, Some("Made^Up")),
            study(None, None),
            study(Some(""), Some("  ")),
        ]);
        let people = people_of(&index, true);
        assert_eq!(people.len(), 4, "{people:?}");
        // Blank is as good as not given; with nothing given, a person is called by number.
        assert_eq!(people[3].studies, [3, 4]);
        assert_eq!(people[3].label, "Person 4");
    }
}
