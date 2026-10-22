//! Which of the licences a package offers Voxa takes it under, and which licence a text that
//! comes with a package is the text of.

use std::fs;

use super::choice::{PERMISSIVE, covers, taken};
use super::package::is_licence_file;
use super::standard::licence_ids;

/// No licence.
const NONE: [&str; 0] = [];

#[test]
fn of_a_choice_the_most_permissive_licence_voxa_accepts_is_taken() {
    let cases: [(&str, Option<&[&str]>); 18] = [
        ("MIT OR Apache-2.0", Some(&["MIT"])),
        ("Apache-2.0 OR MIT", Some(&["MIT"])),
        ("MIT/Apache-2.0", Some(&["MIT"])),
        ("Apache-2.0 OR GPL-2.0-only", Some(&["Apache-2.0"])),
        ("MIT OR Apache-2.0 OR Zlib", Some(&["Zlib"])),
        ("BSD-3-Clause OR Apache-2.0", Some(&["BSD-3-Clause"])),
        ("BSD-2-Clause OR Apache-2.0 OR MIT", Some(&["MIT"])),
        // A licence the workspace does not accept is never taken, though it asks less.
        ("0BSD OR MIT OR Apache-2.0", Some(&["MIT"])),
        ("Unlicense OR MIT", Some(&["MIT"])),
        // Licences that must all be kept, with a choice made among the others.
        (
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            Some(&["MIT", "Unicode-3.0"]),
        ),
        (
            "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0",
            Some(&["MIT", "OFL-1.1", "Ubuntu-font-1.0"]),
        ),
        ("Apache-2.0 AND MIT", Some(&["Apache-2.0", "MIT"])),
        ("Apache-2.0 WITH LLVM-exception OR MIT", Some(&["MIT"])),
        ("MIT", Some(&["MIT"])),
        // Nothing the workspace accepts is on offer, or the expression cannot be read.
        ("Unlicense", None),
        ("GPL-2.0-only OR Unlicense", None),
        ("MIT OR", None),
        ("(MIT OR Apache-2.0", None),
    ];
    for (expression, wanted) in cases {
        assert_eq!(taken(expression).as_deref(), wanted, "{expression}");
    }
}

#[test]
fn every_licence_the_workspace_allows_is_ranked_and_no_other() {
    let allowed = allowed();
    assert_eq!(PERMISSIVE.len(), allowed.len(), "{allowed:?}");
    for id in allowed {
        assert!(PERMISSIVE.contains(&id.as_str()), "{id} is not ranked");
    }
}

#[test]
fn a_text_is_known_by_its_name_or_else_by_its_words() {
    let offered = ["MIT", "Apache-2.0", "Zlib", "GPL-2.0-only", "BSD-3-Clause"];
    let named: [(&str, &[&str]); 9] = [
        ("LICENSE-MIT", &["MIT"]),
        ("LICENSE-APACHE.md", &["Apache-2.0"]),
        ("license-apache-2.0", &["Apache-2.0"]),
        ("LICENSE.MIT", &["MIT"]),
        ("LICENSE-GPLv2", &["GPL-2.0-only"]),
        ("LICENSE-ZLIB.md", &["Zlib"]),
        ("LICENSE-LIBM-MIT", &["MIT"]),
        ("LICENSE-BSD", &["BSD-3-Clause"]),
        ("LICENSE-0BSD", &[]),
    ];
    for (name, wanted) in named {
        assert_eq!(covers(name, "", &offered), wanted, "{name}");
    }
    // A name that does not say is read: the words only a licence's full text has.
    let mit = "MIT License\n\nCopyright (c) 2017 Someone\n\nPermission is hereby granted, \
               to deal in the  Software\nwithout restriction, and so on.";
    assert_eq!(covers("LICENSE", mit, &offered), ["MIT"]);
    let both = format!(
        "{mit}\n\nApache License\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION"
    );
    assert_eq!(
        covers("LICENSE.txt", &both, &offered),
        ["MIT", "Apache-2.0"]
    );
    let zlib = "Altered source versions must be plainly marked as such.";
    assert_eq!(covers("LICENSE", zlib, &offered), ["Zlib"]);
    // A note of which licences apply is the text of none.
    let note = "Licensed under the Apache License, Version 2.0 or the MIT license, at your option.";
    assert_eq!(covers("COPYRIGHT", note, &offered), NONE);
    assert_eq!(covers("LICENSE", "MIT OR Apache-2.0", &offered), NONE);
    // Only licences on offer are named.
    assert_eq!(covers("LICENSE-MIT", mit, &["Apache-2.0"]), NONE);
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

/// The licences `deny.toml` allows, by their SPDX names.
pub(super) fn allowed() -> Vec<String> {
    let deny = fs::read_to_string(crate::workspace_root().join("deny.toml")).unwrap();
    let (_, licences) = deny.split_once("[licenses]").unwrap();
    let (_, allowed) = licences.split_once("allow = [").unwrap();
    let (allowed, _) = allowed.split_once(']').unwrap();
    allowed
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .filter_map(|line| line.trim().strip_prefix('"')?.strip_suffix("\","))
        .map(str::to_owned)
        .collect()
}
