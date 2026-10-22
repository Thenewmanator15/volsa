//! The program the package carries: how it is built, and what is read of it afterwards.
//!
//! It is built for release with Microsoft's C runtime inside it, so that it needs no Visual
//! C++ redistributable, and its import table is read to be sure of that. Only the package is
//! built so: the flags go to this one build, for its own target folder, and an ordinary build
//! is left as it was.
//!
//! The places on the computer it is built on are taken out of the paths the program records,
//! such as where a panic happened: the cargo home becomes `cargo`, the workspace `voxa`, and
//! a target folder outside the workspace `target`. The built program is then searched for
//! those places, and the package is refused if any is still there.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::notices::TARGET;

/// The package that is the program, and the program's file as the build names it and as the
/// package carries it.
const PACKAGE: &str = "voxa-app";
pub const PROGRAM: &str = "voxa.exe";
/// What the build adds to the flags the compiler is given: the C runtime inside the program.
const STATIC_RUNTIME: &str = "-Ctarget-feature=+crt-static";
/// What separates one flag from the next in `CARGO_ENCODED_RUSTFLAGS`, so that a path with a
/// space in it stays one flag.
const BETWEEN: char = '\u{1f}';
/// The libraries of Microsoft's C and C++ runtime, by how their names start in lower case,
/// that a program with that runtime inside it does not import.
const VISUAL_CPP: [&str; 4] = ["vcruntime", "msvcp", "ucrtbase", "api-ms-win-crt-"];

/// Words the program's manifest holds: what it asks of Windows, the common controls of
/// version 6 for the system's message windows and the scale of each screen.
const MANIFEST_ASKS: [&str; 2] = ["Microsoft.Windows.Common-Controls", "PerMonitorV2"];

/// A place on the computer the program is built on, and the name the program records for it
/// in its place.
pub type Remap = (PathBuf, &'static str);

/// The places taken out of the paths the program records: the cargo home, where the code of
/// every package Voxa uses is kept, the workspace `root`, and `target` when it is outside the
/// workspace. A later one wins where two hold the same path.
pub fn remaps(cargo_home: Option<PathBuf>, root: &Path, target: &Path) -> Vec<Remap> {
    let mut remaps: Vec<Remap> = cargo_home.into_iter().map(|home| (home, "cargo")).collect();
    remaps.push((root.to_path_buf(), "voxa"));
    if !target.starts_with(root) {
        remaps.push((target.to_path_buf(), "target"));
    }
    remaps
}

/// The cargo home: where `CARGO_HOME` says, or `.cargo` in the person's own folder,
/// `USERPROFILE` on Windows and `HOME` elsewhere, as cargo itself has it.
pub fn cargo_home(named: Option<OsString>, own_folder: Option<OsString>) -> Option<PathBuf> {
    let named = named.filter(|named| !named.is_empty()).map(PathBuf::from);
    let own = own_folder.filter(|own| !own.is_empty());
    named.or_else(|| own.map(|own| PathBuf::from(own).join(".cargo")))
}

/// The cargo home of this computer.
fn this_cargo_home() -> Option<PathBuf> {
    let own = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    cargo_home(std::env::var_os("CARGO_HOME"), std::env::var_os(own))
}

/// Builds the program for release, with the C runtime inside it and the places of this
/// computer taken out of the paths it records, and says where it is and which places were
/// taken out. Naming the target puts the build in a folder of its own, and keeps the flags
/// off the programs that run as it is built.
pub fn build(root: &Path, target: &Path) -> Result<(PathBuf, Vec<Remap>), String> {
    let remapped = remaps(this_cargo_home(), root, target);
    let encoded = std::env::var("CARGO_ENCODED_RUSTFLAGS").ok();
    let plain = std::env::var("RUSTFLAGS").ok();
    let flags = flags(encoded.as_deref(), plain.as_deref(), &remapped);
    let args = [
        "build",
        "--release",
        "--locked",
        "--package",
        PACKAGE,
        "--target",
        TARGET,
    ];
    crate::run("cargo", &args, &[("CARGO_ENCODED_RUSTFLAGS", &flags)])?;
    let program = target.join(TARGET).join("release").join(PROGRAM);
    Ok((program, remapped))
}

/// The flags the compiler is given for the package, as `CARGO_ENCODED_RUSTFLAGS` holds them:
/// any already given, encoded or in `RUSTFLAGS`, then the C runtime inside the program, then
/// each place to take out of the paths it records. Cargo takes the encoded flags before
/// `RUSTFLAGS`, so those given in `RUSTFLAGS` are carried over.
pub fn flags(encoded: Option<&str>, plain: Option<&str>, remaps: &[Remap]) -> String {
    let mut all: Vec<String> = match (encoded, plain) {
        (Some(encoded), _) if !encoded.is_empty() => {
            encoded.split(BETWEEN).map(str::to_owned).collect()
        }
        (_, Some(plain)) => plain.split_whitespace().map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    all.push(STATIC_RUNTIME.to_owned());
    for (place, name) in remaps {
        all.push(format!("--remap-path-prefix={}={name}", place.display()));
    }
    all.join(&BETWEEN.to_string())
}

/// Fails, naming them, if any of the libraries a program imports is Microsoft's C or C++
/// runtime: then the C runtime is not inside the program, and it would need the Visual C++
/// redistributable on every computer it runs on.
pub fn without_visual_cpp(imported: &[String]) -> Result<(), String> {
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
        concat!(
            "{} imports {}, so the C runtime is not inside it and it would need the Visual ",
            "C++ redistributable"
        ),
        PROGRAM,
        found.join(", ")
    ))
}

/// Fails, naming them, if `image`, the bytes of the program, still records any of the places
/// of `remaps`, written with either slash.
pub fn without_places(image: &[u8], remaps: &[Remap]) -> Result<(), String> {
    let found: Vec<String> = remaps
        .iter()
        .map(|(place, _)| place.display().to_string())
        .filter(|place| {
            let other = place.replace('\\', "/");
            [place.as_str(), other.as_str()]
                .iter()
                .any(|written| holds(image, written.as_bytes()))
        })
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{PROGRAM} still records {}, a place on the computer it was built on",
        found.join(" and ")
    ))
}

/// Fails if `image`, the bytes of the program, carries no manifest asking for the common
/// controls of version 6 and for the scale of each screen.
pub fn with_manifest(image: &[u8]) -> Result<(), String> {
    let asks = MANIFEST_ASKS
        .iter()
        .all(|words| holds(image, words.as_bytes()));
    if asks {
        return Ok(());
    }
    Err(format!(
        concat!(
            "{} carries no manifest asking for the common controls of version 6 and the ",
            "scale of each screen: the build did not put crates/voxa-app/voxa.exe.manifest ",
            "into it"
        ),
        PROGRAM
    ))
}

/// Whether `bytes` hold `wanted` anywhere in them.
fn holds(bytes: &[u8], wanted: &[u8]) -> bool {
    !wanted.is_empty() && bytes.windows(wanted.len()).any(|here| here == wanted)
}

#[cfg(test)]
mod tests;
