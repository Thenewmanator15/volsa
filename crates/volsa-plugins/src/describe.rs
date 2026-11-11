//! A plugin's description, `plugin.json`, in format 1: what the plugin is, where it came
//! from, what scans it accepts, how a scan is prepared for its network, and what the network
//! gives back.
//!
//! Every setting that changes the answer must be stated: none has a default, and a description
//! that leaves one out, or holds a field this version does not know, is refused with the field
//! named. A description's words are data: Volsa shows them, inside the frame that says the
//! plugin was added by the person, and never follows them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use volsa_segment::Toward;

use fields::Obj;

mod fields;
mod input;
mod output;

pub use output::{
    AlignOutput, FieldUnit, Gives, LabelOutput, Moves, Name, NumberOutput, Output, Side,
    ValueOutput,
};
#[cfg(test)]
mod refusal_tests;
#[cfg(test)]
pub(crate) mod tests;
mod text;

/// The format of `plugin.json` that this version of Volsa reads.
pub const PLUGIN_FORMAT: u32 = 1;

/// A plugin's description, read and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Description {
    /// A stable identity its maker chose: 3 to 64 of `a-z`, `0-9`, dots and dashes.
    pub id: String,
    /// Its name, as it is shown.
    pub name: String,
    /// Its version, as it is shown.
    pub version: String,
    /// Who made it, in its maker's words.
    pub made_by: String,
    /// Where it came from, in its maker's words: shown as text, never as a link.
    pub source: String,
    /// Its licence, in its maker's words.
    pub licence: String,
    /// One sentence of what it does.
    pub about: String,
    /// Up to eight sentences its maker wants shown wherever its results are.
    pub limits: Vec<String>,
    /// What it gives back.
    pub kind: Kind,
    /// Every file of the plugin, with its size and its SHA-256.
    pub files: Vec<Listed>,
    /// The name of the network's file, among `files`.
    pub network: String,
    /// What scans it accepts.
    pub accepts: Accepts,
    /// How a scan is prepared for the network.
    pub input: Input,
    /// What the network gives, and what it means.
    pub output: Output,
}

/// What a plugin gives back: one routine of Volsa's for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A label for each voxel, with names: `label-map`.
    LabelMap,
    /// One number: `number`.
    Number,
    /// A number for each voxel: `value-map`.
    ValueMap,
    /// How one scan lies on another: `alignment`.
    Alignment,
}

impl Kind {
    /// What a plugin of this kind gives, as a sentence says it.
    pub fn words(self) -> &'static str {
        match self {
            Kind::LabelMap => "a label map",
            Kind::Number => "one number",
            Kind::ValueMap => "a value map",
            Kind::Alignment => "an alignment",
        }
    }
}

/// One file of a plugin, as its description lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its name in the plugin's folder.
    pub name: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// Its SHA-256, as 64 lower-case hexadecimal digits.
    pub sha256: String,
}

/// What scans a plugin accepts.
#[derive(Debug, Clone, PartialEq)]
pub struct Accepts {
    /// The DICOM modalities, in capitals, such as `MR`: at least one.
    pub modality: Vec<String>,
    /// The contrasts.
    pub contrast: Contrasts,
    /// The part of the body, in its maker's words; empty when not said. Shown only.
    pub body_part: String,
    /// The widest voxel spacing it was made for, in millimetres, if its maker says.
    pub thickest_mm: Option<f64>,
}

/// The contrasts a plugin accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contrasts {
    /// Any contrast.
    Any,
    /// These, and no other.
    Only(Vec<Contrast>),
}

/// A contrast, by the words a scanner writes in a series' description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Contrast {
    /// T1-weighted.
    T1,
    /// A 3D T1-weighted sequence, such as MPRAGE: a kind of T1.
    T1In3d,
    /// T2-weighted.
    T2,
    /// Proton density.
    Pd,
    /// FLAIR.
    Flair,
    /// T2*-weighted.
    T2Star,
    /// Diffusion-weighted.
    Diffusion,
    /// A map of the apparent diffusion coefficient.
    AdcMap,
    /// Susceptibility-weighted.
    Swi,
    /// STIR.
    Stir,
    /// Time-of-flight angiography.
    Tof,
}

impl Contrast {
    /// Every contrast, in the order the words are listed.
    pub const ALL: [Contrast; 11] = [
        Contrast::T1,
        Contrast::T1In3d,
        Contrast::T2,
        Contrast::Pd,
        Contrast::Flair,
        Contrast::T2Star,
        Contrast::Diffusion,
        Contrast::AdcMap,
        Contrast::Swi,
        Contrast::Stir,
        Contrast::Tof,
    ];

    /// What it is called, as a plain name and a description say it.
    pub fn word(self) -> &'static str {
        match self {
            Contrast::T1 => "T1",
            Contrast::T1In3d => "3D T1",
            Contrast::T2 => "T2",
            Contrast::Pd => "PD",
            Contrast::Flair => "FLAIR",
            Contrast::T2Star => "T2*",
            Contrast::Diffusion => "Diffusion",
            Contrast::AdcMap => "ADC map",
            Contrast::Swi => "SWI",
            Contrast::Stir => "STIR",
            Contrast::Tof => "Angiography (TOF)",
        }
    }

    /// The contrast a description names by this word: its own word, or `TOF`.
    pub fn of_word(word: &str) -> Option<Self> {
        if word == "TOF" {
            return Some(Contrast::Tof);
        }
        Contrast::ALL
            .into_iter()
            .find(|contrast| contrast.word() == word)
    }
}

/// How a scan is prepared for the network.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    /// The distance between voxels along each spatial dimension of the input, in the
    /// tensor's order, in millimetres.
    pub spacing_mm: [f64; 3],
    /// The direction in the patient each spatial dimension of the input points to, in the
    /// tensor's order, the last changing fastest.
    pub axes: [Toward; 3],
    /// Where the input's one channel lies.
    pub layout: Layout,
    /// What size of input the network is given.
    pub fit: Fit,
    /// What lies outside the scan.
    pub outside: Outside,
    /// How the brightness is scaled.
    pub scaling: Scaling,
}

/// Where a tensor's one channel lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    /// `[1, 1, d0, d1, d2]`.
    ChannelsFirst,
    /// `[1, d0, d1, d2, 1]`.
    ChannelsLast,
}

/// What size of input the network is given: always a fixed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// One box of this size, centred on the scan.
    Whole([usize; 3]),
    /// Blocks of this size covering the scan, `step` voxels apart.
    Blocks {
        /// The size of a block.
        size: [usize; 3],
        /// How far apart blocks start, from 1 to the block's size.
        step: [usize; 3],
    },
}

/// What a voxel outside the scan is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outside {
    /// The scan's lowest value, before scaling.
    Lowest,
    /// 0, after scaling.
    Zero,
}

/// How the brightness is scaled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scaling {
    /// The `low`-th percentile becomes 0 and the `high`-th 1, everything held between.
    Percentiles {
        /// The percentile that becomes 0.
        low: f64,
        /// The percentile that becomes 1.
        high: f64,
        /// Over which voxels.
        over: Over,
    },
    /// The mean taken away and the rest divided by the standard deviation.
    ZScore {
        /// Which voxels the two are worked out over.
        voxels: Voxels,
        /// Over which voxels.
        over: Over,
    },
    /// Values held within `clip`, then `(x - subtract) / divide`.
    Fixed {
        /// The lowest and the highest value kept.
        clip: [f64; 2],
        /// What is taken away.
        subtract: f64,
        /// What the rest is divided by: never 0.
        divide: f64,
    },
    /// The scan's own values: the model scales inside itself.
    None,
}

/// Over which voxels statistics are worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Every voxel of the scan at the plugin's spacing, before any box.
    Scan,
    /// The whole box, surroundings included.
    Grid,
}

/// Which voxels a z-score is worked out over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voxels {
    /// All of them.
    All,
    /// Those above 0.
    AboveZero,
}

/// Why a description was refused. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescribeError {
    /// It is not JSON, or not an object, or has no format number.
    #[error("This folder's plugin.json is not a description Volsa can read.")]
    NotJson,
    /// It is in a format this version does not read.
    #[error("This plugin is in format {0}, which this version of Volsa cannot read.")]
    Format(u32),
    /// A field breaks a rule: its path, and how.
    #[error("The description's {field} {why}.")]
    Field {
        /// Where the field is, its keys joined by dots.
        field: String,
        /// How it breaks a rule, to follow its name in a sentence.
        why: &'static str,
    },
    /// It is of a kind this version cannot run yet.
    #[error("This version of Volsa cannot run plugins that give {} yet.", .0.words())]
    NotYet(Kind),
}

impl Description {
    /// Whether this version of Volsa can run a plugin of this kind: today, a label map.
    ///
    /// # Errors
    ///
    /// [`DescribeError::NotYet`] for every other kind.
    pub fn runnable(&self) -> Result<(), DescribeError> {
        match self.kind {
            Kind::LabelMap => Ok(()),
            other => Err(DescribeError::NotYet(other)),
        }
    }
}

/// Reads a description from the bytes of `plugin.json`, checking every rule of format 1.
///
/// # Errors
///
/// [`DescribeError`] says the first thing found wrong, naming the field.
pub fn read_description(text: &[u8]) -> Result<Description, DescribeError> {
    let value: Value = serde_json::from_slice(text).map_err(|_| DescribeError::NotJson)?;
    let top = Obj::top(&value)?;
    let format = top.get("format").and_then(Value::as_u64);
    let format = format.and_then(|format| u32::try_from(format).ok());
    let format = format.ok_or(DescribeError::NotJson)?;
    if format != PLUGIN_FORMAT {
        return Err(DescribeError::Format(format));
    }
    let kind = match top.word("kind")? {
        "label-map" => Kind::LabelMap,
        "number" => Kind::Number,
        "value-map" => Kind::ValueMap,
        "alignment" => Kind::Alignment,
        _ => return Err(top.wrong("kind", "is not one Volsa knows")),
    };
    let description = Description {
        id: fields::id(&top)?,
        name: top.text("name", 60)?,
        version: top.text("version", 20)?,
        made_by: top.text("made_by", 80)?,
        source: top.text("source", 200)?,
        licence: top.text("licence", 200)?,
        about: top.text("about", 300)?,
        limits: fields::limits(&top)?,
        kind,
        files: fields::files(&top)?,
        network: top.text("network", 255)?,
        accepts: fields::accepts(&top)?,
        input: input::input(&top)?,
        output: output::output(&top, kind)?,
    };
    top.only(&TOP_FIELDS)?;
    Ok(description)
}

/// Every field a description of format 1 holds at its top.
const TOP_FIELDS: [&str; 15] = [
    "format", "kind", "id", "name", "version", "made_by", "source", "licence", "about", "limits",
    "files", "network", "accepts", "input", "output",
];
