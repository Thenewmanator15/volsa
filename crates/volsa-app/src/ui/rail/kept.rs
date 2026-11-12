//! What the rail keeps from one frame to the next: whether the other files are open, what it
//! last saw of the viewports, and its rows, which are worked out again only when what they come
//! from has changed. Drawing the rail then does no work that grows with the export: the rows in
//! sight are found among the kept ones by their heights.

use volsa_core::SeriesId;
use volsa_dicom::Index;

use super::rows::{self, Row};
use crate::naming::{Group, group_of};
use crate::people::{Person, people_of};
use crate::state::AppState;

/// What the rail keeps from one frame to the next.
#[derive(Debug, Default)]
pub struct RailState {
    /// Whether the other files are shown under the scans.
    pub other_open: bool,
    /// What the rail last saw of the index and the viewports.
    looked: Option<Looked>,
    /// The rows as last worked out, and what they were worked out from.
    laid: Option<Laid>,
    /// How many times the rows have been worked out.
    #[cfg(test)]
    pub(super) builds: usize,
}

/// What the rail last saw: how many times the index had changed, the series each viewport of
/// the layout showed, and the document that was open.
#[derive(Debug)]
struct Looked {
    index: u64,
    shown: Vec<Option<SeriesId>>,
    document: Option<String>,
}

/// The rows of the rail, and what they were worked out from.
#[derive(Debug)]
pub struct Laid {
    /// What they were worked out from.
    source: Source,
    /// Whose studies the export holds, in the order the rail lists them.
    pub people: Vec<Person>,
    /// The rows, from the top.
    pub rows: Vec<Row>,
    /// The top of each row from the top of the list, and last, the foot of the last row.
    pub tops: Vec<f32>,
}

/// What the rows are worked out from.
#[derive(Debug, PartialEq)]
struct Source {
    index: u64,
    shown: Option<SeriesId>,
    other_open: bool,
    show_patient: bool,
}

/// The series the active viewport shows.
pub fn shown_in(state: &AppState) -> Option<&SeriesId> {
    let viewport = state.viewports.get(state.active)?;
    viewport.series.as_ref()
}

impl RailState {
    /// Open the other files once a viewport of the layout comes to show one of them or a
    /// document comes to be open, and when an export that holds no scans has been read while one
    /// of its other files is shown, as it is when Volsa opens such an export, whichever viewport
    /// is active. Their card is then in sight.
    pub fn follow(&mut self, state: &AppState) {
        let viewports = state.viewports.iter().take(state.layout.count());
        let shown: Vec<Option<&SeriesId>> = viewports.map(|v| v.series.as_ref()).collect();
        let was = self.looked.take();
        let same_index = was
            .as_ref()
            .is_some_and(|was| was.index == state.index_changes);
        let before = |at: usize| was.as_ref().and_then(|was| was.shown.get(at)?.as_ref());
        let other = |id: &SeriesId| {
            let listed = state.series(id);
            listed.is_some_and(|listed| group_of(&listed.info) == Group::Other)
        };
        let places = shown.iter().enumerate();
        let came = places
            .filter_map(|(at, id)| id.filter(|id| before(at) != Some(*id)))
            .any(other);
        let document = state.document.as_ref().map(|open| open.entry.as_str());
        let opened =
            document.is_some() && was.as_ref().map(|was| was.document.as_deref()) != Some(document);
        let no_scans = !same_index && state.index.as_ref().is_some_and(holds_no_scans);
        let only_other =
            no_scans && (document.is_some() || shown.iter().flatten().any(|id| other(id)));
        self.other_open |= came || opened || only_other;
        self.looked = Some(Looked {
            index: state.index_changes,
            shown: shown.into_iter().map(Option::<&SeriesId>::cloned).collect(),
            document: document.map(str::to_owned),
        });
    }

    /// The rows of the rail for `state`, whose index is `index`, worked out again only when the
    /// index, the series shown, whether the other files are open or whether the patient's
    /// details are shown has changed since they last were.
    pub fn laid(&mut self, index: &Index, state: &AppState) -> &Laid {
        let source = Source {
            index: state.index_changes,
            shown: shown_in(state).cloned(),
            other_open: self.other_open,
            show_patient: state.show_patient,
        };
        if self.laid.as_ref().is_some_and(|laid| laid.source != source) {
            self.laid = None;
        }
        #[cfg(test)]
        {
            self.builds += usize::from(self.laid.is_none());
        }
        self.laid.get_or_insert_with(|| Laid::of(index, source))
    }
}

impl Laid {
    /// The rows for `index` as `source` asks for them.
    fn of(index: &Index, source: Source) -> Self {
        let people = people_of(index, source.show_patient);
        let asked = rows::Asked {
            index,
            people: &people,
            shown: source.shown.as_ref(),
            other_open: source.other_open,
            show_patient: source.show_patient,
        };
        let rows = rows::rows(&asked);
        let mut tops = Vec::with_capacity(rows.len() + 1);
        let mut top = 0.0;
        for row in &rows {
            tops.push(top);
            top += row.height();
        }
        tops.push(top);
        Self {
            source,
            people,
            rows,
            tops,
        }
    }

    /// How tall all the rows are together.
    pub fn height(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    /// The place of the first row whose foot is below `y`, from the top of the list.
    pub fn first_below(&self, y: f32) -> usize {
        self.tops.partition_point(|top| *top <= y).saturating_sub(1)
    }
}

/// Whether an export holds none but other files.
fn holds_no_scans(index: &Index) -> bool {
    !index
        .series
        .iter()
        .any(|series| group_of(&series.info) == Group::Scans)
}
