//! The notices of made-up packages that offer a choice of licences, that come with no licence
//! file, whose repository keeps a text they leave out, or whose files another package's build
//! compiles in.

use std::path::Path;

use serde_json::{Value, json};

use super::standard::standard;
use super::tests::{APACHE, LISTED, entry, metadata, number, package, packages};
use super::upstream::{Extra, Generated, Holders, Known};
use super::{notices, shipped};

/// What the made-up workspace's notices know beyond its packages' folders.
pub(super) const KNOWN: Known = Known {
    holders: &[
        Holders {
            packages: &["gamma 0.3.0"],
            lines: &["Copyright (c) 2019 Gam Ma"],
            source: "https://example.com/gamma/LICENSE-MIT",
        },
        Holders {
            packages: &["epaint_default_fonts 0.36.2"],
            lines: &["Copyright (c) 2018 Font Maker"],
            source: "https://example.com/fonts/LICENSE-MIT",
        },
        Holders {
            packages: &["kappa 0.4.0"],
            lines: &[],
            source: "https://example.com/kappa/LICENSE-MIT",
        },
        Holders {
            packages: &["xi 3.1.0"],
            lines: &["Copyright (c) 2013 Made-up Registry Group"],
            source: "https://example.com/registry.xml",
        },
    ],
    extra: &[Extra {
        packages: &["kappa 0.4.0"],
        file: "LICENSE.other",
        covers: "its code derived from another's",
        source: "https://example.com/kappa/LICENSE.other",
        text: "Made-up other terms.\n",
    }],
    generated: &[Generated {
        package: "nu",
        from: "xi",
        what: "nu compiles in code its build makes from the registry this package holds",
    }],
};

/// A made-up MIT text whose file's name does not say what it is.
const MIT: &str = "MIT License\n\nPermission is granted to deal in the Software without \
                   restriction, made up.\n";

/// The made-up workspace with more packages: `lambda` and `mu` offer choices, `kappa` comes
/// with no licence file and its repository names no holder, and `nu`'s build compiles in the
/// files of `xi`, which comes with none either.
fn more(root: &Path) -> Vec<Value> {
    let lambda = [
        ("LICENSE-APACHE", APACHE),
        (
            "LICENSE-GPLv2",
            "GNU GENERAL PUBLIC LICENSE\nMade-up GPL.\n",
        ),
    ];
    let mu = [
        ("LICENSE", MIT),
        ("LICENSE-ZLIB.md", "Made-up zlib terms.\n"),
        ("LICENSE-APACHE.md", APACHE),
    ];
    let mut kappa = package(root, ("kappa", "0.4.0"), "MIT", &[]);
    kappa["authors"] = json!(["Kap Pa <kap@example.com>"]);
    let mut all = packages(root);
    all.extend([
        package(
            root,
            ("lambda", "1.3.0"),
            "Apache-2.0 OR GPL-2.0-only",
            &lambda,
        ),
        package(root, ("mu", "0.8.9"), "MIT OR Zlib OR Apache-2.0", &mu),
        kappa,
        package(root, ("nu", "0.6.1"), "Apache-2.0", &[("LICENSE", APACHE)]),
        package(root, ("xi", "3.1.0"), "Apache-2.0", &[]),
    ]);
    all
}

/// What `cargo tree` lists for the larger workspace: not `xi`, which only a build uses.
fn listed() -> String {
    format!("{LISTED}lambda v1.3.0\nmu v0.8.9\nkappa v0.4.0\nnu v0.6.1\n")
}

#[test]
fn of_a_choice_the_licence_taken_is_named_and_the_texts_of_the_others_are_left_out() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&more(root.path())), &shipped(&listed()), &KNOWN).unwrap();
    let lambda = entry(&written, "lambda 1.3.0");
    assert!(
        lambda.contains(&"  Voxa takes it under: Apache-2.0"),
        "{lambda:?}"
    );
    number(&lambda, "LICENSE-APACHE");
    assert!(!written.contains("LICENSE-GPLv2") && !written.contains("Made-up GPL."));
    // The zlib licence asks less than MIT; a file whose name does not say is known by its words.
    let mu = entry(&written, "mu 0.8.9");
    assert!(mu.contains(&"  Voxa takes it under: Zlib"), "{mu:?}");
    number(&mu, "LICENSE-ZLIB.md");
    assert!(
        !mu.iter().any(|line| line.starts_with("  LICENSE:")),
        "{mu:?}"
    );
    assert!(
        !mu.iter().any(|line| line.starts_with("  LICENSE-APACHE")),
        "{mu:?}"
    );
    let zeta = entry(&written, "zeta 1.0.0");
    assert!(!zeta.iter().any(|line| line.contains("APACHE")), "{zeta:?}");
    // A package that offers no choice says nothing of one.
    for heading in ["beta 2.1.0", "delta 0.1.0", "nu 0.6.1"] {
        let lines = entry(&written, heading);
        assert!(
            !lines.iter().any(|line| line.contains("takes it")),
            "{lines:?}"
        );
    }
}

#[test]
fn where_the_repository_names_no_holder_the_authors_are_named_and_its_extra_text_given() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&more(root.path())), &shipped(&listed()), &KNOWN).unwrap();
    let kappa = entry(&written, "kappa 0.4.0");
    let said = [
        "  Authors: Kap Pa",
        "  Copyright: no holder is named in https://example.com/kappa/LICENSE-MIT",
        "  Its code comes with no licence file; these standard texts stand in:",
    ];
    assert_eq!(kappa[3..6], said, "{kappa:?}");
    // The standard text, with the line it leaves for the holders taken out, as theirs has it.
    let kept = standard("MIT").unwrap();
    let without = kept.replace("Copyright (c) <year> <copyright holders>\n\n", "");
    assert!(without.len() < kept.len() && !without.contains("<year>"));
    let text = number(&kappa, "MIT");
    let heading = "the standard text of MIT, with no copyright line, as in its repository";
    let heading = format!("Text {text}: {heading}\n");
    assert!(written.contains(&format!("{heading}{}\n\n{without}", "=".repeat(72))));
    // The text its repository keeps for code in it, which the package leaves out.
    let extra = number(&kappa, "LICENSE.other, for its code derived from another's");
    let heading = "LICENSE.other of kappa's repository, https://example.com/kappa/LICENSE.other";
    let heading = format!("Text {extra}: {heading}\n");
    assert!(written.contains(&format!(
        "{heading}{}\n\nMade-up other terms.\n",
        "=".repeat(72)
    )));
}

#[test]
fn a_package_whose_files_a_build_compiles_in_is_named_with_why() {
    let root = tempfile::tempdir().unwrap();
    let written = notices(&metadata(&more(root.path())), &shipped(&listed()), &KNOWN).unwrap();
    let xi = entry(&written, "xi 3.1.0");
    let said = [
        "  Built in: nu compiles in code its build makes from the registry this package holds",
        "  Copyright: Copyright (c) 2013 Made-up Registry Group",
        "  As written in: https://example.com/registry.xml",
    ];
    for words in said {
        assert!(xi.contains(&words), "{xi:?}");
    }
    // The standard Apache text leaves no line for the holders, so it is given as it is.
    let text = number(&xi, "Apache-2.0");
    assert!(written.contains(&format!("Text {text}: the standard text of Apache-2.0\n")));
    // Six packages of the smaller workspace, four more in the program, and `xi`.
    assert!(
        written.contains("built from the 11 packages below"),
        "{written}"
    );
    // Without `nu` in the program, `xi` is not either.
    let without_nu = listed().replace("nu v0.6.1\n", "");
    let written = notices(&metadata(&more(root.path())), &shipped(&without_nu), &KNOWN).unwrap();
    assert!(!written.contains("xi 3.1.0") && !written.contains("Made-up Registry"));
}

#[test]
fn what_the_notices_cannot_know_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    // A package with no licence file whose holders xtask does not know, and a new version of
    // one it knows, which is looked at again.
    for (name, version) in [("omicron", "0.1.0"), ("gamma", "0.3.1")] {
        let mut some = more(path);
        some.push(package(path, (name, version), "MIT", &[]));
        let listing = format!("{}{name} v{version}\n", listed());
        let said = notices(&metadata(&some), &shipped(&listing), &KNOWN).unwrap_err();
        assert!(
            said.contains(&format!("{name} {version}")) && said.contains("HOLDERS"),
            "{said}"
        );
    }
    // Two packages of the name whose files a build compiles in: which is meant is not known.
    let mut some = more(path);
    some.push(package(path, ("xi", "4.0.0"), "Apache-2.0", &[]));
    let said = notices(&metadata(&some), &shipped(&listed()), &KNOWN).unwrap_err();
    assert!(
        said.contains("nu") && said.contains("2 packages called xi"),
        "{said}"
    );
}
