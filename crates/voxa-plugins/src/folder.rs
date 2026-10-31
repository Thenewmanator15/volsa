//! A folder offered as a plugin, checked where it is before anything is copied: its
//! description, and every file the description lists, by name, kind, size and SHA-256.
//!
//! Nothing in the folder but `plugin.json` and the files it lists is read. A name that would
//! lead out of the folder, a link, or a file that is not the one listed is refused, and the
//! sentence never names the file: the names are kept with the error for the log alone.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use voxa_models::{Supported, file_sum, is_plain_name};

use crate::{
    DescribeError, Description, NetworkError, NetworkFacts, read_description, read_network,
};

#[cfg(test)]
mod tests;

/// The name of a plugin's description in its folder.
pub const DESCRIPTION: &str = "plugin.json";
/// The longest a description may be, in bytes: 1 MiB.
const MOST_DESCRIPTION: u64 = 1 << 20;
/// What Windows marks a link or another reparse point with among a file's attributes.
#[cfg(windows)]
const REPARSE_POINT: u32 = 0x400;

/// A folder offered as a plugin, every check passed.
#[derive(Debug, Clone, PartialEq)]
pub struct Offered {
    /// Its description.
    pub description: Description,
    /// The SHA-256 of its `plugin.json`, in lower-case hex: it names this plugin exactly.
    pub sum: String,
    /// Where the folder is.
    pub folder: PathBuf,
    /// What its network takes and gives, as its file says.
    pub network: NetworkFacts,
}

/// Why a folder is not taken as a plugin. Each sentence is fit to be shown; the names some
/// carry are for the log only.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// There is no folder at the place given.
    #[error("There is no folder at the place given.")]
    NotFound,
    /// The folder holds no `plugin.json`, or one too long to be a description.
    #[error("That folder holds no plugin.json, or one too long to be a description.")]
    NoDescription,
    /// The description breaks a rule.
    #[error(transparent)]
    Describe(#[from] DescribeError),
    /// A listed name would lead out of the folder.
    #[error("The description names a file outside its folder.")]
    NotPlain(String),
    /// A file is listed twice, perhaps in two cases of its letters.
    #[error("The description lists one file twice.")]
    Twice(String),
    /// A listed file is not there.
    #[error("The plugin is missing a file its description lists.")]
    Missing(String),
    /// A listed file is a link, a folder, or anything else that is not a plain file.
    #[error("A file the description lists is a link or not a plain file.")]
    NotAFile(String),
    /// A listed file is not of the size listed.
    #[error("A file is not the one the description lists, so it may be damaged.")]
    WrongSize(String),
    /// A listed file is not of the sum listed.
    #[error("A file is not the one the description lists, so it may be damaged.")]
    WrongSum(String),
    /// A file is over 2 GiB, or all of them together are over 4 GiB.
    #[error("The plugin's files are larger than Voxa takes: 2 GiB a file, 4 GiB together.")]
    TooLarge,
    /// The network file breaks a rule.
    #[error(transparent)]
    Network(#[from] NetworkError),
}

/// How large a plugin's files may be, in bytes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The most one file may hold.
    pub(crate) file: u64,
    /// The most all of them together may hold.
    pub(crate) total: u64,
}

impl Limits {
    /// Voxa's own: 2 GiB a file, 4 GiB together.
    const REAL: Limits = Limits {
        file: 1 << 31,
        total: 1 << 32,
    };
}

/// Checks the folder offered as a plugin, where it is: its description, every file the
/// description lists, then the network file's structure. `supported` is what the runtime runs.
///
/// # Errors
///
/// [`PluginError`] says the first thing found wrong.
pub fn check_folder(folder: &Path, supported: Supported) -> Result<Offered, PluginError> {
    check_with(folder, supported, Limits::REAL)
}

/// [`check_folder`] with the limits given, so that a test can pass them with small files.
pub(crate) fn check_with(
    folder: &Path,
    supported: Supported,
    limits: Limits,
) -> Result<Offered, PluginError> {
    if !folder.is_dir() {
        return Err(PluginError::NotFound);
    }
    let text = read_limited(&folder.join(DESCRIPTION), MOST_DESCRIPTION)
        .ok_or(PluginError::NoDescription)?;
    let description = read_description(&text)?;
    check_network_named(&description)?;
    check_names(&description, limits)?;
    for listed in &description.files {
        let path = folder.join(&listed.name);
        let missing = || PluginError::Missing(listed.name.clone());
        let metadata = fs::symlink_metadata(&path).map_err(|_| missing())?;
        if !plain_file(&metadata) {
            return Err(PluginError::NotAFile(listed.name.clone()));
        }
        if metadata.len() != listed.bytes {
            return Err(PluginError::WrongSize(listed.name.clone()));
        }
        let sum = file_sum(&path, limits.file).map_err(|_| missing())?;
        if sum != listed.sha256 {
            return Err(PluginError::WrongSum(listed.name.clone()));
        }
    }
    let network = read_network(
        &folder.join(&description.network),
        &description.files,
        supported,
    )?;
    Ok(Offered {
        description,
        sum: hex(&Sha256::digest(&text)),
        folder: folder.to_owned(),
        network,
    })
}

/// The bytes of a file, if it is there and no longer than `most`.
fn read_limited(path: &Path, most: u64) -> Option<Vec<u8>> {
    let mut text = Vec::new();
    File::open(path)
        .ok()?
        .take(most + 1)
        .read_to_end(&mut text)
        .ok()?;
    (u64::try_from(text.len()).ok()? <= most).then_some(text)
}

/// The network must be one of the listed files, and a `.onnx` file.
fn check_network_named(description: &Description) -> Result<(), PluginError> {
    let network = &description.network;
    let listed = description.files.iter().any(|file| file.name == *network);
    let onnx = network.to_ascii_lowercase().ends_with(".onnx");
    if listed && onnx {
        Ok(())
    } else {
        let why = "must be one of its files, a .onnx file";
        let field = "network".to_owned();
        Err(DescribeError::Field { field, why }.into())
    }
}

/// Every listed name a plain name, none twice whatever the case of its letters, and the sizes
/// within the limits.
fn check_names(description: &Description, limits: Limits) -> Result<(), PluginError> {
    let mut seen = HashSet::new();
    let mut total: u64 = 0;
    for listed in &description.files {
        if !is_plain_name(&listed.name) {
            return Err(PluginError::NotPlain(listed.name.clone()));
        }
        if !seen.insert(listed.name.to_lowercase()) {
            return Err(PluginError::Twice(listed.name.clone()));
        }
        total = total.saturating_add(listed.bytes);
        if listed.bytes > limits.file || total > limits.total {
            return Err(PluginError::TooLarge);
        }
    }
    Ok(())
}

/// Whether what is at a path, as found without following a link, is a plain file: no link,
/// no folder, and on Windows no other reparse point.
fn plain_file(metadata: &Metadata) -> bool {
    let kind = metadata.file_type();
    kind.is_file() && !kind.is_symlink() && !reparse_point(metadata)
}

/// Whether Windows marks the file as a reparse point.
#[cfg(windows)]
fn reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & REPARSE_POINT != 0
}

/// Elsewhere there are no reparse points.
#[cfg(not(windows))]
fn reparse_point(_metadata: &Metadata) -> bool {
    false
}

/// Bytes as lower-case hex digits.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        // Writing to a string cannot fail.
        let _ = write!(text, "{byte:02x}");
        text
    })
}
