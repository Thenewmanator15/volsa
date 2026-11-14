//! Reading a segmentation of the export (DICOM SEG) on a worker, and taking it in as the labels
//! open, as a label file is read: a newer read, or labels given up, overtake it. Each segment
//! is one part, named and coloured as the file says; a segment with no colour of its own is
//! given the one a label of its number is given. Nothing here writes a path or an entry's name
//! anywhere.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_dicom::{Segmentation, read_segmentation};
use volsa_labels::{
    Entry, LabelGrid, LabelSet, MAX_NAME, Side, Table, Trust, colour_for, label_set,
};

use super::{Shared, Workers, lock, shielded};
use crate::action::Action;
use crate::labels::NamesFrom;
use crate::naming::printable;
use crate::state::{Notice, NoticeKind};

/// What is said when the segmentation cannot be got out of the export.
const NOT_FOUND: &str = "This segmentation could not be read. It is not in the export any more.";
/// What is said when reading stops part-way inside Volsa.
const STOPPED: &str = "Reading the segmentation stopped part-way.";
/// What is said of a segmentation whose masks were fractions, cut at half.
pub const CUT_AT_HALF: &str = "This segmentation gives each voxel a fraction of a segment. \
                               Voxels at half or more are shown as in the segment, and the \
                               rest are not.";
/// The notice about fractions takes the place of an earlier one.
const FRACTIONS: &str = "segmentation fractions";

impl Workers {
    /// Read the segmentation in `entry` of the export that is open on a worker, keep the label
    /// set made of it and report it. A read overtaken by a newer one, or given up, keeps and
    /// reports nothing.
    pub(super) fn read_segmentation(&self, entry: String) {
        let number = self.shared.label_reads.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let mut outcome = Err(STOPPED.to_owned());
            shielded(|| outcome = shared.segmentation_in(&entry));
            let mut kept = lock(&shared.labels);
            if shared.label_reads.load(Ordering::SeqCst) != number {
                return;
            }
            let report = match outcome {
                Ok(set) => {
                    let set = Arc::new(set);
                    *kept = Some(Arc::clone(&set));
                    // Labels from a file Volsa did not make are not saved as Volsa's own.
                    shared.keeping.read(&set, None);
                    Action::LabelsRead {
                        title: set.title.clone(),
                        parts: set.parts.clone(),
                        verdict: None,
                        corners: set.corners(),
                        names_from: NamesFrom::Table,
                        made: None,
                        run: None,
                    }
                }
                Err(why) => Action::LabelsFailed(why),
            };
            drop(kept);
            shared.report(report);
        });
    }
}

impl Shared {
    /// The segmentation in `entry` as a label set, or the sentence that says why it cannot be
    /// had. When its masks were fractions, a notice says how they were cut.
    fn segmentation_in(&self, entry: &str) -> Result<LabelSet, String> {
        let bytes = match self.read_entry(entry) {
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => {
                tracing::warn!("a segmentation could not be read: {}", error.kind());
                return Err(NOT_FOUND.to_owned());
            }
            None => return Err(NOT_FOUND.to_owned()),
        };
        let read = read_segmentation(&bytes).map_err(|why| why.to_string())?;
        drop(bytes);
        if read.fractional {
            self.report(Action::Post(Notice {
                kind: NoticeKind::Info,
                text: CUT_AT_HALF.to_owned(),
                details: Vec::new(),
                key: Some(FRACTIONS),
            }));
        }
        segmentation_labels(read)
    }
}

/// A segmentation as a label set: one part for each segment that marks a voxel, named and
/// coloured as the file says, in the file's order.
///
/// # Errors
///
/// The sentence that says why, when the segmentation has more segments than Volsa lists.
fn segmentation_labels(read: Segmentation) -> Result<LabelSet, String> {
    let entries = read.segments.iter().map(|segment| {
        let named = printable(segment.label.trim());
        let name = if named.trim().is_empty() {
            format!("Segment {}", segment.number)
        } else {
            named.chars().take(MAX_NAME).collect()
        };
        let id = u32::from(segment.number);
        Entry {
            name,
            group: String::new(),
            side: Side::Midline,
            ids: vec![id],
            colour: segment.colour.unwrap_or_else(|| colour_for(id)),
            trust: Trust::NotSaid,
        }
    });
    let table = Table {
        title: printable(&read.title).chars().take(MAX_NAME).collect(),
        entries: entries.collect(),
        verdict: None,
        made: None,
    };
    let grid = LabelGrid {
        dims: read.dims,
        ids: read
            .numbers
            .iter()
            .map(|number| u32::from(*number))
            .collect(),
        placement: read.placement,
    };
    drop(read);
    label_set(&grid, Some(&table)).map_err(|why| why.to_string())
}

#[cfg(test)]
mod tests;
