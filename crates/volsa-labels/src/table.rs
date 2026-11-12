//! Tables that name the numbers of a label volume. Three shapes are read: the table the
//! labelling pipeline writes beside its labels (names with a group, a colour, a number for
//! each side, how each was made, and the model's own check); a plain list of numbers with
//! names and colours; and a 3D Slicer colour table.

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use crate::made::{Made, read_made};

/// The longest a name, a group or a title is kept, in characters.
pub const MAX_NAME: usize = 80;
/// The longest an account of how a part was made is kept.
pub(crate) const MAX_HOW: usize = 300;
/// The longest table read, in bytes.
pub(crate) const MAX_TABLE_BYTES: usize = 4 << 20;
/// The most names a table may give.
pub(crate) const MAX_ENTRIES: usize = 65_535;

/// Which side of the patient a part is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The patient's left.
    Left,
    /// The patient's right.
    Right,
    /// On the midline, or with no side said.
    Midline,
}

/// How far a part's name can be trusted, by how the part was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// A trained model made it.
    Model,
    /// It is approximate; the words say how it was made, if the table does.
    Approximate(String),
    /// The table has no name for this number.
    NotNamed,
    /// The table names it and does not say how it was made.
    NotSaid,
}

/// One name of a table: a structure on one side, and the numbers that stand for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// What the structure is called.
    pub name: String,
    /// The group it is listed under; empty when the table has none.
    pub group: String,
    /// Which side it is on.
    pub side: Side,
    /// The numbers that stand for it in the label volume.
    pub ids: Vec<u32>,
    /// The colour it is drawn in: red, green, blue.
    pub colour: [u8; 3],
    /// How far its name can be trusted.
    pub trust: Trust,
}

/// A labelling model's check of its own result.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// Whether the check passed.
    pub passed: bool,
    /// The score below which a part of the check fails.
    pub fail_below: f64,
    /// What scored below that, with its score.
    pub low: Vec<(String, f64)>,
}

/// A table of names.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// What the labels are of, as the table says it; empty when it does not.
    pub title: String,
    /// The names, in the table's order.
    pub entries: Vec<Entry>,
    /// The model's own check, when the table carries one.
    pub verdict: Option<Verdict>,
    /// Who made the labels, when the table says; `None` for every table made elsewhere.
    pub made: Option<Made>,
}

/// Why a file cannot be read as a table of names.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    /// It is none of the kinds of table Volsa reads.
    #[error("This is not a table of names that Volsa can read.")]
    NotATable,
    /// It is too long to be one.
    #[error("This file is too long to be a table of names.")]
    TooLong,
}

/// Read a table of names. A file whose name ends `.ctbl` is read as a 3D Slicer colour
/// table; anything else as JSON, in either of the two shapes.
///
/// # Errors
///
/// When the text is too long, or holds no table.
pub fn read_table(file_name: &str, text: &str) -> Result<Table, TableError> {
    if text.len() > MAX_TABLE_BYTES {
        return Err(TableError::TooLong);
    }
    let table = if file_name.to_ascii_lowercase().ends_with(".ctbl") {
        slicer(text)
    } else {
        let written: Written = serde_json::from_str(text).map_err(|_| TableError::NotATable)?;
        written.into_table().ok_or(TableError::NotATable)?
    };
    if table.entries.is_empty() {
        return Err(TableError::NotATable);
    }
    Ok(table)
}

/// A colour that is the same every time for a number, and far from its neighbours': hues a
/// golden angle apart, which never settle into a few.
pub fn colour_for(number: u32) -> [u8; 3] {
    let hue = (f64::from(number) * 137.507_764).rem_euclid(360.0);
    // Three strengths of colour in turn, so that hues that do fall close still differ.
    let (strong, bright) = match number % 3 {
        0 => (0.70, 0.95),
        1 => (0.55, 0.80),
        _ => (0.85, 0.70),
    };
    let sector = hue / 60.0;
    let part = sector - sector.floor();
    let (low, falling, rising) = (
        bright * (1.0 - strong),
        bright * (1.0 - strong * part),
        bright * (1.0 - strong * (1.0 - part)),
    );
    let rgb = match byte(sector.floor()) {
        0 => [bright, rising, low],
        1 => [falling, bright, low],
        2 => [low, bright, rising],
        3 => [low, falling, bright],
        4 => [rising, low, bright],
        _ => [bright, low, falling],
    };
    rgb.map(|share| byte((share * 255.0).round()))
}

/// A whole number from 0 to 255, held to that span.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn byte(value: f64) -> u8 {
    value.clamp(0.0, 255.0) as u8
}

/// Words kept to a length, with no space at either end: trimmed, cut, and trimmed again of
/// any space the cut left at the end. Kept again, the words do not change.
pub(crate) fn kept(text: &str, most: usize) -> String {
    let cut: String = text.trim().chars().take(most).collect();
    cut.trim_end().to_owned()
}

/// A colour written as `#rrggbb`.
fn hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.trim().strip_prefix('#')?;
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let pair = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    Some([pair(0)?, pair(2)?, pair(4)?])
}

/// A JSON object read as its names and values in the order written. A table's order is the
/// order its parts are listed in, and must not be shuffled into the alphabet.
#[derive(Debug, Default)]
struct InOrder<T>(Vec<(String, T)>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for InOrder<T> {
    fn deserialize<D: serde::Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        struct Pairs<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Pairs<T> {
            type Value = InOrder<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry::<String, T>()? {
                    pairs.push(pair);
                }
                Ok(InOrder(pairs))
            }
        }
        from.deserialize_map(Pairs(std::marker::PhantomData))
    }
}

/// Either shape of JSON table, as written.
#[derive(Debug, Deserialize)]
struct Written {
    #[serde(default)]
    title: String,
    /// Named by the pipeline when a trained model made the labels.
    model: Option<serde_json::Value>,
    /// Who made the labels, if Volsa did. Any value is taken, so that it can never spoil a table.
    volsa: Option<serde_json::Value>,
    regions: Option<InOrder<Region>>,
    qc: Option<Check>,
    parts: Option<InOrder<String>>,
    #[serde(default)]
    colours: InOrder<String>,
}

/// One name of the pipeline's table.
#[derive(Debug, Deserialize)]
struct Region {
    #[serde(default)]
    group: String,
    colour: Option<String>,
    #[serde(default)]
    ids: Sides,
    #[serde(default)]
    approximate: bool,
    #[serde(default)]
    how: String,
}

/// The number for each side a name has.
#[derive(Debug, Default, Deserialize)]
struct Sides {
    left: Option<u32>,
    right: Option<u32>,
    midline: Option<u32>,
}

/// The model's own check, as the pipeline writes it.
#[derive(Debug, Deserialize)]
struct Check {
    #[serde(default)]
    scores: InOrder<Option<f64>>,
    fail_below: Option<f64>,
    passed: Option<bool>,
}

impl Written {
    fn into_table(self) -> Option<Table> {
        let by_model = self.model.is_some();
        let entries = match (self.regions, self.parts) {
            (Some(regions), _) => regions_entries(regions, by_model),
            (None, Some(parts)) => plain_entries(parts, &self.colours),
            (None, None) => return None,
        };
        Some(Table {
            title: kept(&self.title, MAX_NAME),
            entries,
            verdict: self.qc.and_then(Check::verdict),
            made: self.volsa.as_ref().and_then(read_made),
        })
    }
}

impl Check {
    fn verdict(self) -> Option<Verdict> {
        let fail_below = self.fail_below.filter(|score| score.is_finite())?;
        // A score the file leaves empty is a group that was not scored, and fails.
        let low: Vec<(String, f64)> = self
            .scores
            .0
            .into_iter()
            .map(|(name, score)| (name, score.unwrap_or(f64::NAN)))
            .filter(|(_, score)| score.is_nan() || *score < fail_below)
            .map(|(name, score)| (kept(&name, MAX_NAME), score))
            .collect();
        Some(Verdict {
            passed: self.passed.unwrap_or(low.is_empty()),
            fail_below,
            low,
        })
    }
}

/// The names of the pipeline's table: one for each side a name has, left before right.
fn regions_entries(regions: InOrder<Region>, by_model: bool) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (name, region) in regions.0 {
        let trust = if region.approximate {
            Trust::Approximate(kept(&region.how, MAX_HOW))
        } else if by_model {
            Trust::Model
        } else {
            Trust::NotSaid
        };
        let sides = [
            (Side::Left, region.ids.left),
            (Side::Right, region.ids.right),
            (Side::Midline, region.ids.midline),
        ];
        for (side, id) in sides {
            let Some(id) = id.filter(|_| entries.len() < MAX_ENTRIES) else {
                continue;
            };
            let colour = region.colour.as_deref().and_then(hex);
            entries.push(Entry {
                name: kept(&name, MAX_NAME),
                group: kept(&region.group, MAX_NAME),
                side,
                ids: vec![id],
                colour: colour.unwrap_or_else(|| colour_for(id)),
                trust: trust.clone(),
            });
        }
    }
    entries
}

/// The names of a plain table: a name for a number, and perhaps a colour for it.
fn plain_entries(parts: InOrder<String>, colours: &InOrder<String>) -> Vec<Entry> {
    let colour_of = |number: &str| {
        let written = colours.0.iter().find(|(of, _)| of.trim() == number);
        written.and_then(|(_, colour)| hex(colour))
    };
    let entry = |(number, name): (String, String)| {
        let id: u32 = number.trim().parse().ok()?;
        Some(Entry {
            name: kept(&name, MAX_NAME),
            group: String::new(),
            side: Side::Midline,
            ids: vec![id],
            colour: colour_of(number.trim()).unwrap_or_else(|| colour_for(id)),
            trust: Trust::NotSaid,
        })
    };
    let entries = parts.0.into_iter().filter_map(entry);
    entries.take(MAX_ENTRIES).collect()
}

/// A 3D Slicer colour table: a number, a name with underscores for spaces, then red, green
/// and blue, on each line. Lines that are comments, short or odd are passed over, and so is
/// number 0, the background.
fn slicer(text: &str) -> Table {
    let entry = |line: &str| {
        let mut fields = line.split_whitespace();
        let id: u32 = fields.next()?.parse().ok()?;
        let name = fields.next()?.replace('_', " ");
        let mut shade = || fields.next()?.parse::<u8>().ok();
        let colour = [shade()?, shade()?, shade()?];
        let (name, side) = match name.trim() {
            left if left.ends_with("(left)") => (left.trim_end_matches("(left)"), Side::Left),
            right if right.ends_with("(right)") => (right.trim_end_matches("(right)"), Side::Right),
            other => (other, Side::Midline),
        };
        (id != 0).then(|| Entry {
            name: kept(name, MAX_NAME),
            group: String::new(),
            side,
            ids: vec![id],
            colour,
            trust: Trust::NotSaid,
        })
    };
    let lines = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'));
    Table {
        title: String::new(),
        entries: lines.filter_map(entry).take(MAX_ENTRIES).collect(),
        verdict: None,
        made: None,
    }
}

#[cfg(test)]
mod tests;
