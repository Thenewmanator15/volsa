//! `cargo xtask notices` writes `THIRD-PARTY-NOTICES.txt` at the root of the repository: the
//! licence and notice texts of every package built into the Voxa program. `cargo xtask check`
//! holds the committed file to what this would write now.
//!
//! The packages are those a release build of `voxa-app` for 64-bit Windows compiles: the
//! normal dependencies reachable from it on `x86_64-pc-windows-msvc`, as `cargo tree` resolves
//! them. Build and dev dependencies are not shipped. `cargo metadata` gives each package's
//! licence, repository and folder. Its own graph does not give the list, because it turns on
//! every feature any crate of the workspace asks for, its dev dependencies' among them, and
//! so names some eighty packages that are never compiled into the program.
//!
//! For each package the file gives its name, version, licence and repository, and the texts in
//! its folder whose names start with LICENSE, LICENCE, COPYING, NOTICE or COPYRIGHT. A package
//! that comes with none is given the standard text of each of its licences, from
//! `xtask/licences`. The typefaces the interface library builds in are named with their
//! licences. Texts that are the same word for word are written once and referred to by number.

mod package;
mod standard;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::process::{Command, Stdio};

use serde_json::Value;

use package::{Package, files_ending, licence_files, packages, read};
use standard::{FONTS, FONTS_FOLDER, TYPEFACES, licence_ids, standard};

/// The file the notices are written to, at the root of the repository.
pub const FILE: &str = "THIRD-PARTY-NOTICES.txt";
/// The target the program is built for: the packages compiled for it are the ones shipped.
pub const TARGET: &str = "x86_64-pc-windows-msvc";
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
    notices(&metadata, &shipped(&listing))
}

/// What cargo prints when run with `args` at the root of the repository, with `--locked` so
/// that nothing here changes `Cargo.lock`.
pub fn cargo(args: &[&str]) -> Result<String, String> {
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

/// The notices for the packages of `metadata` that `shipped` names.
pub fn notices(metadata: &Value, shipped: &BTreeSet<(String, String)>) -> Result<String, String> {
    let packages = packages(metadata, shipped)?;
    let mut texts = Texts::default();
    let mut entries = String::new();
    for package in &packages {
        entries.push_str(&entry(package, &mut texts)?);
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
         licence and notice texts that come with it. For a package that comes with none,\n\
         the standard text of each of its licences stands in. Texts that are the same\n\
         word for word are written once, under TEXTS, and referred to by number.\n\n\
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

/// One package's entry: its heading, what its manifest says, and a line for each text,
/// ending with a blank line.
fn entry(package: &Package, texts: &mut Texts) -> Result<String, String> {
    let expression = package.licence.as_deref();
    let mut lines = vec![
        package.heading(),
        format!("  Licence: {}", expression.unwrap_or("not given")),
    ];
    lines.push(match (&package.repository, &package.homepage) {
        (Some(repository), _) => format!("  Repository: {repository}"),
        (None, Some(homepage)) => format!("  Homepage: {homepage}"),
        (None, None) => "  Repository: not given".to_owned(),
    });
    if expression.is_some_and(|expression| licence_ids(expression).contains(&"MPL-2.0")) {
        lines.push(source(package));
    }
    let files = licence_files(package)?;
    let fonts = font_texts(package)?;
    if files.is_empty() {
        lines.extend(standard_texts(package, &fonts, texts)?);
    }
    for file in files.iter().chain(&fonts) {
        let called = || format!("{file} of {}", package.heading());
        let number = texts.number(&read(&package.folder.join(file))?, called);
        lines.push(format!("  {file}: text {number}"));
    }
    Ok(lines.join("\n") + "\n\n")
}

/// Where the source of a package under the Mozilla Public License can be had, as that
/// licence asks of a program that ships it.
fn source(package: &Package) -> String {
    let at = if package.from_crates_io {
        format!(
            "https://crates.io/crates/{}/{}",
            package.name, package.version
        )
    } else {
        package
            .repository
            .clone()
            .unwrap_or_else(|| "its repository".to_owned())
    };
    format!("  Source: Voxa uses this package unchanged. Its source code is at {at}")
}

/// The licence files of the typefaces, when `package` is the font package; none otherwise.
/// A font file there that [`TYPEFACES`] does not name is an error.
fn font_texts(package: &Package) -> Result<Vec<String>, String> {
    if package.name != FONTS {
        return Ok(Vec::new());
    }
    let folder = package.folder.join(FONTS_FOLDER);
    for font in files_ending(&folder, ".ttf")?
        .into_iter()
        .chain(files_ending(&folder, ".otf")?)
    {
        if !TYPEFACES.iter().any(|typeface| typeface.font == font) {
            let what = format!("{} has a typeface xtask does not know", package.heading());
            return Err(format!(
                "{what}, {FONTS_FOLDER}/{font}: name it and its licence in TYPEFACES"
            ));
        }
    }
    Ok(TYPEFACES
        .iter()
        .map(|typeface| format!("{FONTS_FOLDER}/{}", typeface.text))
        .collect())
}

/// For a package that comes with no licence file: who wrote it, and the standard text of
/// each of its licences that the typefaces' own files do not already give.
fn standard_texts(
    package: &Package,
    fonts: &[String],
    texts: &mut Texts,
) -> Result<Vec<String>, String> {
    let expression = package.licence.as_deref().unwrap_or_default();
    let covered = |id: &&str| {
        !fonts.is_empty()
            && TYPEFACES
                .iter()
                .any(|typeface| typeface.covers == Some(*id))
    };
    let mut lines = Vec::new();
    if !package.authors.is_empty() {
        lines.push(format!("  Authors: {}", package.authors.join(", ")));
    }
    lines.push("  Its code comes with no licence file; these standard texts stand in:".to_owned());
    let mut given = 0;
    for id in licence_ids(expression).iter().filter(|id| !covered(id)) {
        if let Some(text) = standard(id) {
            let number = texts.number(&text, || format!("the standard text of {id}"));
            lines.push(format!("  {id}: text {number}"));
            given += 1;
        }
    }
    if given > 0 || !fonts.is_empty() {
        return Ok(lines);
    }
    let what = format!("{} comes with no licence file", package.heading());
    Err(format!(
        "{what}, and xtask/licences keeps no standard text for {expression}: add one there"
    ))
}

/// A text as every text in the file is written: lines ending with `\n` and no spaces after
/// them, and no blank lines before or after.
fn tidy(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .skip_while(|line| line.is_empty())
        .collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let mut tidied = lines.join("\n");
    if !tidied.is_empty() {
        tidied.push('\n');
    }
    tidied
}

/// The texts written once each at the end of the file, numbered from 1 in the order they are
/// first met.
#[derive(Default)]
struct Texts {
    /// Each text, after what it is called where it is first met.
    written: Vec<(String, String)>,
    /// The number of each text, by its words with one space between each two.
    numbers: HashMap<String, usize>,
}

impl Texts {
    /// The number of `text`, which is given one when it is new; `called` says what it is.
    fn number(&mut self, text: &str, called: impl FnOnce() -> String) -> usize {
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if let Some(&number) = self.numbers.get(&words) {
            return number;
        }
        self.written.push((called(), text.to_owned()));
        self.numbers.insert(words, self.written.len());
        self.written.len()
    }

    /// Every text, each under its number and what it is.
    fn written(&self) -> String {
        let each = self.written.iter().enumerate();
        let each = each.map(|(at, (called, text))| {
            format!("{RULE}\nText {}: {called}\n{RULE}\n\n{text}\n", at + 1)
        });
        each.collect()
    }
}
