//! `cargo xtask notices` writes `THIRD-PARTY-NOTICES.txt` at the root of the repository: the
//! licence and notice texts of every package built into the Voxa program. `cargo xtask check`
//! holds the committed file to what this would write now.
//!
//! The packages are those a release build of `voxa-app` for 64-bit Windows compiles: the
//! normal dependencies reachable from it on `x86_64-pc-windows-msvc`, as `cargo tree` resolves
//! them. Build and dev dependencies are not shipped, except a package whose files another's
//! build turns into code that is compiled in (the module `upstream` names them). `cargo
//! metadata` gives each package's licence, repository and folder. Its own graph does not give
//! the list, because it turns on every feature any crate of the workspace asks for, its dev
//! dependencies' among them, and so names some eighty packages that are never compiled into
//! the program.
//!
//! For each package the file gives its name, version, licence and repository, and the texts in
//! its folder whose names start with LICENSE, LICENCE, COPYING, NOTICE or COPYRIGHT. Where a
//! package offers a choice of licences, Voxa takes the most permissive it accepts (the module
//! `choice`), the file says which, and the texts of the licences not taken are left out. A
//! package that comes with no text of its licence is given the standard text, from
//! `xtask/licences`, with the copyright lines of the licence file in its repository. The
//! typefaces the interface library builds in are named with their licences. Texts that are
//! the same word for word are written once and referred to by number.

mod choice;
#[cfg(test)]
mod choice_tests;
mod entry;
mod package;
mod standard;
#[cfg(test)]
mod tests;
mod texts;
mod upstream;
#[cfg(test)]
mod upstream_tests;

use std::collections::BTreeSet;
use std::fs;
use std::process::{Command, Stdio};

use serde_json::Value;

use entry::entry;
use package::{Package, packages, read};
use standard::{FONTS, FONTS_FOLDER, TYPEFACES};
use texts::{Texts, tidy};
use upstream::{KNOWN, Known, with_generated};

/// The file the notices are written to, at the root of the repository.
pub const FILE: &str = "THIRD-PARTY-NOTICES.txt";
/// The target the program is built for: the packages compiled for it are the ones shipped.
const TARGET: &str = "x86_64-pc-windows-msvc";
/// The package that is the program.
const PROGRAM: &str = "voxa-app";
/// The line that sets each text apart from the next.
const RULE: &str = "========================================================================";

/// `cargo xtask notices`: write the notices afresh.
pub fn write() -> Result<(), String> {
    let path = crate::workspace_root().join(FILE);
    let written = generate()?;
    let unwritten = |error| format!("could not write {}: {error}", path.display());
    fs::write(&path, &written).map_err(unwritten)?;
    println!("wrote {FILE}, {} bytes", written.len());
    Ok(())
}

/// Part of `cargo xtask check`: the committed notices must be what [`write()`] would write now.
pub fn up_to_date() -> Result<(), String> {
    println!("\n==> {FILE} names every package built into Voxa as it is now");
    let now = generate()?;
    let committed = fs::read_to_string(crate::workspace_root().join(FILE)).ok();
    held_to(committed.as_deref(), &now)
}

/// Whether the committed notices are the same as those written now, whatever ends their
/// lines; if they are not, a sentence that says what to do.
pub fn held_to(committed: Option<&str>, now: &str) -> Result<(), String> {
    let same = committed.is_some_and(|committed| committed.replace("\r\n", "\n") == now);
    let how = match committed {
        _ if same => return Ok(()),
        Some(_) => "is out of date with the packages Voxa is built from",
        None => "is missing",
    };
    Err(format!(
        "{FILE} {how}: run `cargo xtask notices` and commit the file"
    ))
}

/// The notices, from what cargo says of the workspace now.
fn generate() -> Result<String, String> {
    let described = cargo(&[
        "metadata",
        "--format-version",
        "1",
        "--filter-platform",
        TARGET,
    ])?;
    let metadata: Value = serde_json::from_str(&described)
        .map_err(|error| format!("could not read what cargo metadata said: {error}"))?;
    let edges = [
        "--edges", "normal", "--target", TARGET, "--prefix", "none", "--format", "{p}",
    ];
    let listing = cargo(&[["tree", "--package", PROGRAM].as_slice(), &edges].concat())?;
    notices(&metadata, &shipped(&listing), &KNOWN)
}

/// What cargo prints when run with `args` at the root of the repository, with `--locked` so
/// that nothing here changes `Cargo.lock`.
fn cargo(args: &[&str]) -> Result<String, String> {
    let shown = format!("cargo {} --locked", args.join(" "));
    let output = Command::new("cargo")
        .args(args)
        .arg("--locked")
        .current_dir(crate::workspace_root())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not start `{shown}`: {error}"))?;
    if !output.status.success() {
        return Err(format!("`{shown}` failed"));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| format!("`{shown}` printed text that is not UTF-8"))
}

/// The name and version of each package `cargo tree` lists, a line each as `name vVERSION`
/// and perhaps more after. Lines that are not so are passed by.
pub fn shipped(listing: &str) -> BTreeSet<(String, String)> {
    let named = |line: &str| {
        let mut words = line.split_whitespace();
        let name = words.next()?;
        let version = words.next()?.strip_prefix('v')?;
        Some((name.to_owned(), version.to_owned()))
    };
    listing.lines().filter_map(named).collect()
}

/// The notices for the packages of `metadata` that `shipped` names, and those whose files
/// their builds compile in, with what `known` says beyond the packages' folders.
pub fn notices(
    metadata: &Value,
    shipped: &BTreeSet<(String, String)>,
    known: &Known,
) -> Result<String, String> {
    let packages = packages(metadata, &with_generated(metadata, shipped, known)?)?;
    let mut texts = Texts::default();
    let mut entries = String::new();
    for package in &packages {
        entries.push_str(&entry(package, &mut texts, known)?);
    }
    let mut written = heading(packages.len());
    if let Some(fonts) = packages.iter().find(|package| package.name == FONTS) {
        written.push_str(&typefaces(fonts, &mut texts)?);
    }
    written.push_str("PACKAGES\n--------\n\n");
    written.push_str(&entries);
    written.push_str("TEXTS\n-----\n\n");
    written.push_str(&texts.written());
    // One line end closes the file, and no blank line.
    written.truncate(written.trim_end().len());
    written.push('\n');
    Ok(written)
}

/// What the file is, before the first section.
fn heading(count: usize) -> String {
    format!(
        "THIRD-PARTY NOTICES FOR VOXA\n\
         ============================\n\n\
         Voxa is built from the {count} packages below, other people's work, each under\n\
         its own licence. For each, this file gives its licence and repository and the\n\
         licence and notice texts that come with it. Where a package offers a choice of\n\
         licences, Voxa takes the most permissive of those it accepts, which \"Voxa takes\n\
         it under\" names, and the texts of the others are left out. For a package that\n\
         comes with no text of its licence, the standard text stands in, with the\n\
         copyright lines of the licence file in its repository where that file has any.\n\
         Texts that are the same word for word are written once, under TEXTS, and\n\
         referred to by number.\n\n\
         Voxa's own licence is in LICENSE-MIT and LICENSE-APACHE, and its own notice,\n\
         with the credit to SynthSeg, is in NOTICE.\n\n\
         `cargo xtask notices` writes this file from the packages compiled into the\n\
         Voxa program for 64-bit Windows. Do not edit it by hand.\n\n"
    )
}

/// The section that names the typefaces in the font package, each with its licence.
fn typefaces(fonts: &Package, texts: &mut Texts) -> Result<String, String> {
    let mut lines = vec![
        "TYPEFACES".to_owned(),
        "---------".to_owned(),
        String::new(),
        format!(
            "The interface library builds these typefaces into the program, from\n{}. Each \
             is under its own licence, whose text comes\nwith it:",
            fonts.heading()
        ),
        String::new(),
    ];
    for typeface in &TYPEFACES {
        let file = format!("{FONTS_FOLDER}/{}", typeface.text);
        let number = texts.number(&read(&fonts.folder.join(&file))?, || file.clone());
        let (name, licence) = (typeface.name, typeface.licence);
        lines.push(format!("  {name}: {licence}; {file}, text {number}"));
    }
    Ok(lines.join("\n") + "\n\n")
}
