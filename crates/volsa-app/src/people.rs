//! Whose scans an export holds, and in what order the rail lists them. An export can hold more
//! than one person's studies, and a person who takes two people's scans for one person's could
//! compare them, so the rail says how many people there are and heads each one's studies.
//!
//! The rail lists the newest study first and the undated last, and each person by their newest
//! study; what opens first among the other files is the first the rail lists.

use std::cmp::Reverse;

use volsa_core::{SeriesInfo, StudyInfo};
use volsa_dicom::Index;

use crate::naming::{Group, group_of, printable};

/// One person whose studies are in an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// What their studies are headed with: `Person 1`, `Person 2`… while the patient's details
    /// are hidden, and their name and identifier while they are shown.
    pub label: String,
    /// Their studies, by their places in the index's list of studies, the newest first.
    pub studies: Vec<usize>,
}

/// The places of the studies of `index` in the order the rail lists them: the newest first and
/// those with no date last. Studies of one date, and the undated, keep the index's order.
pub fn newest_first(index: &Index) -> Vec<usize> {
    let mut order: Vec<usize> = (0..index.studies.len()).collect();
    order.sort_by_key(|at| {
        let date = index.studies.get(*at).and_then(date_of);
        (date.is_none(), Reverse(date))
    });
    order
}

/// A study's date, when its files give one as eight digits, `YYYYMMDD`, which sort as the days
/// do.
fn date_of(study: &StudyInfo) -> Option<&str> {
    let date = study.date.as_deref()?.trim();
    let digits = date.len() == 8 && date.bytes().all(|byte| byte.is_ascii_digit());
    digits.then_some(date)
}

/// The first series of `group` as the rail lists them: person by person, study by study, and
/// each series of several volumes by its first.
pub fn first_of(index: &Index, group: Group) -> Option<&SeriesInfo> {
    let people = people_of(index, false);
    let places = people.iter().flat_map(|person| person.studies.iter());
    let studies = places.filter_map(|at| index.studies.get(*at));
    let mut series = studies.flat_map(|study| study.series.iter());
    series.find(|info| info.leads_its_set() && group_of(info) == group)
}

/// The people whose studies are in `index`, each first listed where their newest study is.
///
/// Two studies are one person's when they give the same patient identifier and the same name,
/// as written, but for the spaces around them. Studies that differ in either are taken as two
/// people's: telling one person's scans apart as two people's does no harm, the other way
/// round could. Each is labelled with their name and identifier while `show_patient` is true,
/// and with their number otherwise.
pub fn people_of(index: &Index, show_patient: bool) -> Vec<Person> {
    let mut keys: Vec<(Option<&str>, Option<&str>)> = Vec::new();
    let mut people: Vec<Person> = Vec::new();
    for at in newest_first(index) {
        let Some(study) = index.studies.get(at) else {
            continue;
        };
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
    use volsa_core::{StudyId, StudyInfo};

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
    fn studies_are_listed_newest_first_and_the_undated_last() {
        let on = |date: Option<&str>| StudyInfo {
            date: date.map(str::to_owned),
            ..study(Some("MADE-UP-1"), Some("Made^Up"))
        };
        let dates = [
            Some("20190412"),
            None,
            Some("20210830"),
            Some("made-up"),
            Some("20200102"),
        ];
        let index = index_of(dates.into_iter().map(on).collect());
        assert_eq!(newest_first(&index), [2, 4, 0, 1, 3]);
    }

    #[test]
    fn people_are_listed_by_their_newest_study_and_their_studies_newest_first() {
        let of = |who: &str, date: &str| StudyInfo {
            date: Some(date.to_owned()),
            ..study(Some(who), Some("Made^Up"))
        };
        let studies = vec![
            of("MADE-UP-1", "20180101"),
            of("MADE-UP-2", "20190101"),
            of("MADE-UP-1", "20200101"),
        ];
        let people = people_of(&index_of(studies), true);
        assert_eq!(people[0].label, "Made^Up · MADE-UP-1");
        assert_eq!(
            (people[0].studies.as_slice(), people[1].studies.as_slice()),
            (&[2, 0][..], &[1][..])
        );
        assert_eq!(people_of(&index_of(Vec::new()), false), []);
    }

    #[test]
    fn the_first_of_a_group_is_the_first_the_rail_lists() {
        let page = |id: &str| SeriesInfo {
            id: volsa_core::SeriesId(id.to_owned()),
            modality: "SC".to_owned(),
            ..SeriesInfo::default()
        };
        let in_study = |who: &str, date: &str, id: &str| StudyInfo {
            date: Some(date.to_owned()),
            series: vec![page(id)],
            ..study(Some(who), None)
        };
        let older = in_study("MADE-UP-1", "20190101", "made-up older page");
        let newer = in_study("MADE-UP-2", "20200101", "made-up newer page");
        let index = index_of(vec![older, newer]);
        let first = first_of(&index, Group::Other).map(|info| info.id.0.as_str());
        assert_eq!(first, Some("made-up newer page"));
        assert_eq!(first_of(&index, Group::Scans), None);
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
