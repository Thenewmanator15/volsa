//! The notices written for a small made-up workspace: its metadata, the list `cargo tree`
//! gives, and its packages' folders on disk.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::package::is_licence_file;
use super::standard::{licence_ids, standard};
use super::{held_to, notices, shipped};

/// The made-up Apache text, as two packages ship it: the same words, spaced otherwise.
const APACHE: &str = "Apache License\nVersion 2.0\n\nMade-up terms.\n";
const APACHE_SPACED: &str = "Apache License\r\n  Version 2.0   \r\n\r\nMade-up terms.\r\n";

/// Writes `files` into a package's folder under `root`, and gives the package as the metadata
/// names it.
fn package(
    root: &Path,
    name_version: (&str, &str),
    licence: &str,
    files: &[(&str, &str)],
) -> Value {
    let (name, version) = name_version;
    let folder = root.join(format!("{name}-{version}"));
    fs::create_dir_all(&folder).unwrap();
    for (file, text) in files {
        let path = folder.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let index = "registry+https://github.com/rust-lang/crates.io-index";
    json!({
        "name": name,
        "version": version,
        "id": format!("{index}#{name}@{version}"),
        "license": licence,
        "license_file": null,
        "repository": format!("https://example.com/{name}"),
        "authors": [],
        "source": index,
        "manifest_path": folder.join("Cargo.toml"),
    })
}

/// The typefaces' files, as the interface library's font package ships them.
const FONTS: [(&str, &str); 8] = [
    ("fonts/Hack-Regular.ttf", "a font"),
    (
        "fonts/Hack-Regular.txt",
        "Made-up Hack and Bitstream Vera terms.\n",
    ),
    ("fonts/Ubuntu-Light.ttf", "a font"),
    ("fonts/UFL.txt", "Made-up Ubuntu Font Licence.\n"),
    ("fonts/NotoEmoji-Regular.ttf", "a font"),
    ("fonts/OFL.txt", "Made-up Open Font Licence.\n"),
    ("fonts/emoji-icon-font.ttf", "a font"),
    (
        "fonts/emoji-icon-font-mit-license.txt",
        "Made-up emoji icon terms.\n",
    ),
];

/// The made-up workspace's packages, in no order, its own crate among them.
fn packages(root: &Path) -> Vec<Value> {
    let zeta = [
        (
            "LICENSE-MIT",
            "Copyright 2020 Zeta.\n\nMade-up MIT terms.\n",
        ),
        ("LICENSE-APACHE", APACHE),
    ];
    let beta = [
        ("license-apache", APACHE_SPACED),
        ("COPYRIGHT", "Copyright 2021 Beta.\n"),
        ("README.md", "Not a licence.\n"),
        ("LICENSES/MIT.txt", "In a folder, so not read.\n"),
    ];
    let mut gamma = package(root, ("gamma", "0.3.0"), "MIT", &[]);
    gamma["authors"] = json!(["Gam Ma <gam@example.com>", "Another Author"]);
    gamma["repository"] = Value::Null;
    let mut own = package(
        root,
        ("voxa-app", "0.5.0"),
        "MIT OR Apache-2.0",
        &[("LICENSE-MIT", "Ours.\n")],
    );
    own["id"] = json!("path+file:///made/up/voxa-app#0.5.0");
    own["source"] = Value::Null;
    vec![
        package(root, ("zeta", "1.0.0"), "MIT OR Apache-2.0", &zeta),
        package(
            root,
            ("unused", "9.9.9"),
            "MIT",
            &[("LICENSE", "Never built in.\n")],
        ),
        own,
        package(root, ("beta", "2.1.0"), "Apache-2.0/MIT", &beta),
        gamma,
        package(
            root,
            ("delta", "0.1.0"),
            "MPL-2.0",
            &[("LICENSE", "Made-up MPL.\n")],
        ),
        package(
            root,
            ("epaint_default_fonts", "0.36.2"),
            "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0",
            &FONTS,
        ),
    ]
}

/// What `cargo tree` lists for the made-up program: its own crate, marked by its folder, and
/// a package met twice, marked `(*)`. `unused` is in the metadata but not in the program.
const LISTED: &str = "voxa-app v0.5.0 (G:\\made\\up\\voxa-app)\nzeta v1.0.0\nbeta v2.1.0\n\
                      gamma v0.3.0\ndelta v0.1.0\nzeta v1.0.0 (*)\nepaint_default_fonts v0.36.2\n";

/// The metadata of a made-up workspace whose one member is `voxa-app`.
fn metadata(packages: &[Value]) -> Value {
    json!({ "packages": packages, "workspace_members": ["path+file:///made/up/voxa-app#0.5.0"] })
}

/// The lines of the entry for one package: from its heading to the blank line after it.
fn entry<'a>(notices: &'a str, heading: &str) -> Vec<&'a str> {
    let mut lines = notices.lines().skip_while(|line| *line != heading);
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("no entry for {heading}"));
    std::iter::once(first)
        .chain(lines.take_while(|line| !line.is_empty()))
        .collect()
}

/// The number of the text an entry names for `what`, from its line `  what: text N`.
fn number(entry: &[&str], what: &str) -> String {
    let start = format!("  {what}: text ");
    let line = entry.iter().find_map(|line| line.strip_prefix(&start));
    line.unwrap_or_else(|| panic!("{entry:?} names no text for {what}"))
        .to_owned()
}

#[test]
fn every_package_built_into_the_program_is_named_once_in_order() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&packages(root.path())), &shipped(LISTED)).unwrap();
    let headings = [
        "beta 2.1.0",
        "delta 0.1.0",
        "epaint_default_fonts 0.36.2",
        "gamma 0.3.0",
        "zeta 1.0.0",
    ];
    let at: Vec<usize> = headings
        .iter()
        .map(|heading| written.find(&format!("\n{heading}\n")).unwrap())
        .collect();
    assert!(at.is_sorted(), "{at:?}");
    for heading in headings {
        assert_eq!(
            written.matches(&format!("\n{heading}\n")).count(),
            1,
            "{heading}"
        );
    }
    // Voxa's own crate is not a third party, and a package not built in is not named.
    assert!(!written.contains("voxa-app 0.5.0") && !written.contains("Ours."));
    assert!(!written.contains("unused") && !written.contains("Never built in."));
    // What the metadata says of each.
    let zeta = entry(&written, "zeta 1.0.0");
    assert_eq!(
        zeta[1..3],
        [
            "  Licence: MIT OR Apache-2.0",
            "  Repository: https://example.com/zeta"
        ]
    );
}

#[test]
fn the_same_words_are_written_once_and_referred_to() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&packages(root.path())), &shipped(LISTED)).unwrap();
    assert_eq!(written.matches("Made-up terms.").count(), 1);
    let zeta = number(&entry(&written, "zeta 1.0.0"), "LICENSE-APACHE");
    let beta = entry(&written, "beta 2.1.0");
    assert_eq!(number(&beta, "license-apache"), zeta);
    // Only files whose names start as licences and notices do, and none in a folder.
    assert!(written.contains("Copyright 2021 Beta.") && number(&beta, "COPYRIGHT") != zeta);
    assert!(!written.contains("Not a licence.") && !written.contains("In a folder"));
    // One line end closes the file, and no blank line.
    assert!(written.ends_with("Made-up MIT terms.\n"), "{written}");
    // A text is called after where it is first met, in the order the packages are written.
    assert!(written.contains(&format!("Text {zeta}: license-apache of beta 2.1.0\n")));
    // The same metadata in another order writes the same file.
    let mut backwards = packages(root.path());
    backwards.reverse();
    assert_eq!(
        notices(&metadata(&backwards), &shipped(LISTED)).unwrap(),
        written
    );
}

#[test]
fn a_package_that_ships_no_text_is_given_the_standard_text_of_its_licence() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&packages(root.path())), &shipped(LISTED)).unwrap();
    let gamma = entry(&written, "gamma 0.3.0");
    let mit = number(&gamma, "MIT");
    assert!(gamma.contains(&"  Repository: not given"), "{gamma:?}");
    // Who holds the copyright, as far as the manifest says, without their addresses.
    assert!(
        gamma.contains(&"  Authors: Gam Ma, Another Author"),
        "{gamma:?}"
    );
    assert!(!written.contains("gam@example.com"));
    let kept = standard("MIT").unwrap();
    let heading = format!("Text {mit}: the standard text of MIT\n");
    assert!(written.contains(&format!("{heading}{}\n\n{kept}", "=".repeat(72))));
}

#[test]
fn the_typefaces_are_named_with_their_licences() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&packages(root.path())), &shipped(LISTED)).unwrap();
    let fonts = entry(&written, "epaint_default_fonts 0.36.2");
    let said = [
        ("Hack", "Bitstream Vera", "fonts/Hack-Regular.txt"),
        ("Ubuntu Light", "Ubuntu Font Licence 1.0", "fonts/UFL.txt"),
        ("Noto Emoji", "SIL Open Font Licence 1.1", "fonts/OFL.txt"),
        (
            "emoji-icon-font",
            "MIT licence",
            "fonts/emoji-icon-font-mit-license.txt",
        ),
    ];
    for (name, licence, file) in said {
        let text = number(&fonts, file);
        let line = written
            .lines()
            .find(|line| line.starts_with(&format!("  {name}")));
        let line = line.unwrap_or_else(|| panic!("{name} is not named"));
        assert!(
            line.contains(licence) && line.ends_with(&format!("text {text}")),
            "{line}"
        );
    }
    // The package's own code is under MIT or Apache 2.0, and ships no text of either; the
    // two font licences are in the typefaces' own files.
    number(&fonts, "MIT");
    number(&fonts, "Apache-2.0");
    assert!(
        !fonts
            .iter()
            .any(|line| line.starts_with("  OFL-1.1") || line.starts_with("  Ubuntu-font"))
    );
}

#[test]
fn a_package_under_the_mozilla_licence_says_where_its_source_is() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&packages(root.path())), &shipped(LISTED)).unwrap();
    let delta = entry(&written, "delta 0.1.0").join("\n");
    assert!(
        delta.contains("https://crates.io/crates/delta/0.1.0"),
        "{delta}"
    );
    let zeta = entry(&written, "zeta 1.0.0").join("\n");
    assert!(!zeta.contains("crates.io"), "{zeta}");
}

#[test]
fn what_cannot_be_written_truly_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    // No text ships, and none is kept for the licence.
    let mut some = packages(path);
    some.push(package(path, ("eta", "1.0.0"), "Unlicense", &[]));
    let listed = format!("{LISTED}eta v1.0.0\n");
    let said = notices(&metadata(&some), &shipped(&listed)).unwrap_err();
    assert!(
        said.contains("eta 1.0.0") && said.contains("Unlicense"),
        "{said}"
    );
    // A typeface that is not in the table of typefaces.
    let some = packages(path);
    let new_font = path.join("epaint_default_fonts-0.36.2/fonts/New-Font.ttf");
    fs::write(&new_font, "a font").unwrap();
    let said = notices(&metadata(&some), &shipped(LISTED)).unwrap_err();
    assert!(said.contains("New-Font.ttf"), "{said}");
    fs::remove_file(&new_font).unwrap();
    // A package the program is built from, that the metadata does not describe.
    let listed = format!("{LISTED}theta v0.0.1\n");
    let said = notices(&metadata(&packages(path)), &shipped(&listed)).unwrap_err();
    assert!(said.contains("theta 0.0.1"), "{said}");
}

#[test]
fn the_list_cargo_tree_gives_names_each_package_once() {
    let listed: Vec<(String, String)> = shipped(LISTED).into_iter().collect();
    let pairs = [
        ("beta", "2.1.0"),
        ("delta", "0.1.0"),
        ("epaint_default_fonts", "0.36.2"),
        ("gamma", "0.3.0"),
        ("voxa-app", "0.5.0"),
        ("zeta", "1.0.0"),
    ];
    let wanted: Vec<(String, String)> = pairs
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(listed, wanted);
    assert_eq!(shipped("\n\nnot a package\n"), BTreeSet::new());
}

#[test]
fn files_are_licences_and_notices_by_how_their_names_start() {
    for name in [
        "LICENSE",
        "LICENSE-MIT",
        "licence.txt",
        "COPYING",
        "Notice.md",
        "COPYRIGHT",
        "license-apache-2.0",
    ] {
        assert!(is_licence_file(name), "{name}");
    }
    for name in ["README.md", "UNLICENSE", "Cargo.toml", "src", "LICENS"] {
        assert!(!is_licence_file(name), "{name}");
    }
}

#[test]
fn a_licence_expression_names_its_licences() {
    assert_eq!(
        licence_ids("(MIT OR Apache-2.0) AND OFL-1.1"),
        ["MIT", "Apache-2.0", "OFL-1.1"]
    );
    assert_eq!(licence_ids("Apache-2.0/MIT"), ["Apache-2.0", "MIT"]);
    assert_eq!(
        licence_ids("Apache-2.0 WITH LLVM-exception OR MIT"),
        ["Apache-2.0", "MIT"]
    );
}

#[test]
fn every_licence_the_workspace_allows_has_a_standard_text() {
    let deny = fs::read_to_string(crate::workspace_root().join("deny.toml")).unwrap();
    let (_, licences) = deny.split_once("[licenses]").unwrap();
    let (_, allowed) = licences.split_once("allow = [").unwrap();
    let (allowed, _) = allowed.split_once(']').unwrap();
    let ids: Vec<&str> = allowed
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .filter_map(|line| line.trim().strip_prefix('"')?.strip_suffix("\","))
        .collect();
    assert!(ids.len() >= 10, "{ids:?}");
    for id in ids {
        let text = standard(id).unwrap_or_else(|| panic!("no standard text is kept for {id}"));
        assert!(text.len() > 500, "{id}");
    }
}

#[test]
fn the_committed_file_is_held_to_what_would_be_written_now() {
    assert_eq!(held_to(Some("one\r\ntwo\r\n"), "one\ntwo\n"), Ok(()));
    for committed in [Some("one\nthree\n"), None] {
        let said = held_to(committed, "one\ntwo\n").unwrap_err();
        assert!(said.contains("run `cargo xtask notices`"), "{said}");
    }
}
