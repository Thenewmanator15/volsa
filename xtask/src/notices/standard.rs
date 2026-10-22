//! The standard texts of the licences the workspace allows, for a package that comes with no
//! text of its own, and the typefaces the interface library builds into the program.

/// The standard text of each licence `deny.toml` allows, by its SPDX name. Each is copied from
/// a package on crates.io that carries the licence's own text. Where the licence begins with a
/// copyright line, the year and holders are left for the reader, as the licence gives them.
const STANDARD: [(&str, &str); 13] = [
    ("Apache-2.0", include_str!("../../licences/Apache-2.0.txt")),
    (
        "BSD-2-Clause",
        include_str!("../../licences/BSD-2-Clause.txt"),
    ),
    (
        "BSD-3-Clause",
        include_str!("../../licences/BSD-3-Clause.txt"),
    ),
    ("BSL-1.0", include_str!("../../licences/BSL-1.0.txt")),
    ("CC0-1.0", include_str!("../../licences/CC0-1.0.txt")),
    ("IJG", include_str!("../../licences/IJG.txt")),
    ("ISC", include_str!("../../licences/ISC.txt")),
    ("MIT", include_str!("../../licences/MIT.txt")),
    ("MPL-2.0", include_str!("../../licences/MPL-2.0.txt")),
    ("OFL-1.1", include_str!("../../licences/OFL-1.1.txt")),
    (
        "Ubuntu-font-1.0",
        include_str!("../../licences/Ubuntu-font-1.0.txt"),
    ),
    (
        "Unicode-3.0",
        include_str!("../../licences/Unicode-3.0.txt"),
    ),
    ("Zlib", include_str!("../../licences/Zlib.txt")),
];

/// The standard text of the licence SPDX names `id`, tidied as every text in the file is, or
/// `None` when none is kept for it.
pub fn standard(id: &str) -> Option<String> {
    let kept = STANDARD.iter().find(|(name, _)| *name == id);
    kept.map(|(_, text)| super::tidy(text))
}

/// The licences an SPDX expression names, in order and each once: not the words that join
/// them, and not an exception named after `WITH`. The old form `MIT/Apache-2.0` reads as OR.
pub fn licence_ids(expression: &str) -> Vec<&str> {
    let parted = |c: char| c.is_whitespace() || matches!(c, '(' | ')' | '/');
    let mut ids = Vec::new();
    let mut exception = false;
    for word in expression.split(parted).filter(|word| !word.is_empty()) {
        if std::mem::take(&mut exception) {
            continue;
        }
        match word {
            "AND" | "OR" => {}
            "WITH" => exception = true,
            id if !ids.contains(&id) => ids.push(id),
            _ => {}
        }
    }
    ids
}

/// The package of the interface library that holds the typefaces it builds into a program.
pub const FONTS: &str = "epaint_default_fonts";
/// The folder in that package that holds the typefaces and their licences.
pub const FONTS_FOLDER: &str = "fonts";

/// A typeface the interface library builds into the program.
pub struct Typeface {
    /// What it is called.
    pub name: &'static str,
    /// Its licence, in words.
    pub licence: &'static str,
    /// Its font file, in [`FONTS_FOLDER`].
    pub font: &'static str,
    /// The file beside it that holds its licence.
    pub text: &'static str,
    /// The licence of the package's own expression that this file is the text of, if any.
    pub covers: Option<&'static str>,
}

/// Every typeface in [`FONTS`]: a font file there that is not named here stops the notices
/// being written, so that a new typeface is never shipped without its licence.
pub const TYPEFACES: [Typeface; 4] = [
    Typeface {
        name: "Hack Regular",
        licence: "MIT licence, and the Bitstream Vera licence for its glyphs from Bitstream Vera",
        font: "Hack-Regular.ttf",
        text: "Hack-Regular.txt",
        covers: None,
    },
    Typeface {
        name: "Ubuntu Light",
        licence: "Ubuntu Font Licence 1.0",
        font: "Ubuntu-Light.ttf",
        text: "UFL.txt",
        covers: Some("Ubuntu-font-1.0"),
    },
    Typeface {
        name: "Noto Emoji Regular",
        licence: "SIL Open Font Licence 1.1",
        font: "NotoEmoji-Regular.ttf",
        text: "OFL.txt",
        covers: Some("OFL-1.1"),
    },
    Typeface {
        name: "emoji-icon-font",
        licence: "MIT licence",
        font: "emoji-icon-font.ttf",
        text: "emoji-icon-font-mit-license.txt",
        covers: None,
    },
];
