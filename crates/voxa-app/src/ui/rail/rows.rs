//! What the rail holds, from top to bottom, as rows of fixed heights, so that the rows in sight
//! can be found without laying out the others: the scans, each person's and each study's
//! under a heading where there are several, then the heading of the other files and, while
//! they are open, the other files in the same order.

use voxa_core::{SeriesId, SeriesInfo, StudyInfo};
use voxa_dicom::Index;

use super::card;
use super::people::Person;
use crate::naming::{Group, group_of};
use crate::ui::browser::{face_of, one_for_each};

/// How tall the heading of a person's studies is, with the room above it.
const PERSON: f32 = 30.0;
/// How tall the heading of a study is.
const STUDY: f32 = 22.0;
/// How tall the heading of the other files is, with the room above it.
const OTHER: f32 = 34.0;
/// The room between one card and the next.
pub const GAP: f32 = 4.0;

/// One row of the rail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Row<'a> {
    /// The heading of one person's studies.
    Person(&'a str),
    /// The heading of a study: its date and description.
    Study(&'a StudyInfo),
    /// The card of a series, and the series whose small picture it carries.
    Card {
        /// The series the card stands for.
        info: &'a SeriesInfo,
        /// The series whose picture is on it.
        face: &'a SeriesId,
    },
    /// The heading of the other files: how many cards it holds.
    Other(usize),
}

impl Row<'_> {
    /// How tall the row is, with the room that goes with it.
    pub fn height(&self) -> f32 {
        match self {
            Self::Person(_) => PERSON,
            Self::Study(_) => STUDY,
            Self::Card { .. } => card::HEIGHT + GAP,
            Self::Other(_) => OTHER,
        }
    }
}

/// What the rail is asked to hold.
pub struct Asked<'a> {
    /// What the export holds.
    pub index: &'a Index,
    /// Whose studies they are.
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
pub fn rows<'a>(asked: &Asked<'a>) -> Vec<Row<'a>> {
    let mut rows = group(asked, Group::Scans);
    let others = group(asked, Group::Other);
    let count = others
        .iter()
        .filter(|row| matches!(row, Row::Card { .. }))
        .count();
    if count > 0 {
        rows.push(Row::Other(count));
        if asked.other_open {
            rows.extend(others);
        }
    }
    rows
}

/// The rows of the series of one group, person by person and study by study.
fn group<'a>(asked: &Asked<'a>, which: Group) -> Vec<Row<'a>> {
    let several = asked.people.len() > 1;
    let head_people = several || asked.show_patient;
    let head_studies = several || asked.index.studies.len() > 1;
    let mut rows = Vec::new();
    for person in asked.people {
        let studies = person.studies.iter();
        let studies = studies.filter_map(|at| asked.index.studies.get(*at));
        let mut theirs = Vec::new();
        for study in studies {
            let cards = cards_of(study, asked.shown, which);
            if cards.is_empty() {
                continue;
            }
            if head_studies {
                theirs.push(Row::Study(study));
            }
            theirs.extend(cards);
        }
        if theirs.is_empty() {
            continue;
        }
        if head_people {
            rows.push(Row::Person(&person.label));
        }
        rows.extend(theirs);
    }
    rows
}

/// The cards of a study's series in `group`, one for each series of several volumes.
fn cards_of<'a>(study: &'a StudyInfo, shown: Option<&SeriesId>, which: Group) -> Vec<Row<'a>> {
    let standing = one_for_each(&study.series, shown).into_iter();
    standing
        .filter(|info| group_of(info) == which)
        .map(|info| Row::Card {
            info,
            face: &face_of(&study.series, info).id,
        })
        .collect()
}
