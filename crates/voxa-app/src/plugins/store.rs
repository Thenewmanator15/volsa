//! Voxa's own folder of plugins, `plugins` in its settings folder: a copy of each plugin
//! added, in a folder named by the first 16 digits of its description's sum. A copy is added
//! whole or not at all: written under another name first, then put in place, then checked
//! again.

#![allow(
    dead_code,
    reason = "the workers that use it come with the next commit"
)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use voxa_models::Supported;
use voxa_plugins::{DESCRIPTION, Installed, Offered, PluginError, installed};

/// The folder of plugins, in the settings folder.
const FOLDER: &str = "plugins";
/// How many digits of the sum name a copy's folder.
const NAMED_BY: usize = 16;

/// The folder plugins are kept in, beside the kept labels.
pub(crate) fn folder(settings: &Path) -> PathBuf {
    settings.join(FOLDER)
}

/// Every plugin added: each copy read and checked, or its folder's name and why it is not.
pub(crate) fn list(
    folder: &Path,
    supported: Supported,
) -> Vec<Result<Installed, (String, PluginError)>> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut copies: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            name.len() == NAMED_BY && name.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .collect();
    copies.sort();
    copies
        .into_iter()
        .map(|copy| {
            installed(&copy, supported).map_err(|why| {
                let name = copy
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned());
                (name.unwrap_or_default(), why)
            })
        })
        .collect()
}

/// Adds a folder offered: its description and the files it lists, and nothing else, copied
/// into a folder of their own, which is checked again. A plugin already added with the same
/// sum is that plugin. On any failure nothing is left behind.
///
/// # Errors
///
/// [`PluginError`] when the copy could not be made, or does not pass its check.
pub(crate) fn add(
    offered: &Offered,
    folder: &Path,
    supported: Supported,
) -> Result<Installed, PluginError> {
    let named = offered.sum.get(..NAMED_BY).ok_or(PluginError::Changed)?;
    let copy = folder.join(named);
    if copy.is_dir() {
        return installed(&copy, supported);
    }
    let part = folder.join(format!(".adding-{named}"));
    let made = copy_files(offered, folder, &part).and_then(|()| fs::rename(&part, &copy));
    if made.is_err() {
        let _ = fs::remove_dir_all(&part);
        return Err(PluginError::Changed);
    }
    installed(&copy, supported).inspect_err(|_| {
        let _ = fs::remove_dir_all(&copy);
    })
}

/// Copies the description and the listed files into `part`.
fn copy_files(offered: &Offered, folder: &Path, part: &Path) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let _ = fs::remove_dir_all(part);
    fs::create_dir(part)?;
    fs::copy(offered.folder.join(DESCRIPTION), part.join(DESCRIPTION))?;
    for listed in &offered.description.files {
        fs::copy(offered.folder.join(&listed.name), part.join(&listed.name))?;
    }
    Ok(())
}

/// Removes a plugin's copy.
///
/// # Errors
///
/// When the copy could not be removed.
pub(crate) fn remove(copy: &Path) -> io::Result<()> {
    fs::remove_dir_all(copy)
}

/// The plugin already added with the same identity as the folder offered, if one is.
pub(crate) fn same_id<'a>(added: &'a [Installed], offered: &Offered) -> Option<&'a Installed> {
    added
        .iter()
        .find(|plugin| plugin.description.id == offered.description.id)
}
