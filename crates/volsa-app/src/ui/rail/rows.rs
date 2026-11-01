//! What the rail holds, from top to bottom, as rows of fixed heights, so that the rows in sight
//! can be found without laying out the others: the scans, each person's and each study's
//! under a heading where there are several, then the heading of the other files and, while
//! they are open, the other files in the same order: each study's series that are not scans,
//! then its documents. A row names what it shows by its place in the index, so that the rows can
//! be kept from one frame to the next.

use volsa_core::{SeriesId, StudyInfo};
use volsa_dicom::Index;

use super::card;
use crate::documents;
use crate::naming::{Group, group_of};
use crate::people::Person;
use crate::ui::browser::one_for_each;

/// How tall the heading of a person's studies is, with the room above it.
const PERSON: f32 = 30.0;
/// How tall the heading of a study is.
const STUDY: f32 = 22.0;
/// How tall the heading of the other files is, with the room above it.
const OTHER: f32 = 34.0;
/// The room between one card and the next.
pub const GAP: f32 = 4.0;

/// One row of the rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The heading of one person's studies, by the person's place among the people.
    Person(usize),
    /// The heading of a study, by its place among the index's studies.
    Study(usize),
    /// The card of a series.
    Card {
        /// The place of its study among the index's studies.
        study: usize,
        /// Its place among that study's series.
        series: usize,
    },
    /// The card of a document, by its place among the index's documents.
    Document(usize),
    /// The heading over the documents of studies that hold no scans.
    Unplaced,
    /// The heading of the other files: how many cards it holds.
    Other(usize),
}

impl Row {
    /// How tall the row is, with the room that goes with it.
    pub fn height(self) -> f32 {
        match self {
            Self::Person(_) => PERSON,
            Self::Study(_) | Self::Unplaced => STUDY,
            Self::Card { .. } | Self::Document(_) => card::HEIGHT + GAP,
            Self::Other(_) => OTHER,
        }
    }
}

/// What the rail is asked to hold.
pub struct Asked<'a> {
    /// What the export holds.
    pub index: &'a Index,
    /// Whose studies they are, in the order the rail lists them.
    pub people: &'a [Person],
    /// The series the active viewport shows, whose volume stands for its series.
    pub shown: Option<&'a SeriesId>,
    /// Whether the other files are open.
    pub other_open: bool,
    /// Whether the patient's details are shown, which heads even one person's studies.
    pub show_patient: bool,
}

/// The rows of the rail. Each person's studies are headed when there are several people, or
/// the patient's details are shown; each study is headed when the export holds several
/// studies or several people, the top bar naming the study otherwise.
pub fn rows(asked: &Asked<'_>) -> Vec<Row> {
    let mut rows = group(asked, Group::Scans);
    let others = group(asked, Group::Other);
    let cards = |row: &&Row| matches!(row, Row::Card { .. } | Row::Document(_));
    let count = others.iter().filter(cards).count();
    if count > 0 {
        rows.push(Row::Other(count));
        if asked.other_open {
            rows.extend(others);
        }
    }
    rows
}

/// The rows of the series of one group, person by person and study by study.
fn group(asked: &Asked<'_>, which: Group) -> Vec<Row> {
    let several = asked.people.len() > 1;
    let head_people = several || asked.show_patient;
    let head_studies = several || asked.index.studies.len() > 1;
    let mut rows = Vec::new();
    for (place, person) in asked.people.iter().enumerate() {
        let mut theirs = Vec::new();
        for &at in &person.studies {
            let Some(study) = asked.index.studies.get(at) else {
                continue;
            };
            let mut cards = cards_of(at, study, asked.shown, which);
            if which == Group::Other {
                cards.extend(
                    documents::of_study(asked.index, at)
                        .into_iter()
                        .map(Row::Document),
                );
            }
            if cards.is_empty() {
                continue;
            }
            if head_studies {
                theirs.push(Row::Study(at));
            }
            theirs.extend(cards);
        }
        if theirs.is_empty() {
            continue;
        }
        if head_people {
            rows.push(Row::Person(place));
        }
        rows.extend(theirs);
    }
    if which == Group::Other {
        rows.extend(unplaced(asked.index));
    }
    rows
}

/// The rows of the documents of studies that hold no scans, under a heading that says so
/// whenever the export holds studies, so that they are not taken for those of the study above.
fn unplaced(index: &Index) -> Vec<Row> {
    let alone = documents::unplaced(index);
    let heading = (!alone.is_empty() && !index.studies.is_empty()).then_some(Row::Unplaced);
    heading
        .into_iter()
        .chain(alone.into_iter().map(Row::Document))
        .collect()
}

/// The cards of the series in `which` of the study at `at`, one for each series of several
/// volumes.
fn cards_of(at: usize, study: &StudyInfo, shown: Option<&SeriesId>, which: Group) -> Vec<Row> {
    let standing = one_for_each(&study.series, shown).into_iter();
    standing
        .filter(|series| {
            let info = study.series.get(*series);
            info.is_some_and(|info| group_of(info) == which)
        })
        .map(|series| Row::Card { study: at, series })
        .collect()
}
