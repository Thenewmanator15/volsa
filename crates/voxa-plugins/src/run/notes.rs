//! What a reader of a plugin's result should know of how its labels were made: the spacing
//! the plugin was made for, what a whole box left out, and that blocks were used.

use voxa_core::Volume;

use super::Prepared;
use crate::accepts::in_mm;
use crate::{Description, overhang_mm, spacing_note};

/// The note on a result made in blocks.
pub const BLOCKS_NOTE: &str =
    "Run in blocks: near the edges between blocks, labels may differ from the model's own program.";

/// What a reader should know of how the labels were made: the spacing the plugin was made for,
/// what a whole box left out, and that blocks were used.
pub(super) fn notes(
    description: &Description,
    prepared: &Prepared,
    volume: &Volume,
) -> Vec<String> {
    let mut notes = Vec::new();
    notes.extend(spacing_note(
        volume.spacing_mm(),
        description.accepts.thickest_mm,
    ));
    if prepared.plan.is_some() {
        notes.push(BLOCKS_NOTE.to_owned());
    } else if let Some(overhang) = overhang_mm(&prepared.grid, prepared.region) {
        let spacing = description.input.spacing_mm;
        let sides: Vec<String> = prepared
            .block
            .iter()
            .zip(spacing)
            .map(|(side, mm)| in_mm(*side as f64 * mm))
            .collect();
        notes.push(format!(
            "The plugin's box of {} mm left out part of the scan, up to {} mm beyond the box.",
            sides.join(" × "),
            in_mm(overhang)
        ));
    }
    notes
}
