//! The packages built into the program, as `cargo metadata` describes them, and the licence
//! and notice files in their folders.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Where a package on crates.io comes from, as `cargo metadata` writes it.
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";
/// How the name of a licence or notice file starts, in capitals.
const LICENCE_FILES: [&str; 5] = ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"];

/// What the notices say of one package, and where its files are.
pub struct Package {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// Its licence, as an SPDX expression, when its manifest gives one.
    pub licence: Option<String>,
    /// Its repository, when its manifest gives one.
    pub repository: Option<String>,
    /// Its home page, when its manifest gives one.
    pub homepage: Option<String>,
    /// Its authors, without their addresses.
    pub authors: Vec<String>,
    /// The folder that holds its source.
    pub folder: PathBuf,
    /// The file its manifest names as its licence, if it names one.
    pub licence_file: Option<String>,
    /// Whether it comes from crates.io.
    pub from_crates_io: bool,
}

impl Package {
    /// Its name and version, as the notices write them.
    pub fn heading(&self) -> String {
        format!("{} {}", self.name, self.version)
    }
}

/// The packages of `metadata` that `shipped` names, other than the workspace's own, sorted
/// by name and then version. A package `shipped` names that `metadata` does not describe is
/// an error: its licence would be left out.
pub fn packages(
    metadata: &Value,
    shipped: &BTreeSet<(String, String)>,
) -> Result<Vec<Package>, String> {
    let own = metadata["workspace_members"].as_array().cloned();
    let own = own.unwrap_or_default();
    let described = metadata["packages"].as_array();
    let described = described.ok_or("cargo metadata described no packages")?;
    let mut found = Vec::new();
    let mut ours = BTreeSet::new();
    for each in described {
        let package = package(each)?;
        let key = (package.name.clone(), package.version.clone());
        if own.contains(&each["id"]) {
            ours.insert(key);
        } else if shipped.contains(&key) {
            found.push(package);
        }
    }
    found.sort_by(|one, other| (&one.name, &one.version).cmp(&(&other.name, &other.version)));
    let is_found = |(name, version): &&(String, String)| {
        ours.contains(&(name.clone(), version.clone()))
            || found
                .iter()
                .any(|p| &p.name == name && &p.version == version)
    };
    if let Some((name, version)) = shipped.iter().find(|key| !is_found(key)) {
        let why = "so its licence cannot be found";
        return Err(format!(
            "{name} {version} is built into Voxa, but cargo metadata does not describe it, {why}"
        ));
    }
    Ok(found)
}

/// One package, from its entry in the metadata.
fn package(described: &Value) -> Result<Package, String> {
    let text = |key: &str| described[key].as_str().map(str::to_owned);
    let name = text("name").ok_or("cargo metadata described a package with no name")?;
    let lacks = |what: &str| format!("cargo metadata gave {name} no {what}");
    let version = text("version").ok_or_else(|| lacks("version"))?;
    let manifest = text("manifest_path").ok_or_else(|| lacks("manifest"))?;
    let folder = Path::new(&manifest).parent().map(Path::to_path_buf);
    let folder = folder.ok_or_else(|| lacks("folder"))?;
    let authors = described["authors"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let authors = authors
        .iter()
        .filter_map(Value::as_str)
        .filter_map(without_address);
    Ok(Package {
        licence: text("license"),
        repository: text("repository"),
        homepage: text("homepage"),
        authors: authors.collect(),
        folder,
        licence_file: text("license_file"),
        from_crates_io: described["source"].as_str() == Some(CRATES_IO),
        name,
        version,
    })
}

/// An author as a manifest names them, without the address it may give in angle brackets.
fn without_address(author: &str) -> Option<String> {
    let name = author.split('<').next().unwrap_or_default().trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Whether a file is a licence or a notice, by how its name starts, in any case.
pub fn is_licence_file(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    LICENCE_FILES.iter().any(|start| name.starts_with(start))
}

/// The licence and notice files directly in a package's folder, with the file its manifest
/// names as its licence, sorted by name.
pub fn licence_files(package: &Package) -> Result<Vec<String>, String> {
    let folder = &package.folder;
    let unread = |error| format!("could not list {}: {error}", folder.display());
    let mut names = Vec::new();
    for entry in fs::read_dir(folder).map_err(unread)? {
        let entry = entry.map_err(unread)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_licence_file(&name) && entry.path().is_file() {
            names.push(name);
        }
    }
    if let Some(own) = &package.licence_file {
        let own = own.replace('\\', "/");
        if !names.contains(&own) {
            names.push(own);
        }
    }
    names.sort();
    Ok(names)
}

/// The names of the files in a folder that end with `ending`, sorted.
pub fn files_ending(folder: &Path, ending: &str) -> Result<Vec<String>, String> {
    let unread = |error| format!("could not list {}: {error}", folder.display());
    let mut names = Vec::new();
    for entry in fs::read_dir(folder).map_err(unread)? {
        let name = entry.map_err(unread)?.file_name();
        let name = name.to_string_lossy();
        if name.to_ascii_lowercase().ends_with(ending) {
            names.push(name.into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// The text of a file, tidied. Bytes that are not UTF-8 are read as the replacement character.
pub fn read(file: &Path) -> Result<String, String> {
    let bytes =
        fs::read(file).map_err(|error| format!("could not read {}: {error}", file.display()))?;
    Ok(super::tidy(&String::from_utf8_lossy(&bytes)))
}
