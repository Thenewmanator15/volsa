//! One package's entry in the notices: what its manifest says, which licence Volsa takes it
//! under, and a line for each of its texts.

use super::choice::{covers, taken};
use super::package::{Package, files_ending, licence_files, read};
use super::standard::{FONTS, FONTS_FOLDER, TYPEFACES, licence_ids, standard};
use super::texts::{Texts, tidy};
use super::upstream::{Extra, Known, holders_of, with_holders};

/// The licences of one package.
struct Licences<'a> {
    /// Those its manifest names, each once.
    offered: Vec<&'a str>,
    /// Those Volsa takes it under.
    taken: Vec<&'a str>,
}

impl Licences<'_> {
    /// Whether it offers a choice, and so some of its licences are not taken.
    fn choice(&self) -> bool {
        self.taken != self.offered
    }
}

/// A licence or notice file that comes with a package: its name, its text, and the licences
/// on offer it is the text of.
struct Shipped<'a> {
    /// Its name in the package's folder.
    name: String,
    /// Its text, tidied.
    text: String,
    /// The licences on offer it is the text of: none for a note.
    covers: Vec<&'a str>,
}

/// One package's entry: its heading, what its manifest says, which licence Volsa takes it
/// under, and a line for each text, ending with a blank line.
pub fn entry(package: &Package, texts: &mut Texts, known: &Known) -> Result<String, String> {
    let expression = package.licence.as_deref();
    let taken = match expression {
        Some(expression) => taken(expression).ok_or_else(|| refused(package, expression))?,
        None => Vec::new(),
    };
    let offered = expression.map(licence_ids).unwrap_or_default();
    let licences = Licences { offered, taken };
    let mut lines = described(package, &licences, known);
    let names = licence_files(package)?;
    let kept = kept(package, &names, &licences)?;
    let fonts = font_texts(package)?;
    let wanting = wanting(&kept, &licences);
    if kept.is_empty() || !wanting.is_empty() {
        let none = names.is_empty();
        let given = standard_texts(package, &wanting, &fonts, none, texts, known)?;
        lines.extend(given);
    }
    for file in &kept {
        let called = || format!("{} of {}", file.name, package.heading());
        let number = texts.number(&file.text, called);
        lines.push(format!("  {}: text {number}", file.name));
    }
    for file in &fonts {
        let called = || format!("{file} of {}", package.heading());
        let number = texts.number(&read(&package.folder.join(file))?, called);
        lines.push(format!("  {file}: text {number}"));
    }
    lines.extend(extra_texts(package, texts, known));
    Ok(lines.join("\n") + "\n\n")
}

/// The first lines of an entry: the package, its licence, which Volsa takes it under when it
/// offers a choice, its repository, why it is built in when only a build uses it, and where
/// its source is when its licence asks that.
fn described(package: &Package, licences: &Licences, known: &Known) -> Vec<String> {
    let expression = package.licence.as_deref().unwrap_or("not given");
    let mut lines = vec![package.heading(), format!("  Licence: {expression}")];
    if licences.choice() {
        let taken = licences.taken.join(" AND ");
        lines.push(format!("  Volsa takes it under: {taken}"));
    }
    lines.push(match (&package.repository, &package.homepage) {
        (Some(repository), _) => format!("  Repository: {repository}"),
        (None, Some(homepage)) => format!("  Homepage: {homepage}"),
        (None, None) => "  Repository: not given".to_owned(),
    });
    let built_from = known.generated.iter().find(|g| g.from == package.name);
    if let Some(generated) = built_from {
        lines.push(format!("  Built in: {}", generated.what));
    }
    if licences.taken.contains(&"MPL-2.0") {
        lines.push(source(package));
    }
    lines
}

/// The licence and notice files of a package that go into the notices, of those called
/// `names`: under a choice, a text only of licences not taken is left out.
fn kept<'a>(
    package: &Package,
    names: &[String],
    licences: &Licences<'a>,
) -> Result<Vec<Shipped<'a>>, String> {
    let mut kept = Vec::new();
    for name in names {
        let text = read(&package.folder.join(name))?;
        let covers = covers(name, &text, &licences.offered);
        let not_taken = |id: &&str| !licences.taken.contains(id);
        if licences.choice() && !covers.is_empty() && covers.iter().all(not_taken) {
            continue;
        }
        let name = name.clone();
        kept.push(Shipped { name, text, covers });
    }
    Ok(kept)
}

/// The licences taken whose text does not come with the package: all of them when nothing
/// does; under a choice, those no file kept is the text of; otherwise none, the files that
/// come with the package standing for its licence.
fn wanting<'a>(kept: &[Shipped], licences: &Licences<'a>) -> Vec<&'a str> {
    if kept.is_empty() {
        return licences.taken.clone();
    }
    if !licences.choice() {
        return Vec::new();
    }
    let given = |id: &&str| kept.iter().any(|file| file.covers.contains(id));
    let taken = licences.taken.iter().copied();
    taken.filter(|id| !given(id)).collect()
}

/// Why a package cannot be written: none of the licences it offers is one Volsa accepts.
fn refused(package: &Package, expression: &str) -> String {
    format!(
        "{} is under {expression}, and Volsa accepts none of those licences: deny.toml \
         allows them, and xtask/src/notices/choice.rs ranks them",
        package.heading()
    )
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
    format!("  Source: Volsa uses this package unchanged. Its source code is at {at}")
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

/// For a package that comes with no text of `wanting`, the licences it is taken under: who
/// holds its copyright, and the standard text of each that the typefaces' own files do not
/// already give, with the holders' lines in it. `none` says that no licence file came with it
/// at all.
fn standard_texts(
    package: &Package,
    wanting: &[&str],
    fonts: &[String],
    none: bool,
    texts: &mut Texts,
    known: &Known,
) -> Result<Vec<String>, String> {
    let covered = |id: &&str| {
        !fonts.is_empty()
            && TYPEFACES
                .iter()
                .any(|typeface| typeface.covers == Some(*id))
    };
    let wanting: Vec<&str> = wanting.iter().copied().filter(|id| !covered(id)).collect();
    if wanting.is_empty() {
        if fonts.is_empty() {
            let gives = package.licence.as_deref().unwrap_or("no licence");
            let what = format!("{} comes with no licence file", package.heading());
            return Err(format!("{what}, and its manifest gives {gives}"));
        }
        return Ok(Vec::new());
    }
    let holders = holders_of(package, known)?;
    let mut lines = Vec::new();
    if holders.lines.is_empty() {
        if !package.authors.is_empty() {
            lines.push(format!("  Authors: {}", package.authors.join(", ")));
        }
        lines.push(format!(
            "  Copyright: no holder is named in {}",
            holders.source
        ));
    } else {
        lines.extend(
            holders
                .lines
                .iter()
                .map(|line| format!("  Copyright: {line}")),
        );
        lines.push(format!("  As written in: {}", holders.source));
    }
    lines.push(if none {
        "  Its code comes with no licence file; these standard texts stand in:".to_owned()
    } else {
        "  No text of the licence it is taken under comes with it; these stand in:".to_owned()
    });
    for id in wanting {
        let kept = standard(id).ok_or_else(|| {
            let what = format!("{} comes with no text of {id}", package.heading());
            format!("{what}, and xtask/licences keeps no standard text for it: add one there")
        })?;
        let (text, called) = match with_holders(&kept, holders, &package.heading()) {
            Some((text, how)) => (text, format!("the standard text of {id}, {how}")),
            None => (kept, format!("the standard text of {id}")),
        };
        let number = texts.number(&text, || called);
        lines.push(format!("  {id}: text {number}"));
    }
    Ok(lines)
}

/// A line for each text that the repository of a package keeps for code in it and the
/// package leaves out ([`Known::extra`]).
fn extra_texts(package: &Package, texts: &mut Texts, known: &Known) -> Vec<String> {
    let (name, heading) = (package.name.as_str(), package.heading());
    let extra = known.extra.iter();
    let extra = extra.filter(|extra| extra.packages.contains(&heading.as_str()));
    let line = |extra: &Extra| {
        let called = || format!("{} of {name}'s repository, {}", extra.file, extra.source);
        let number = texts.number(&tidy(extra.text), called);
        format!("  {}, for {}: text {number}", extra.file, extra.covers)
    };
    extra.map(line).collect()
}
