//! Highlighting the thin dark tubes of a series' volume on a worker, and taking them in as the
//! labels open, as a label file is read: a newer read, or labels given up, overtake it. When
//! labels are open on the study, the tubes are put onto them as one more part, so that both
//! are shown together; otherwise they open alone.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_core::{Grid as _, SeriesId};
use volsa_labels::{
    LabelGrid, LabelSet, MAX_MADE_LINE, MAX_NOTES, Made, label_set, write_labels, write_table,
};
use volsa_segment::{veins, veins_onto, veins_table};

use super::kept::{MAX_SAVED_BYTES, Saved};
use super::{Workers, lock, shielded};
use crate::action::Action;
use crate::labels::NamesFrom;

/// What is said when the volume of the series is no longer there.
const NO_VOLUME: &str = "The 3D view of this scan is no longer there to look for dark tubes in.";
/// What is said when the work stops part-way inside Volsa.
const STOPPED: &str = "Highlighting stopped part-way: something went wrong inside Volsa.";
/// What the made-up model is called where a result says who made it.
pub const MADE_BY: &str = "Dark-tube filter (no trained model)";
/// What a result of a scan not named as SWI or T2* says.
pub const NOT_NAMED: &str = "This scan's description does not name it as SWI or T2*. Veins are \
                             dark on those; on other scans what is highlighted may not be \
                             veins at all.";
/// What labels the tubes were put onto say of them.
pub const PUT_ONTO: &str = "Dark tubes (mostly veins) were added to these labels by a filter, \
                            not a trained model.";

/// What a run is given.
struct Asked {
    named: bool,
    when: String,
    /// The labels open, with where their names came from, to put the tubes onto.
    onto: Option<(Arc<LabelSet>, NamesFrom)>,
}

impl Workers {
    /// Highlight the thin dark tubes of `series`' volume on a worker, keep the labels made of
    /// them and report them as labels read. `named` says whether the series is named as SWI or
    /// T2*; if not, the result says so. With `onto`, the tubes are put onto the labels held.
    pub(super) fn find_veins(&self, series: &SeriesId, named: bool, onto: Option<NamesFrom>) {
        let number = self.shared.label_reads.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        let volume = lock(&self.shared.volumes).own(series);
        let held = lock(&self.shared.labels).clone();
        let asked = Asked {
            named,
            when: self.shared.labelling.labeller().today(),
            onto: held.zip(onto),
        };
        self.spawn(move || {
            let overtaken = || shared.label_reads.load(Ordering::SeqCst) != number;
            let mut outcome = Err(STOPPED.to_owned());
            shielded(|| {
                outcome = match volume.as_deref() {
                    Some(held) => found(held, &asked, &overtaken),
                    None => Err(NO_VOLUME.to_owned()),
                };
            });
            let mut kept = lock(&shared.labels);
            if overtaken() {
                return;
            }
            let report = match outcome {
                Ok((set, saved, names_from)) => {
                    let set = Arc::new(set);
                    *kept = Some(Arc::clone(&set));
                    shared.keeping.read(&set, saved);
                    Action::LabelsRead {
                        title: set.title.clone(),
                        parts: set.parts.clone(),
                        verdict: set.verdict.clone(),
                        corners: set.corners(),
                        names_from,
                        made: set.made.clone(),
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

/// The labels of the dark tubes of `held`, alone or put onto the labels asked, with their two
/// files as Volsa would save them when they are alone.
fn found(
    held: &volsa_core::HeldVolume,
    asked: &Asked,
    overtaken: &(dyn Fn() -> bool + Sync),
) -> Result<(LabelSet, Option<Saved>, NamesFrom), String> {
    // The filter is given 32-bit values of the whole scan, as labelling is: made here, for
    // this alone.
    let volume = held.to_volume(0..held.dims()[2]).ok_or(NO_VOLUME)?;
    let grid = veins(&volume, overtaken).map_err(|why| why.to_string())?;
    drop(volume);
    let not_named = (!asked.named).then(|| NOT_NAMED.to_owned());
    if let Some((open, names_from)) = &asked.onto {
        let mut set = veins_onto(open, &grid).map_err(|why| why.to_string())?;
        if let Some(made) = set.made.as_mut() {
            let notes = [PUT_ONTO.to_owned()].into_iter().chain(not_named);
            made.notes
                .retain(|note| note != PUT_ONTO && note != NOT_NAMED);
            made.notes.extend(notes);
            made.notes.truncate(MAX_NOTES);
        }
        // Labels changed from what was read are not saved as if they were the file.
        return Ok((set, None, *names_from));
    }
    let (set, saved) = alone(&grid, not_named, &asked.when)?;
    Ok((set, saved, NamesFrom::Table))
}

/// The tubes as labels of their own, said to be made by the filter.
fn alone(
    grid: &LabelGrid,
    not_named: Option<String>,
    when: &str,
) -> Result<(LabelSet, Option<Saved>), String> {
    let line = |text: &str| text.chars().take(MAX_MADE_LINE).collect::<String>();
    let table = volsa_labels::Table {
        made: Some(Made {
            by: line(&format!("Volsa {}", env!("CARGO_PKG_VERSION"))),
            model: line(MADE_BY),
            model_sum: String::new(),
            device: "the processor".to_owned(),
            when: line(when),
            notes: not_named.into_iter().collect(),
            added: None,
        }),
        ..veins_table()
    };
    let set = label_set(grid, Some(&table)).map_err(|why| why.to_string())?;
    let labels = write_labels(grid).ok();
    let text = write_table(&table).ok();
    let saved = labels
        .zip(text)
        .filter(|(labels, _)| labels.len() <= MAX_SAVED_BYTES)
        .map(|(labels, table)| Saved { labels, table });
    Ok((set, saved))
}
