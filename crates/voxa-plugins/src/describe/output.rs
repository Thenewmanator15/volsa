//! The `output` part of a description, by the kind of plugin: what the network gives and what
//! it means.

use std::collections::HashSet;

use super::fields::Obj;
use super::input::pair;
use serde::{Deserialize, Serialize};

use super::{DescribeError, Kind};

/// The longest a label's name or group is kept, as a table keeps it.
const NAME_CHARACTERS: usize = voxa_labels::MAX_NAME;

/// Reads `output` for a plugin of this kind.
pub(super) fn output(top: &Obj, kind: Kind) -> Result<Output, DescribeError> {
    let group = top.object("output")?;
    Ok(match kind {
        Kind::LabelMap => Output::Labels(labels(&group)?),
        Kind::Number => Output::Number(number(&group)?),
        Kind::ValueMap => Output::ValueMap(value_map(&group)?),
        Kind::Alignment => Output::Alignment(alignment(&group)?),
    })
}

/// The name of the output to read, when there is one.
fn from(group: &Obj) -> Result<Option<String>, DescribeError> {
    group.maybe_text("from", 200)
}

/// A label map's output.
fn labels(group: &Obj) -> Result<LabelOutput, DescribeError> {
    let names = names(group)?;
    let known: HashSet<u16> = names.iter().map(|name| name.id).collect();
    let (gives, keys): (Gives, &[&str]) = match group.word("gives")? {
        "scores" => (scores(group, &known)?, &["channels"]),
        "chance" => (chance(group, &known)?, &["label", "threshold"]),
        "labels" => (Gives::Labels, &[]),
        _ => return Err(group.wrong("gives", "is not one Voxa knows")),
    };
    let from = from(group)?;
    let keep_largest_piece = group.flag("keep_largest_piece")?;
    let mut known = vec!["gives", "from", "keep_largest_piece", "names"];
    known.extend_from_slice(keys);
    group.only(&known)?;
    Ok(LabelOutput {
        gives,
        from,
        keep_largest_piece,
        names,
    })
}

/// A label: a whole number from 1 to 65 535.
fn label(value: &serde_json::Value) -> Option<u16> {
    value.as_u64().and_then(|label| u16::try_from(label).ok())
}

/// The channels of a scores output, each 0 or a named label.
fn scores(group: &Obj, named: &HashSet<u16>) -> Result<Gives, DescribeError> {
    let list = group.list("channels")?;
    let channels: Option<Vec<u16>> = list.iter().map(label).collect();
    let channels = channels
        .filter(|channels| !channels.is_empty())
        .ok_or_else(|| group.wrong("channels", "must be a list of labels from 0 to 65535"))?;
    if channels.iter().any(|&id| id != 0 && !named.contains(&id)) {
        return Err(group.wrong("channels", "names a label with no name"));
    }
    Ok(Gives::Scores { channels })
}

/// The label and threshold of a chance output.
fn chance(group: &Obj, named: &HashSet<u16>) -> Result<Gives, DescribeError> {
    let label = label(group.value("label")?)
        .filter(|id| named.contains(id))
        .ok_or_else(|| group.wrong("label", "names a label with no name"))?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a chance's threshold is a single"
    )]
    let threshold = group.number("threshold")? as f32;
    Ok(Gives::Chance { label, threshold })
}

/// The names of the labels: at least one, ids from 1 to 65 535 given once, and no name and
/// side given twice.
fn names(group: &Obj) -> Result<Vec<Name>, DescribeError> {
    let entries = group.objects("names")?;
    if entries.is_empty() {
        return Err(group.wrong("names", "is empty"));
    }
    let mut ids = HashSet::new();
    let mut seen = HashSet::new();
    let mut names = Vec::with_capacity(entries.len());
    for entry in &entries {
        let id = label(entry.value("id")?)
            .filter(|id| *id != 0)
            .ok_or_else(|| entry.wrong("id", "must be a whole number from 1 to 65535"))?;
        let name = entry.text("name", NAME_CHARACTERS)?;
        let side = match entry.word("side")? {
            "left" => Side::Left,
            "right" => Side::Right,
            "midline" => Side::Midline,
            _ => return Err(entry.wrong("side", "is not one Voxa knows")),
        };
        let group_name = entry.optional_text("group", NAME_CHARACTERS)?;
        let colour = colour(entry)?;
        entry.only(&["id", "name", "side", "group", "colour"])?;
        if !ids.insert(id) {
            return Err(group.wrong("names", "gives one id twice"));
        }
        if !seen.insert((name.clone(), side)) {
            return Err(group.wrong("names", "gives one name and side twice"));
        }
        names.push(Name {
            id,
            name,
            side,
            group: group_name,
            colour,
        });
    }
    Ok(names)
}

/// A colour, if one is given: three whole numbers from 0 to 255.
fn colour(entry: &Obj) -> Result<Option<[u8; 3]>, DescribeError> {
    let Some(value) = entry.get("colour") else {
        return Ok(None);
    };
    let part = |value: &serde_json::Value| value.as_u64().and_then(|part| u8::try_from(part).ok());
    let parts = value
        .as_array()
        .map(|list| list.iter().map(part).collect::<Option<Vec<u8>>>());
    match parts.flatten().as_deref() {
        Some(&[red, green, blue]) => Ok(Some([red, green, blue])),
        _ => Err(entry.wrong("colour", "must be three whole numbers from 0 to 255")),
    }
}

/// One number's output.
fn number(group: &Obj) -> Result<NumberOutput, DescribeError> {
    let name = group.text("name", 60)?;
    let unit = group.optional_text("unit", 20)?;
    let decimals = u8::try_from(group.whole("decimals")?)
        .ok()
        .filter(|places| *places <= 6)
        .ok_or_else(|| group.wrong("decimals", "must be a whole number from 0 to 6"))?;
    let range = match group.get("range") {
        None => None,
        Some(_) => Some(pair(group, "range")?),
    };
    let from = from(group)?;
    group.only(&["name", "unit", "decimals", "range", "from"])?;
    Ok(NumberOutput {
        name,
        unit,
        decimals,
        range,
        from,
    })
}

/// A value map's output.
fn value_map(group: &Obj) -> Result<ValueOutput, DescribeError> {
    let unit = group.optional_text("unit", 20)?;
    let range = pair(group, "range")?;
    let from = from(group)?;
    group.only(&["unit", "range", "from"])?;
    Ok(ValueOutput { unit, range, from })
}

/// An alignment's output.
fn alignment(group: &Obj) -> Result<AlignOutput, DescribeError> {
    let (gives, keys): (Moves, &[&str]) = match group.word("gives")? {
        "field" => {
            let unit = match group.word("unit")? {
                "mm" => FieldUnit::Mm,
                "voxels" => FieldUnit::Voxels,
                _ => return Err(group.wrong("unit", "is not one Voxa knows")),
            };
            (Moves::Field { unit }, &["gives", "unit", "from"])
        }
        "matrix" => (Moves::Matrix, &["gives", "from"]),
        _ => return Err(group.wrong("gives", "is not one Voxa knows")),
    };
    let from = from(group)?;
    group.only(keys)?;
    Ok(AlignOutput { gives, from })
}

/// What the network gives, by the kind of plugin.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// A label map.
    Labels(LabelOutput),
    /// One number.
    Number(NumberOutput),
    /// A value map.
    ValueMap(ValueOutput),
    /// An alignment.
    Alignment(AlignOutput),
}

/// What a label-map network gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOutput {
    /// What its answer holds.
    pub gives: Gives,
    /// The output to read, when the network has several.
    pub from: Option<String>,
    /// Whether only the largest connected piece of everything labelled is kept.
    pub keep_largest_piece: bool,
    /// Every label that can appear, with its name.
    pub names: Vec<Name>,
}

/// What a label-map network's answer holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Gives {
    /// A score for each channel; each voxel takes the label of its highest.
    Scores {
        /// The label of each channel, 0 for nothing.
        channels: Vec<u16>,
    },
    /// One chance; a voxel above `threshold` takes `label`.
    Chance {
        /// The label given.
        label: u16,
        /// The chance a voxel must pass.
        threshold: f32,
    },
    /// A label for each voxel.
    Labels,
}

/// A label's name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Name {
    /// The label, from 1 to 65 535.
    pub id: u16,
    /// Its name.
    pub name: String,
    /// The side it lies on.
    pub side: Side,
    /// The group it is listed under; may be empty.
    pub group: String,
    /// Its colour, if its maker gives one.
    pub colour: Option<[u8; 3]>,
}

/// The side a labelled part lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// The patient's left.
    Left,
    /// The patient's right.
    Right,
    /// On the middle line, or on neither side.
    Midline,
}

impl From<Side> for voxa_labels::Side {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => voxa_labels::Side::Left,
            Side::Right => voxa_labels::Side::Right,
            Side::Midline => voxa_labels::Side::Midline,
        }
    }
}

/// What a network that gives one number gives.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberOutput {
    /// What the number is called.
    pub name: String,
    /// Its unit; may be empty.
    pub unit: String,
    /// How many places it is shown to.
    pub decimals: u8,
    /// The range its maker says it is meaningful in.
    pub range: Option<[f64; 2]>,
    /// The output to read, when the network has several.
    pub from: Option<String>,
}

/// What a network that gives a value map gives.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueOutput {
    /// The values' unit; may be empty.
    pub unit: String,
    /// The range drawn.
    pub range: [f64; 2],
    /// The output to read, when the network has several.
    pub from: Option<String>,
}

/// What an alignment network gives.
#[derive(Debug, Clone, PartialEq)]
pub struct AlignOutput {
    /// How it says the moving scan moves.
    pub gives: Moves,
    /// The output to read, when the network has several.
    pub from: Option<String>,
}

/// How an alignment says the moving scan moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moves {
    /// A displacement for each voxel, `[1, 3, d0, d1, d2]`.
    Field {
        /// The displacements' unit.
        unit: FieldUnit,
    },
    /// One 4 x 4 matrix.
    Matrix,
}

/// The unit of a field of displacements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldUnit {
    /// Millimetres.
    Mm,
    /// Voxels of the plugin's grid.
    Voxels,
}
