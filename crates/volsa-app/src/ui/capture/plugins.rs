//! What a picture of the window can ask for about plugins: a made-up plugin added, as the Parts
//! tab lists it (`plugins`, with `parts`); the card a made-up folder offered is shown on
//! (`plugin-card`); and the warning before a run of it on a scan whose description names
//! another contrast (`plugin-warning`). None of it touches the settings folder.

use std::sync::OnceLock;

use volsa_plugins::{Accepts, Contrast, Contrasts};

use super::{SHOW_VARIABLE, VARIABLE, asks_for};
use crate::plugins::{OfferCard, PluginAction, PluginEntry, Plugins, Warning};

/// The made-up plugin's identity.
const ID: &str = "example.skull-strip";
/// What the made-up warning says, as the run's check would say it.
const WARNED: &str = "This plugin expects T1-weighted; this scan's description says T2-weighted.";

/// The look at the plugins added that a picture asking for made-up plugins is given in place of
/// the first look, which is not made. It answers the look numbered 0.
pub fn made_up_look() -> Option<PluginAction> {
    let entries = made_up()?.entries.clone();
    Some(PluginAction::Looked { look: 0, entries })
}

/// What plugins have open over the window: the made-up card or warning a picture asks for, in
/// place of what `plugins` has open.
pub fn shown(plugins: &Plugins) -> &Plugins {
    let opens = |made: &&Plugins| made.offered.is_some() || made.warning.is_some();
    made_up().filter(opens).unwrap_or(plugins)
}

/// The made-up plugins this picture asks for, if it is a picture and asks for any.
fn made_up() -> Option<&'static Plugins> {
    static MADE_UP: OnceLock<Option<Plugins>> = OnceLock::new();
    let asked = || {
        std::env::var_os(VARIABLE)?;
        asked(&std::env::var(SHOW_VARIABLE).ok()?)
    };
    MADE_UP.get_or_init(asked).as_ref()
}

/// The made-up plugins the words `wanted` ask for, if they ask for any.
pub(super) fn asked(wanted: &str) -> Option<Plugins> {
    let card = asks_for(wanted, "plugin-card");
    let warning = asks_for(wanted, "plugin-warning");
    if !(card || warning || asks_for(wanted, "plugins")) {
        return None;
    }
    Some(Plugins {
        entries: vec![entry("the processor and Made-up card A")],
        offered: card.then(offered),
        warning: warning.then(|| Warning {
            id: ID.to_owned(),
            warning: WARNED.to_owned(),
            stamp: format!("Run after a warning: {}", lower_first(WARNED)),
            held: 0,
        }),
        ..Plugins::default()
    })
}

/// A sentence with its first letter made small, to follow a colon.
fn lower_first(sentence: &str) -> String {
    let mut letters = sentence.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}

/// The made-up plugin, said to run on `runs_on`.
fn entry(runs_on: &str) -> PluginEntry {
    PluginEntry {
        id: ID.to_owned(),
        name: "Made-up skull strip".to_owned(),
        version: "1.0".to_owned(),
        made_by: "A made-up lab".to_owned(),
        source: "https://example.org/made-up-skull-strip".to_owned(),
        licence: "MIT".to_owned(),
        about: "Finds the brain inside the skull of a head scan.".to_owned(),
        limits: vec![
            "Made for adults' heads.".to_owned(),
            "Not checked on scans after surgery.".to_owned(),
        ],
        checked: Ok(()),
        accepts: Accepts {
            modality: vec!["MR".to_owned()],
            contrast: Contrasts::Only(vec![Contrast::T1]),
            body_part: "head".to_owned(),
            thickest_mm: None,
        },
        accepts_words: "MR scans, T1-weighted, of the head".to_owned(),
        returns: "Labels: 1 named part.".to_owned(),
        runs_on: runs_on.to_owned(),
    }
}

/// The card of the made-up folder, which replaces an older version of the plugin.
fn offered() -> OfferCard {
    let settings = [
        "Resampled to 1 mm.",
        "Turned towards the right, front, top.",
        "Fitted to 256 × 256 × 256 voxels, centred on the scan.",
        "Outside the scan: the scan's lowest value.",
        "Brightness: the 2nd to the 98th percentile of the scan becomes 0 to 1.",
        "Answer: the label of the highest of 2 scores.",
        "Only the largest connected piece is kept.",
    ];
    OfferCard {
        entry: entry("measured when it is added"),
        settings: settings.map(str::to_owned).to_vec(),
        replaces: Some("0.9".to_owned()),
    }
}
