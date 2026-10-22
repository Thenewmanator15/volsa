//! `cargo xtask dist [--runtime FOLDER] [--local]` makes the package a release of Voxa offers
//! for 64-bit Windows: `target/dist/voxa-VERSION-windows-x64.zip`, which holds one folder,
//! `voxa-VERSION`, with the program, its readme, its licences and notices, how to set up
//! labelling, the changelog and what Voxa is for. The same folder is left beside the zip.
//!
//! How the program is built, and what is read of it before it is packed, is in
//! [`program`]: the C runtime inside it, no Visual C++ to import, its icon, and no path of the
//! computer it was built on.
//!
//! With `--runtime`, the package also carries ONNX Runtime from that folder, in a folder of its
//! own called `runtime`: its two libraries, and its licence and notices, which a release must
//! carry with them. A runtime folder without them is refused, unless `--local` is given for a
//! package that is not for release, whose zip then ends `-windows-x64-local.zip`.
//! `DirectML.dll` is never carried, since Windows has its own.

mod pe;
mod program;
#[cfg(test)]
mod tests;
mod zipped;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::notices::cargo;
use program::{PROGRAM, Remap};

/// How the task is asked for.
const USAGE: &str = "usage: cargo xtask dist [--runtime FOLDER] [--local]";
/// The package that is the program.
const PACKAGE: &str = "voxa-app";
/// The documents the package carries: the name of each in the package, and where it is in the
/// repository.
const DOCUMENTS: [(&str, &str); 8] = [
    ("README.txt", "packaging/README.txt"),
    ("LICENSE-MIT", "LICENSE-MIT"),
    ("LICENSE-APACHE", "LICENSE-APACHE"),
    ("NOTICE", "NOTICE"),
    ("THIRD-PARTY-NOTICES.txt", "THIRD-PARTY-NOTICES.txt"),
    ("MODELS.md", "docs/models.md"),
    ("CHANGELOG.md", "CHANGELOG.md"),
    ("INTENDED-USE.txt", "docs/intended-use.md"),
];
/// The folder in the package that holds ONNX Runtime, with the libraries it must hold.
const RUNTIME: &str = "runtime";
const LIBRARIES: [&str; 2] = ["onnxruntime.dll", "onnxruntime_providers_shared.dll"];
/// ONNX Runtime's own licence and notices: the name each has in the package, and the names
/// it may have in the runtime folder, with or without `.txt`, as Microsoft's packages have
/// given them.
const RUNTIME_NOTICES: [(&str, [&str; 2]); 2] = [
    ("LICENSE", ["LICENSE", "LICENSE.txt"]),
    (
        "ThirdPartyNotices.txt",
        ["ThirdPartyNotices.txt", "ThirdPartyNotices"],
    ),
];

/// A file the package carries: its path in the package, and the file it comes from.
type Carried = (String, PathBuf);

/// What the task was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Asked {
    /// The folder that holds ONNX Runtime, when the package is to carry it.
    pub runtime: Option<PathBuf>,
    /// Whether the package is for this computer alone, not for release.
    pub local: bool,
}

/// Where the files of a package come from.
pub struct Sources<'a> {
    /// The program, as the build made it.
    pub program: &'a Path,
    /// The root of the repository, which holds the documents.
    pub root: &'a Path,
    /// The folder that holds ONNX Runtime, when the package is to carry it.
    pub runtime: Option<&'a Path>,
    /// Whether the package is for this computer alone, not for release: it may then go
    /// without the runtime's licence and notices, and its zip's name says so.
    pub local: bool,
}

/// A package, made: the folder, the zip of it, the paths of the files in the folder, and a
/// sentence for each file that could have been carried and was not.
#[derive(Debug)]
pub struct Made {
    /// The folder, `voxa-VERSION`.
    pub folder: PathBuf,
    /// The zip that holds the folder.
    pub zip: PathBuf,
    /// The path of each file within the folder, with `/` between the names.
    pub files: Vec<String>,
    /// What was left out, and why.
    pub missing: Vec<String>,
}

/// `cargo xtask dist`, with the arguments after `dist`.
pub fn make(args: &[String]) -> Result<(), String> {
    let asked = asked(args)?;
    if !cfg!(windows) {
        return Err("cargo xtask dist makes the package for Windows, so it runs on Windows".into());
    }
    // A runtime folder a release cannot be made from is refused before anything is built.
    if let Some(runtime) = &asked.runtime {
        runtime_notices(runtime, asked.local)?;
    }
    let said = cargo(&["metadata", "--format-version", "1", "--no-deps"])?;
    let metadata: Value = serde_json::from_str(&said)
        .map_err(|error| format!("could not read what cargo metadata said: {error}"))?;
    let (version, target) = version_and_target(&metadata)
        .ok_or("cargo metadata did not say the program's version and the target folder")?;
    let root = crate::workspace_root();
    let (program, remapped) = program::build(&root, &target)?;
    let imported = checked(&program, &remapped)?;
    let sources = Sources {
        program: &program,
        root: &root,
        runtime: asked.runtime.as_deref(),
        local: asked.local,
    };
    let made = assemble(&sources, &version, &target.join("dist"))?;
    report(&made, &imported);
    Ok(())
}

/// What the arguments ask for, or how the task is asked for: a runtime folder after
/// `--runtime`, and `--local`, each at most once and in either order.
fn asked(args: &[String]) -> Result<Asked, String> {
    let mut asked = Asked::default();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--runtime" if asked.runtime.is_none() => {
                let folder = words.next().filter(|folder| !folder.starts_with("--"));
                asked.runtime = Some(PathBuf::from(folder.ok_or(USAGE)?));
            }
            "--local" if !asked.local => asked.local = true,
            _ => return Err(USAGE.to_owned()),
        }
    }
    Ok(asked)
}

/// The program's version and the folder cargo builds in, from what `cargo metadata` said.
fn version_and_target(metadata: &Value) -> Option<(String, PathBuf)> {
    let packages = metadata.get("packages")?.as_array()?;
    let named = |package: &&Value| package.get("name").and_then(Value::as_str) == Some(PACKAGE);
    let program = packages.iter().find(named)?;
    let version = program.get("version")?.as_str()?;
    let target = metadata.get("target_directory")?.as_str()?;
    Some((version.to_owned(), PathBuf::from(target)))
}

/// Reads the built program and fails if it imports Visual C++, carries no icon or still
/// records a place of `remapped`. The libraries it imports, when it passes.
fn checked(program: &Path, remapped: &[Remap]) -> Result<Vec<String>, String> {
    let image = fs::read(program)
        .map_err(|error| format!("could not read {}: {error}", program.display()))?;
    let imported = pe::imports(&image).map_err(|why| format!("{}: {why}", program.display()))?;
    program::without_visual_cpp(&imported)?;
    let carried = pe::has_resources(&image).map_err(|why| format!("{}: {why}", program.display()));
    if !carried? {
        return Err(format!(
            concat!(
                "{} carries no icon or version details: the build found no resource ",
                "compiler (rc.exe, from the Windows SDK)"
            ),
            PROGRAM
        ));
    }
    program::without_places(&image, remapped)?;
    Ok(imported)
}

/// Makes the package for `version` in `dist` from `sources`: the folder, then the zip of it.
/// Every file is looked for before anything is made, and what an earlier run made for the
/// same version is taken away first.
///
/// # Errors
///
/// A sentence naming the file, when a file the package must carry is not there, or naming
/// what the runtime folder lacks, for a package that is for release; and what went wrong
/// when a file could not be copied or the zip written.
pub fn assemble(sources: &Sources, version: &str, dist: &Path) -> Result<Made, String> {
    let (files, missing) = contents(sources)?;
    let top = format!("voxa-{version}");
    let folder = dist.join(&top);
    let kind = if sources.local { "-local" } else { "" };
    let zip = dist.join(format!("{top}-windows-x64{kind}.zip"));
    if folder.exists() {
        fs::remove_dir_all(&folder).map_err(|error| failed("clear", &folder, &error))?;
    }
    if zip.exists() {
        fs::remove_file(&zip).map_err(|error| failed("clear", &zip, &error))?;
    }
    for (name, from) in &files {
        let to = folder.join(name);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|error| failed("make", parent, &error))?;
        }
        fs::copy(from, &to).map_err(|error| failed("copy", from, &error))?;
    }
    let names: Vec<String> = files.into_iter().map(|(name, _)| name).collect();
    zipped::zipped(&folder, &top, &names, &zip)?;
    Ok(Made {
        folder,
        zip,
        files: names,
        missing,
    })
}

/// What the package carries, as its path in the folder and the file it comes from, and a
/// sentence for each of the runtime's notices the runtime folder lacks.
fn contents(sources: &Sources) -> Result<(Vec<Carried>, Vec<String>), String> {
    let mut files = vec![(PROGRAM.to_owned(), sources.program.to_path_buf())];
    let documents = DOCUMENTS
        .iter()
        .map(|(name, from)| ((*name).to_owned(), sources.root.join(from)));
    files.extend(documents);
    let mut missing = Vec::new();
    if let Some(runtime) = sources.runtime {
        let libraries = LIBRARIES
            .iter()
            .map(|name| (format!("{RUNTIME}/{name}"), runtime.join(name)));
        files.extend(libraries);
        let (notices, lacking) = runtime_notices(runtime, sources.local)?;
        files.extend(notices);
        missing = lacking;
    }
    if let Some((name, from)) = files.iter().find(|(_, from)| !from.is_file()) {
        let from = from.display();
        return Err(format!(
            "the package needs {from} as {name}, and there is no such file"
        ));
    }
    Ok((files, missing))
}

/// ONNX Runtime's licence and notices in the folder `runtime`, as the package carries them,
/// and a sentence for each that the folder lacks. For a package that is not `local`, a folder
/// that lacks either is refused, with a sentence saying what it lacks.
fn runtime_notices(runtime: &Path, local: bool) -> Result<(Vec<Carried>, Vec<String>), String> {
    let (mut carried, mut lacking) = (Vec::new(), Vec::new());
    for (name, found_as) in RUNTIME_NOTICES {
        let found = found_as.iter().map(|named| runtime.join(named));
        match found.into_iter().find(|from| from.is_file()) {
            Some(from) => carried.push((format!("{RUNTIME}/{name}"), from)),
            None => lacking.push(found_as.join(" or ")),
        }
    }
    let folder = runtime.display();
    if !lacking.is_empty() && !local {
        return Err(format!(
            concat!(
                "{} holds no {}, ONNX Runtime's own licence and notices, which a release must ",
                "carry with the runtime: put them in the folder, or give --local for a package ",
                "that is not for release"
            ),
            folder,
            lacking.join(" and no ")
        ));
    }
    let said = lacking.iter().map(|named| {
        let goes = "so this package, which is not for release, goes without it";
        format!("{folder} holds no {named}, ONNX Runtime's own, {goes}")
    });
    Ok((carried, said.collect()))
}

/// The words for a file or folder that could not be dealt with.
fn failed(what: &str, path: &Path, error: &io::Error) -> String {
    format!("could not {what} {}: {error}", path.display())
}

/// Says what was made, how large it is, that the program carries its icon, what it imports,
/// that it records no place of the computer it was built on, and what was left out.
fn report(made: &Made, imported: &[String]) {
    let size = fs::metadata(&made.zip).map_or(0, |file| file.len());
    let megabytes = size as f64 / 1_048_576.0;
    println!("\n==> made {} ({megabytes:.1} MB)", made.zip.display());
    println!("from {}, which holds:", made.folder.display());
    for name in &made.files {
        println!("  {name}");
    }
    println!("{PROGRAM} carries its icon and version details");
    println!(
        "{PROGRAM} imports {}, and nothing of Visual C++",
        imported.join(", ")
    );
    println!("{PROGRAM} records no place on the computer it was built on");
    for sentence in &made.missing {
        println!("warning: {sentence}");
    }
}
