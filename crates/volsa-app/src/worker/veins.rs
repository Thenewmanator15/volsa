//! Highlighting the thin dark tubes of a series' volume on a worker, and taking them in as the
//! labels open, as a label file is read: a newer read, or labels given up, overtake it.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_core::{Grid as _, SeriesId};
use volsa_labels::{LabelSet, MAX_MADE_LINE, Made, label_set, write_labels, write_table};
use volsa_segment::{veins, veins_table};

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

impl Workers {
    /// Highlight the thin dark tubes of `series`' volume on a worker, keep the labels made of
    /// them and report them as labels read. `named` says whether the series is named as SWI or
    /// T2*; if not, the result says so.
    pub(super) fn find_veins(&self, series: &SeriesId, named: bool) {
        let number = self.shared.label_reads.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = Arc::clone(&self.shared);
        let volume = lock(&self.shared.volumes).get(series);
        let when = self.shared.labelling.labeller().today();
        self.spawn(move || {
            let overtaken = || shared.label_reads.load(Ordering::SeqCst) != number;
            let mut outcome = Err(STOPPED.to_owned());
            shielded(|| {
                outcome = match volume.as_deref() {
                    Some(held) => found(held, named, &when, &overtaken),
                    None => Err(NO_VOLUME.to_owned()),
                };
            });
            let mut kept = lock(&shared.labels);
            if overtaken() {
                return;
            }
            let report = match outcome {
                Ok((set, saved)) => {
                    let set = Arc::new(set);
                    *kept = Some(Arc::clone(&set));
                    shared.keeping.read(&set, saved);
                    Action::LabelsRead {
                        title: set.title.clone(),
                        parts: set.parts.clone(),
                        verdict: set.verdict.clone(),
                        corners: set.corners(),
                        names_from: NamesFrom::Table,
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

/// The labels of the dark tubes of `held`, with their two files as Volsa would save them.
fn found(
    held: &volsa_core::HeldVolume,
    named: bool,
    when: &str,
    overtaken: &(dyn Fn() -> bool + Sync),
) -> Result<(LabelSet, Option<Saved>), String> {
    // The filter is given 32-bit values of the whole scan, as labelling is: made here, for
    // this alone.
    let volume = held.to_volume(0..held.dims()[2]).ok_or(NO_VOLUME)?;
    let grid = veins(&volume, overtaken).map_err(|why| why.to_string())?;
    drop(volume);
    let line = |text: &str| text.chars().take(MAX_MADE_LINE).collect::<String>();
    let notes = (!named).then(|| NOT_NAMED.to_owned());
    let table = volsa_labels::Table {
        made: Some(Made {
            by: line(&format!("Volsa {}", env!("CARGO_PKG_VERSION"))),
            model: line(MADE_BY),
            model_sum: String::new(),
            device: "the processor".to_owned(),
            when: line(when),
            notes: notes.into_iter().collect(),
            added: None,
        }),
        ..veins_table()
    };
    let set = label_set(&grid, Some(&table)).map_err(|why| why.to_string())?;
    let labels = write_labels(&grid).ok();
    let text = write_table(&table).ok();
    let saved = labels
        .zip(text)
        .filter(|(labels, _)| labels.len() <= MAX_SAVED_BYTES)
        .map(|(labels, table)| Saved { labels, table });
    Ok((set, saved))
}
