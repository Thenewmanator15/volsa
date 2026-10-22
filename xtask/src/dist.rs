//! `cargo xtask dist [--runtime FOLDER]` makes the package a release of Voxa offers for
//! 64-bit Windows: `target/dist/voxa-VERSION-windows-x64.zip`, which holds one folder,
//! `voxa-VERSION`, with the program, its readme, its licences and notices, how to set up
//! labelling, the changelog and what Voxa is for. The same folder is left beside the zip.
//!
//! The program is built for release with Microsoft's C runtime inside it, so that it needs no
//! Visual C++ redistributable, and its import table is read to be sure of that. Only the
//! package is built so: the flag goes to this one build, for its own target folder, and an
//! ordinary build is left as it was.
//!
//! With `--runtime`, the package also carries ONNX Runtime from that folder, in a folder of its
//! own called `runtime`: its two libraries, and its licence and notices when the folder holds
//! them. `DirectML.dll` is never carried, since Windows has its own.

mod pe;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::notices::{TARGET, cargo};

/// How the task is asked for.
const USAGE: &str = "usage: cargo xtask dist [--runtime FOLDER]";
/// The package that is the program, and the program's file as the build names it and as the
/// package carries it.
const PACKAGE: &str = "voxa-app";
const PROGRAM: &str = "voxa.exe";
/// What the build adds to the flags the compiler is given: the C runtime inside the program.
const STATIC_RUNTIME: &str = "-C target-feature=+crt-static";
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
/// The folder in the package that holds ONNX Runtime, with the libraries it must hold, and
/// the runtime's own licence and notices, which go with it when the folder given holds them.
const RUNTIME: &str = "runtime";
const LIBRARIES: [&str; 2] = ["onnxruntime.dll", "onnxruntime_providers_shared.dll"];
const RUNTIME_NOTICES: [&str; 2] = ["LICENSE", "ThirdPartyNotices.txt"];
/// The libraries of Microsoft's C and C++ runtime, by how their names start in lower case,
/// that a program with that runtime inside it does not import.
const VISUAL_CPP: [&str; 4] = ["vcruntime", "msvcp", "ucrtbase", "api-ms-win-crt-"];

/// A file the package carries: its path in the package, and the file it comes from.
type Carried = (String, PathBuf);

/// Where the files of a package come from.
pub struct Sources<'a> {
    /// The program, as the build made it.
    pub program: &'a Path,
    /// The root of the repository, which holds the documents.
    pub root: &'a Path,
    /// The folder that holds ONNX Runtime, when the package is to carry it.
    pub runtime: Option<&'a Path>,
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
    let runtime = runtime_named(args)?;
    if !cfg!(windows) {
        return Err("cargo xtask dist makes the package for Windows, so it runs on Windows".into());
    }
    let said = cargo(&["metadata", "--format-version", "1", "--no-deps"])?;
    let metadata: Value = serde_json::from_str(&said)
        .map_err(|error| format!("could not read what cargo metadata said: {error}"))?;
    let (version, target) = version_and_target(&metadata)
        .ok_or("cargo metadata did not say the program's version and the target folder")?;
    let program = build(&target)?;
    let image = fs::read(&program)
        .map_err(|error| format!("could not read {}: {error}", program.display()))?;
    let imported = pe::imports(&image).map_err(|why| format!("{}: {why}", program.display()))?;
    without_visual_cpp(&imported)?;
    let carried = pe::has_resources(&image).map_err(|why| format!("{}: {why}", program.display()));
    if !carried? {
        return Err(format!(
            "{PROGRAM} carries no icon or version details: the build found no resource compiler \
             (rc.exe, from the Windows SDK)"
        ));
    }
    let root = crate::workspace_root();
    let sources = Sources {
        program: &program,
        root: &root,
        runtime: runtime.as_deref(),
    };
    let made = assemble(&sources, &version, &target.join("dist"))?;
    report(&made, &imported);
    Ok(())
}

/// The runtime folder the arguments name, if they name one, or how the task is asked for.
fn runtime_named(args: &[String]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(None),
        [flag, folder] if flag == "--runtime" => Ok(Some(PathBuf::from(folder))),
        _ => Err(USAGE.to_owned()),
    }
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

/// Builds the program for release, with the C runtime inside it, and says where it is.
/// Naming the target puts the build in a folder of its own, and keeps the flag off the
/// programs that run as it is built.
fn build(target: &Path) -> Result<PathBuf, String> {
    let flags = with_static_runtime(std::env::var("RUSTFLAGS").ok().as_deref());
    let args = [
        "build",
        "--release",
        "--locked",
        "--package",
        PACKAGE,
        "--target",
        TARGET,
    ];
    crate::run("cargo", &args, &[("RUSTFLAGS", &flags)])?;
    Ok(target.join(TARGET).join("release").join(PROGRAM))
}

/// The flags the compiler is given for the package: any already given, and the C runtime
/// inside the program.
fn with_static_runtime(given: Option<&str>) -> String {
    match given.map(str::trim) {
        Some(given) if !given.is_empty() => format!("{given} {STATIC_RUNTIME}"),
        _ => STATIC_RUNTIME.to_owned(),
    }
}

/// Fails, naming them, if any of the libraries a program imports is Microsoft's C or C++
/// runtime: then the C runtime is not inside the program, and it would need the Visual C++
/// redistributable on every computer it runs on.
fn without_visual_cpp(imported: &[String]) -> Result<(), String> {
    let runtime = |name: &&String| {
        let name = name.to_ascii_lowercase();
        VISUAL_CPP.iter().any(|start| name.starts_with(start))
    };
    let found: Vec<&str> = imported
        .iter()
        .filter(runtime)
        .map(String::as_str)
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{PROGRAM} imports {}, so the C runtime is not inside it and it would need the Visual \
         C++ redistributable",
        found.join(", ")
    ))
}

/// Makes the package for `version` in `dist` from `sources`: the folder, then the zip of it.
/// Every file is looked for before anything is made, and what an earlier run made for the
/// same version is taken away first.
///
/// # Errors
///
/// A sentence naming the file, when a file the package must carry is not there, and what went
/// wrong when a file could not be copied or the zip written.
pub fn assemble(sources: &Sources, version: &str, dist: &Path) -> Result<Made, String> {
    let (files, missing) = contents(sources)?;
    let top = format!("voxa-{version}");
    let folder = dist.join(&top);
    let zip = dist.join(format!("{top}-windows-x64.zip"));
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
    zipped(&folder, &top, &names, &zip)?;
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
        for name in RUNTIME_NOTICES {
            let from = runtime.join(name);
            if from.is_file() {
                files.push((format!("{RUNTIME}/{name}"), from));
            } else {
                missing.push(format!(
                    "{} holds no {name}, ONNX Runtime's own, so the package's runtime folder \
                     goes without it; a release must carry it",
                    runtime.display()
                ));
            }
        }
    }
    if let Some((name, from)) = files.iter().find(|(_, from)| !from.is_file()) {
        let from = from.display();
        return Err(format!(
            "the package needs {from} as {name}, and there is no such file"
        ));
    }
    Ok((files, missing))
}

/// Writes the zip: the folder `top`, its folders and then its files, each compressed and
/// dated now.
fn zipped(folder: &Path, top: &str, names: &[String], zip: &Path) -> Result<(), String> {
    let written =
        |error: &dyn std::fmt::Display| format!("could not write {}: {error}", zip.display());
    let file = File::create(zip).map_err(|error| written(&error))?;
    let mut writer = ZipWriter::new(file);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip_time(now.as_secs()));
    let mut folders = BTreeSet::from([format!("{top}/")]);
    for name in names {
        let parents = name
            .match_indices('/')
            .map(|(at, _)| format!("{top}/{}/", &name[..at]));
        folders.extend(parents);
    }
    for inner in folders {
        writer
            .add_directory(inner, options)
            .map_err(|error| written(&error))?;
    }
    for name in names {
        let from = folder.join(name);
        let mut read = File::open(&from).map_err(|error| failed("read", &from, &error))?;
        writer
            .start_file(format!("{top}/{name}"), options)
            .map_err(|error| written(&error))?;
        io::copy(&mut read, &mut writer).map_err(|error| written(&error))?;
    }
    writer.finish().map_err(|error| written(&error))?;
    Ok(())
}

/// A zip's date and time for `seconds` after the start of 1970, at Greenwich. A time a zip
/// cannot hold, before 1980 or after 2107, is given as the start of 1980.
fn zip_time(seconds: u64) -> DateTime {
    let (year, month, day) = civil(seconds / 86_400);
    let of_day = seconds % 86_400;
    let [hour, minute, second] = [of_day / 3600, of_day % 3600 / 60, of_day % 60]
        .map(|part| u8::try_from(part).unwrap_or(0));
    let year = u16::try_from(year).unwrap_or(0);
    DateTime::from_date_and_time(year, month, day, hour, minute, second).unwrap_or_default()
}

/// The year, month and day of the day `days` after the first of January 1970.
fn civil(days: u64) -> (u64, u8, u8) {
    // Days counted from the first of March of the year 0, in eras of 400 years.
    let counted = days + 719_468;
    let era = counted / 146_097;
    let of_era = counted % 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let from_march = (5 * of_year + 2) / 153;
    let day = of_year - (153 * from_march + 2) / 5 + 1;
    let month = if from_march < 10 {
        from_march + 3
    } else {
        from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    let small = |part: u64| u8::try_from(part).unwrap_or(0);
    (year, small(month), small(day))
}

/// The words for a file or folder that could not be dealt with.
fn failed(what: &str, path: &Path, error: &io::Error) -> String {
    format!("could not {what} {}: {error}", path.display())
}

/// Says what was made, how large it is, that the program carries its icon, what it imports,
/// and what was left out.
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
    for sentence in &made.missing {
        println!("warning: {sentence}");
    }
}
