//! What the notices must know of a package that its folder does not say: who holds the
//! copyright of a package that comes with no licence file, as its repository's licence file
//! says; licence texts its repository keeps for code in it that its package leaves out; and
//! packages whose files another package's build turns into code compiled into Volsa.
//!
//! Each line here is copied from the file it names, at the commit or tag the package was
//! published from (the commit is in the package's `.cargo_vcs_info.json`).

use std::collections::BTreeSet;

use serde_json::Value;

use super::package::Package;

/// Who holds the copyright of packages that come with no licence file, from one licence file
/// in their repository.
pub struct Holders {
    /// The packages, by name and version as the notices head them: a new version of one is
    /// looked at again before the notices can be written.
    pub packages: &'static [&'static str],
    /// Its copyright lines, word for word: none where the file names no holder.
    pub lines: &'static [&'static str],
    /// The file the lines are read from.
    pub source: &'static str,
}

/// A licence text a repository keeps for code in its packages that the packages leave out.
pub struct Extra {
    /// The packages, by name and version, whose code the text covers.
    pub packages: &'static [&'static str],
    /// What the file is called in the repository.
    pub file: &'static str,
    /// The code it covers.
    pub covers: &'static str,
    /// Where the file is.
    pub source: &'static str,
    /// Its text, as kept in `xtask/licences`.
    pub text: &'static str,
}

/// A package whose build turns files of another package into code compiled into Volsa, so
/// that the other's notice goes with Volsa too though it is only a build dependency.
pub struct Generated {
    /// The package that is built into Volsa.
    pub package: &'static str,
    /// The package, a dependency of its build, whose files the code is made from.
    pub from: &'static str,
    /// What the code is, as the notices say it.
    pub what: &'static str,
}

/// What the notices know beyond the packages' folders.
pub struct Known {
    /// Who holds the copyright of each package that comes with no licence file.
    pub holders: &'static [Holders],
    /// Texts that packages leave out.
    pub extra: &'static [Extra],
    /// Packages whose files another's build compiles into Volsa.
    pub generated: &'static [Generated],
}

/// What is known of the packages Volsa is built from now.
pub const KNOWN: Known = Known {
    holders: &HOLDERS,
    extra: &EXTRA,
    generated: &GENERATED,
};

/// The repository of the interface library: its licence file at the commit 0.36.2 was
/// published from.
const EGUI: &str =
    "https://github.com/emilk/egui/blob/49682f8baa058bf49e011035cfbd6e825f88a5ef/LICENSE-MIT";

/// Who holds the copyright of each package Volsa is built from that comes with no licence file.
const HOLDERS: [Holders; 10] = [
    Holders {
        packages: &["accesskit 0.24.1"],
        lines: &[],
        source: "https://github.com/AccessKit/accesskit/blob/a55d3e1a18bb9ef0e4bccc9083fb13c3e0ad8969/LICENSE-MIT",
    },
    Holders {
        packages: &["accesskit_consumer 0.35.0", "accesskit_windows 0.32.1"],
        lines: &[],
        source: "https://github.com/AccessKit/accesskit/blob/1bbcf100942bac96c2c3a4a91cb67b0b20201a24/LICENSE-MIT",
    },
    Holders {
        packages: &["accesskit_winit 0.32.2"],
        lines: &[],
        source: "https://github.com/AccessKit/accesskit/blob/1bbcf100942bac96c2c3a4a91cb67b0b20201a24/LICENSE-APACHE",
    },
    Holders {
        packages: &["clipboard-win 5.4.3"],
        lines: &[],
        source: "https://github.com/DoumanAsh/clipboard-win/blob/ed27008ae5d1f10cfbb266de039f0fb35fd05a56/LICENSE",
    },
    // The commits 0.10.0 was published from are not on GitHub; the tag v0.10.0 is.
    Holders {
        packages: &[
            "dicom-core 0.10.0",
            "dicom-dictionary-std 0.10.0",
            "dicom-encoding 0.10.0",
            "dicom-object 0.10.0",
            "dicom-parser 0.10.0",
            "dicom-pixeldata 0.10.0",
            "dicom-transfer-syntax-registry 0.10.0",
        ],
        lines: &["Copyright (c) 2016 Eduardo Pinho"],
        source: "https://github.com/Enet4/dicom-rs/blob/2941c54cae434dd63a54d0a5546b3b0df42e9f6c/LICENSE-MIT",
    },
    Holders {
        packages: &[
            "ecolor 0.36.2",
            "eframe 0.36.2",
            "egui 0.36.2",
            "egui-wgpu 0.36.2",
            "egui-winit 0.36.2",
            "emath 0.36.2",
            "epaint 0.36.2",
            "epaint_default_fonts 0.36.2",
        ],
        lines: &["Copyright (c) 2018-2021 Emil Ernerfeldt <emil.ernerfeldt@gmail.com>"],
        source: EGUI,
    },
    // Public-domain tables: their files dedicate any copyright to the public domain (CC0).
    Holders {
        packages: &[
            "encoding-index-japanese 1.20141219.5",
            "encoding-index-korean 1.20141219.5",
            "encoding-index-simpchinese 1.20141219.5",
            "encoding-index-singlebyte 1.20141219.5",
            "encoding-index-tradchinese 1.20141219.5",
            "encoding_index_tests 0.1.4",
        ],
        lines: &[],
        source: "https://github.com/lifthrasiir/rust-encoding/tree/eb3d3c307df864f6a25e2ca16d49703e5d963ec5/src/index",
    },
    Holders {
        packages: &["profiling 1.0.18"],
        lines: &["Copyright (c) 2020 Philip Degarmo and other contributors"],
        source: "https://github.com/aclysma/profiling/blob/8271551172eb6fa4cba47369aedd93790c623df9/LICENSE-MIT",
    },
    Holders {
        packages: &["spirv 0.4.0+sdk-1.4.341.0"],
        lines: &[],
        source: "https://github.com/gfx-rs/rspirv/blob/8afc3d0ac8e158128cd1410bb2e4b4c26ab11bb4/LICENSE",
    },
    // The code compiled in is made from the WGL registry, `api/xml/wgl.xml` in the package,
    // a copy of this file; gl-rs's own licence file names no holder.
    Holders {
        packages: &["khronos_api 3.1.0"],
        lines: &["Copyright (c) 2013-2018 The Khronos Group Inc."],
        source: "https://github.com/KhronosGroup/OpenGL-Registry/blob/85d924a5165e7206f4c73c8c10307658dc2bbe10/xml/wgl.xml",
    },
];

/// Texts packages Volsa is built from leave out. AccessKit's code derived from Chromium's says
/// that it is under the BSD-style licence in the repository's `LICENSE.chromium`.
const EXTRA: [Extra; 1] = [Extra {
    packages: &[
        "accesskit 0.24.1",
        "accesskit_consumer 0.35.0",
        "accesskit_windows 0.32.1",
    ],
    file: "LICENSE.chromium",
    covers: "its code derived from Chromium's",
    source: "https://github.com/AccessKit/accesskit/blob/1bbcf100942bac96c2c3a4a91cb67b0b20201a24/LICENSE.chromium",
    text: include_str!("../../licences/accesskit-LICENSE.chromium.txt"),
}];

/// `glutin_wgl_sys`'s build generates its bindings to WGL from the registry in `khronos_api`.
const GENERATED: [Generated; 1] = [Generated {
    package: "glutin_wgl_sys",
    from: "khronos_api",
    what: "glutin_wgl_sys compiles in bindings to WGL that its build makes from the \
           Khronos XML API Registry this package holds",
}];

/// What [`KNOWN`] says of who holds the copyright of `package`, when it comes with no licence
/// file. A package it does not name is an error: the standard text of its licence would go
/// out with no holder in it.
pub fn holders_of(package: &Package, known: &Known) -> Result<&'static Holders, String> {
    let heading = package.heading();
    let named = |each: &&Holders| each.packages.contains(&heading.as_str());
    known.holders.iter().find(named).ok_or_else(|| {
        let what = format!("{heading} comes with no licence text");
        format!(
            "{what}: add who holds its copyright, from the licence file in its repository, to \
             HOLDERS in xtask/src/notices/upstream.rs"
        )
    })
}

/// A standard licence text with the copyright lines of `holders` in place of the line it
/// leaves for them, and what it is then called; `None` for a text that leaves no such line.
/// Where the holders' file names no holder, that line goes.
pub fn with_holders(text: &str, holders: &Holders, of: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let blank = |line: &str| line.starts_with("Copyright") && line.contains("<year>");
    let at = lines.iter().position(|line| blank(line))?;
    let (before, after) = lines.split_at(at);
    let mut after = after.iter().skip(1).copied().peekable();
    let mut written: Vec<&str> = before.to_vec();
    if holders.lines.is_empty() {
        // The blank line after the copyright line goes with it.
        after.next_if(|line| line.trim().is_empty());
    } else {
        written.extend(holders.lines);
    }
    written.extend(after);
    let called = if holders.lines.is_empty() {
        "with no copyright line, as in its repository".to_owned()
    } else {
        format!("with the copyright line of {of}")
    };
    Some((super::tidy(&written.join("\n")), called))
}

/// The packages `shipped` names, with each package whose files the build of a shipped
/// package turns into code compiled into Volsa, by [`Known::generated`]. That package must be
/// in `metadata` once.
pub fn with_generated(
    metadata: &Value,
    shipped: &BTreeSet<(String, String)>,
    known: &Known,
) -> Result<BTreeSet<(String, String)>, String> {
    let described = metadata["packages"].as_array().map(Vec::as_slice);
    let described = described.unwrap_or_default();
    let mut all = shipped.clone();
    for generated in known.generated {
        if !shipped.iter().any(|(name, _)| name == generated.package) {
            continue;
        }
        let named = |each: &&Value| each["name"].as_str() == Some(generated.from);
        let versions = described.iter().filter(named);
        let versions: Vec<&str> = versions
            .filter_map(|each| each["version"].as_str())
            .collect();
        let [version] = versions.as_slice() else {
            let (package, from) = (generated.package, generated.from);
            return Err(format!(
                "{package} is built into Volsa with code made from {from}, and cargo metadata \
                 describes {} packages called {from}",
                versions.len()
            ));
        };
        all.insert((generated.from.to_owned(), (*version).to_owned()));
    }
    Ok(all)
}
