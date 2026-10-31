//! The model folder: the list that says what it should hold, and the check of every file
//! against that list.

use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

/// The format of the list (`model.json`) that this version of Voxa reads.
pub const MODEL_FORMAT: u32 = 1;
/// The kind of model Voxa runs: the networks of `SynthSeg` 2.0.
pub const KIND_SYNTHSEG: &str = "synthseg-2.0";

/// Most the list may hold, in bytes.
const MAX_LIST_BYTES: u64 = 1 << 20;
/// Most one network file may hold, in bytes: 2 GiB.
const MAX_NETWORK_BYTES: u64 = 1 << 31;
/// How many bytes of a file are read at once when its sum is worked out.
const BLOCK: usize = 1 << 20;

/// One of the six networks of a model folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The standard network: an image to the chance of each structure.
    Standard,
    /// The first robust network: an image to a map of classes.
    Robust1,
    /// The second robust network: tidies a map of classes.
    Robust2,
    /// The third robust network: an image and a map of classes to the chance of each structure.
    Robust3,
    /// The cortex network: an image and the cortex to its areas.
    Cortex,
    /// The scores network: a map of classes to the quality scores.
    Scores,
}

impl Role {
    /// All six, in the order the list names them.
    pub const ALL: [Role; 6] = [
        Role::Standard,
        Role::Robust1,
        Role::Robust2,
        Role::Robust3,
        Role::Cortex,
        Role::Scores,
    ];
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Standard => "standard",
            Role::Robust1 => "robust 1",
            Role::Robust2 => "robust 2",
            Role::Robust3 => "robust 3",
            Role::Cortex => "cortex",
            Role::Scores => "scores",
        })
    }
}

/// Which side of the body a structure lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSide {
    /// The left side.
    Left,
    /// The right side.
    Right,
    /// On the middle line, or on neither side.
    Midline,
}

/// A part the networks tell apart, or an area of the cortex.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Structure {
    /// The part's own number, as a label file written by another program holds it.
    pub label: u32,
    /// The part's name, without its side.
    pub name: String,
    /// Which side it lies on.
    pub side: ModelSide,
    /// The class the first stage gives it; 0 for an area, which has none.
    #[serde(default)]
    pub class: u32,
}

/// What the list says about the networks' outputs. Checked when the list is read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Lists {
    /// The 33 structures, in the networks' order, the first being label 0.
    pub structures: Vec<Structure>,
    /// For each place in `structures`, the place of its mirror image on the other side.
    pub flip: Vec<usize>,
    /// How many classes the first stage tells apart.
    pub stage_classes: usize,
    /// The 69 areas of the cortex, the first being label 0.
    pub areas: Vec<Structure>,
    /// The labels of the cortex on the left and on the right.
    pub cortex: [u32; 2],
    /// For each structure, the score group it counts towards; 0 for none.
    pub score_class: Vec<usize>,
    /// The names of the score groups.
    pub score_groups: Vec<String>,
    /// The side of the cube the networks are given, in voxels.
    #[serde(rename = "box")]
    pub box_side: usize,
}

/// A model folder that has been read and checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// What the list says about the networks' outputs.
    pub lists: Lists,
    /// The SHA-256 of the list file, in lower-case hex: it names this model exactly.
    pub sum: String,
    files: PerRole<PathBuf>,
}

impl Model {
    /// Where the file of this network is. It has been checked, but may since have changed.
    pub fn file(&self, role: Role) -> &Path {
        self.files.get(role)
    }
}

/// Why a folder could not be taken as a model. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// There is no folder at the place given.
    #[error("There is no model folder at the place given.")]
    NotFound,
    /// The folder has no readable list, or the list names a file it should not.
    #[error("That is not a model folder: its list is missing, unreadable or names a bad file.")]
    NotAModel,
    /// The list is in a format this version of Voxa does not read.
    #[error("This model folder is in format {0}, which this version of Voxa cannot read.")]
    Format(u32),
    /// The list is for another kind of model.
    #[error("This model folder is for another kind of model than the one Voxa uses.")]
    Kind,
    /// A network the list names is not there.
    #[error("The model folder is missing the {0} network.")]
    Missing(Role),
    /// A network is not the size the list gives.
    #[error("The {0} network is not the size the list gives, so it may be damaged.")]
    WrongSize(Role),
    /// A network is not the one the list names.
    #[error("The {0} network is not the one the list names, so it may be damaged.")]
    WrongSum(Role),
    /// The list breaks a rule; the words say which.
    #[error("The list of parts in the model folder breaks a rule: {0}.")]
    Lists(&'static str),
}

/// One thing of each role, as the list holds them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PerRole<T> {
    standard: T,
    robust_1: T,
    robust_2: T,
    robust_3: T,
    cortex: T,
    scores: T,
}

impl<T> PerRole<T> {
    fn get(&self, role: Role) -> &T {
        match role {
            Role::Standard => &self.standard,
            Role::Robust1 => &self.robust_1,
            Role::Robust2 => &self.robust_2,
            Role::Robust3 => &self.robust_3,
            Role::Cortex => &self.cortex,
            Role::Scores => &self.scores,
        }
    }

    fn map<U>(&self, change: impl Fn(&T) -> U) -> PerRole<U> {
        PerRole {
            standard: change(&self.standard),
            robust_1: change(&self.robust_1),
            robust_2: change(&self.robust_2),
            robust_3: change(&self.robust_3),
            cortex: change(&self.cortex),
            scores: change(&self.scores),
        }
    }
}

/// What the list says of one network file.
#[derive(Debug, Deserialize)]
struct Entry {
    name: String,
    bytes: u64,
    sha256: String,
}

/// The two entries of the list that are read first, so that a list in a format or of a kind
/// this version does not know is said so, whatever else it holds. A list with no kind is of
/// a kind Voxa does not use.
#[derive(Deserialize)]
struct Head {
    format: u32,
    #[serde(default)]
    kind: String,
}

/// The rest of the list that Voxa needs. What is there for people (`made`, `checked`) is not.
#[derive(Deserialize)]
struct Body {
    files: PerRole<Entry>,
    #[serde(flatten)]
    lists: Lists,
}

/// Finds the model in `folder`, reads its list and checks every network file against it:
/// that it is there, of the size the list gives and of the sum the list gives.
///
/// # Errors
///
/// [`ModelError`] says the first thing found wrong, in words fit to be shown.
pub fn read_model(folder: &Path) -> Result<Model, ModelError> {
    if !folder.is_dir() {
        return Err(ModelError::NotFound);
    }
    let text = read_list(folder)?;
    let head: Head = serde_json::from_slice(&text).map_err(|_| ModelError::NotAModel)?;
    if head.format != MODEL_FORMAT {
        return Err(ModelError::Format(head.format));
    }
    if head.kind != KIND_SYNTHSEG {
        return Err(ModelError::Kind);
    }
    let Body { files, lists } = serde_json::from_slice(&text).map_err(|_| ModelError::NotAModel)?;
    if !Role::ALL
        .into_iter()
        .all(|role| is_plain_name(&files.get(role).name))
    {
        return Err(ModelError::NotAModel);
    }
    check_lists(&lists)?;
    let paths = files.map(|entry| folder.join(&entry.name));
    for role in Role::ALL {
        check_file(paths.get(role), files.get(role), MAX_NETWORK_BYTES, role)?;
    }
    Ok(Model {
        lists,
        sum: hex(&Sha256::digest(&text)),
        files: paths,
    })
}

/// The bytes of the list, if there is a list and it is not too long.
fn read_list(folder: &Path) -> Result<Vec<u8>, ModelError> {
    let file = File::open(folder.join("model.json")).map_err(|_| ModelError::NotAModel)?;
    let mut text = Vec::new();
    file.take(MAX_LIST_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|_| ModelError::NotAModel)?;
    if u64::try_from(text.len()).map_or(true, |length| length > MAX_LIST_BYTES) {
        return Err(ModelError::NotAModel);
    }
    Ok(text)
}

/// Whether a name is that of a file in the folder itself: not empty, no separator, no `..`,
/// nothing that would lead out of the folder or to another stream of a file.
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && !name.contains("..")
        && !name.contains(['/', '\\', ':', '\0'])
}

/// Checks one network file against its entry: it is there, it is no larger than `limit`, it
/// has the size and the sum the list gives. A file that cannot be read to its end counts as
/// not matching.
fn check_file(path: &Path, entry: &Entry, limit: u64, role: Role) -> Result<(), ModelError> {
    let file = File::open(path).map_err(|_| ModelError::Missing(role))?;
    let metadata = file.metadata().map_err(|_| ModelError::Missing(role))?;
    if !metadata.is_file() {
        return Err(ModelError::Missing(role));
    }
    if metadata.len() != entry.bytes || entry.bytes > limit {
        return Err(ModelError::WrongSize(role));
    }
    let sum = sum_of(file.take(limit)).map_err(|_| ModelError::WrongSum(role))?;
    if sum.eq_ignore_ascii_case(&entry.sha256) {
        Ok(())
    } else {
        Err(ModelError::WrongSum(role))
    }
}

/// The SHA-256 of the first `limit` bytes of a file, in lower-case hex: of the whole file when
/// it is no longer. It reads a block at a time, so a file of any size is summed in the same
/// small amount of memory.
///
/// # Errors
///
/// When the file cannot be opened or read to its end.
pub fn file_sum(path: &Path, limit: u64) -> io::Result<String> {
    sum_of(File::open(path)?.take(limit))
}

/// The SHA-256 of everything `source` gives, in lower-case hex. It reads a block at a time,
/// so a file of any size is summed in the same small amount of memory.
fn sum_of(mut source: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut block = vec![0_u8; BLOCK];
    loop {
        match source.read(&mut block) {
            Ok(0) => break,
            Ok(count) => hasher.update(block.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(hex(&hasher.finalize()))
}

/// Bytes as lower-case hex digits.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        // Writing to a string cannot fail.
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// Checks the lists against the rules the networks' outputs follow, in this order. The error
/// holds the words of the first rule that fails.
fn check_lists(lists: &Lists) -> Result<(), ModelError> {
    let (structures, flip) = (&lists.structures, &lists.flip);
    let mut labels: Vec<u32> = structures.iter().map(|part| part.label).collect();
    labels.sort_unstable();
    labels.dedup();
    let starts_at_zero = |parts: &[Structure]| parts.first().is_some_and(|part| part.label == 0);
    let has_label = |label: &u32| structures.iter().any(|part| part.label == *label);
    // A list that is its own undoing is a swap of each place from 0 to 32 with one place.
    let swaps = |(place, &other): (usize, &usize)| flip.get(other) == Some(&place);
    let rules = [
        (structures.len() == 33, "there must be 33 structures"),
        (
            starts_at_zero(structures),
            "the first structure must be label 0",
        ),
        (labels.len() == 33, "no two structures may share a label"),
        (flip.len() == 33, "the left-right swap must have 33 places"),
        (
            flip.iter().enumerate().all(swaps),
            "the left-right swap must hold each place from 0 to 32 once and undo itself",
        ),
        (
            (2..=16).contains(&lists.stage_classes),
            "the number of stage classes must be from 2 to 16",
        ),
        (lists.areas.len() == 69, "there must be 69 areas"),
        (
            starts_at_zero(&lists.areas),
            "the first area must be label 0",
        ),
        (
            lists.cortex.iter().all(has_label),
            "both cortex labels must be among the structures",
        ),
        (
            lists.score_class.len() == 33,
            "there must be one score class for each of the 33 structures",
        ),
        (
            lists
                .score_class
                .iter()
                .all(|&class| class <= lists.score_groups.len()),
            "no score class may be beyond the number of score groups",
        ),
        (
            lists.box_side.is_multiple_of(32) && (32..=512).contains(&lists.box_side),
            "the box must be a multiple of 32 from 32 to 512",
        ),
    ];
    match rules.into_iter().find(|&(holds, _)| !holds) {
        Some((_, why)) => Err(ModelError::Lists(why)),
        None => Ok(()),
    }
}
