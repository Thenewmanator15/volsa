//! Writing a table of names: the shape the labelling pipeline writes, which `read_table`
//! reads back.

use std::collections::HashMap;

use serde::ser::{Serialize, Serializer};

use super::WriteError;
use crate::made::{MAX_MADE_LINE, VolsaObject, cut};
use crate::table::{
    Entry, MAX_ENTRIES, MAX_HOW, MAX_NAME, MAX_TABLE_BYTES, Side, Table, Trust, Verdict,
    colour_for, kept,
};

/// Write a table of names as the text of a JSON file, in the shape the labelling pipeline
/// writes: `regions`, each name with its group, its colour, the number for each side and how
/// it was made; `model` when a model made the labels; `qc` for the model's own check; and
/// a `volsa` object that says who made them.
///
/// What is written is read back as it was by [`read_table`](crate::read_table) when each
/// name's entries lie together, left before right before midline, each with one number, one
/// group and one trust. In any other table the entries of a name are brought together at the
/// place of its first, a name with no number is left out, a name with several numbers is
/// written with its first, and where a name's sides differ in group, colour or trust the
/// first side's is written; trust is never raised: the labels are said to be a model's only
/// when every name that is not approximate says so.
///
/// Names, groups and the title are cut as a reader cuts them; so is `Made`. See
/// [`Made`](crate::Made).
///
/// # Errors
///
/// What a reader would refuse or cut short is not written. [`WriteError::NoNames`] when no
/// entry names anything. [`WriteError::TooLarge`] when more than 65 535 entries do, or the
/// text would be longer than a reader takes. [`WriteError::NameTwice`] when two entries have
/// the same name and side.
pub fn write_table(table: &Table) -> Result<String, WriteError> {
    let groups = by_name(&table.entries);
    let named: usize = groups.iter().map(|(_, sides)| sides.len()).sum();
    if named == 0 {
        return Err(WriteError::NoNames);
    }
    if named > MAX_ENTRIES {
        return Err(WriteError::TooLarge);
    }
    let regions = groups
        .iter()
        .map(|(name, sides)| Ok((name.clone(), Region::of(sides)?)))
        .collect::<Result<Vec<_>, WriteError>>()?;
    let document = Document {
        title: kept(&table.title, MAX_NAME),
        model: by_model(&groups).then(|| model_name(table)),
        volsa: table.made.as_ref().map(VolsaObject::of),
        regions: InOrder(regions),
        qc: table.verdict.as_ref().and_then(Check::of),
    };
    let mut text = serde_json::to_string_pretty(&document).map_err(|_| WriteError::TooLarge)?;
    text.push('\n');
    if text.len() > MAX_TABLE_BYTES {
        return Err(WriteError::TooLarge);
    }
    Ok(text)
}

/// The entries that name something, gathered under their names as a reader keeps them, in
/// the order the names first come.
fn by_name(entries: &[Entry]) -> Vec<(String, Vec<&Entry>)> {
    let mut groups: Vec<(String, Vec<&Entry>)> = Vec::new();
    let mut place: HashMap<String, usize> = HashMap::new();
    for entry in entries.iter().filter(|entry| !entry.ids.is_empty()) {
        let name = kept(&entry.name, MAX_NAME);
        if let Some((_, sides)) = place.get(&name).and_then(|at| groups.get_mut(*at)) {
            sides.push(entry);
        } else {
            place.insert(name.clone(), groups.len());
            groups.push((name, vec![entry]));
        }
    }
    groups
}

/// Whether a model is to be named as having made the labels: there are names that are not
/// approximate, and every one of them is a model's.
fn by_model(groups: &[(String, Vec<&Entry>)]) -> bool {
    let approximate = |entry: &&Entry| matches!(entry.trust, Trust::Approximate(_));
    let plain: Vec<_> = groups
        .iter()
        .filter(|(_, sides)| !sides.iter().any(&approximate))
        .collect();
    !plain.is_empty()
        && plain
            .iter()
            .all(|(_, sides)| sides.iter().all(|entry| entry.trust == Trust::Model))
}

/// What the table calls the model: the model of who made it, or else the title, or else a word.
fn model_name(table: &Table) -> String {
    let named = table
        .made
        .as_ref()
        .map(|made| cut(&made.model, MAX_MADE_LINE));
    let named = named.filter(|model| !model.is_empty());
    let titled = Some(kept(&table.title, MAX_NAME)).filter(|title| !title.is_empty());
    named.or(titled).unwrap_or_else(|| "model".to_owned())
}

/// The whole of a table as it is written.
#[derive(serde::Serialize)]
struct Document {
    #[serde(skip_serializing_if = "String::is_empty")]
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volsa: Option<VolsaObject>,
    regions: InOrder<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    qc: Option<Check>,
}

/// Names and values written in the order given, not the alphabet's.
struct InOrder<T>(Vec<(String, T)>);

impl<T: Serialize> Serialize for InOrder<T> {
    fn serialize<S: Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        to.collect_map(self.0.iter().map(|(name, value)| (name, value)))
    }
}

/// One name of the table as it is written.
#[derive(serde::Serialize)]
struct Region {
    #[serde(skip_serializing_if = "String::is_empty")]
    group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    colour: Option<String>,
    ids: Sides,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    approximate: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    how: String,
}

/// The number for each side a name has.
#[derive(Default, serde::Serialize)]
struct Sides {
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    midline: Option<u32>,
}

impl Region {
    /// A name's entries as one region. A colour is written only when one side's is not the
    /// colour its number is given, which a reader finds again for itself.
    fn of(entries: &[&Entry]) -> Result<Self, WriteError> {
        let mut ids = Sides::default();
        for entry in entries {
            let place = match entry.side {
                Side::Left => &mut ids.left,
                Side::Right => &mut ids.right,
                Side::Midline => &mut ids.midline,
            };
            if place.is_some() {
                return Err(WriteError::NameTwice);
            }
            *place = entry.ids.first().copied();
        }
        let own = |entry: &&&Entry| {
            let number = entry.ids.first().copied().unwrap_or(0);
            entry.colour != colour_for(number)
        };
        let how = entries.iter().find_map(|entry| match &entry.trust {
            Trust::Approximate(how) => Some(kept(how, MAX_HOW)),
            _ => None,
        });
        Ok(Self {
            group: entries
                .first()
                .map_or_else(String::new, |entry| kept(&entry.group, MAX_NAME)),
            colour: entries.iter().find(own).map(|entry| hex(entry.colour)),
            ids,
            approximate: how.is_some(),
            how: how.unwrap_or_default(),
        })
    }
}

/// A colour written as `#rrggbb`.
fn hex([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// The model's own check as it is written: the scores that fell below the mark, the mark, and
/// whether it passed.
#[derive(serde::Serialize)]
struct Check {
    scores: InOrder<Option<f64>>,
    fail_below: f64,
    passed: bool,
    low: Vec<String>,
}

impl Check {
    /// A check as written, or `None` for one whose mark is not a number: a reader would not
    /// take it. A score that is not a number is written empty, for a group that was not scored.
    fn of(verdict: &Verdict) -> Option<Self> {
        let scored = |(name, score): &(String, f64)| {
            (
                kept(name, MAX_NAME),
                Some(*score).filter(|score| score.is_finite()),
            )
        };
        verdict.fail_below.is_finite().then(|| {
            let low: Vec<_> = verdict.low.iter().map(scored).collect();
            Self {
                low: low.iter().map(|(name, _)| name.clone()).collect(),
                scores: InOrder(low),
                fail_below: verdict.fail_below,
                passed: verdict.passed,
            }
        })
    }
}
