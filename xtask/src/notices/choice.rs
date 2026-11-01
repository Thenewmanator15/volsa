//! Which of the licences a package offers Volsa takes it under, and which licence a text that
//! comes with a package is the text of.

use super::package::LICENCE_FILES;

/// The licences Volsa accepts, the ones `deny.toml` allows, from the one that asks least of a
/// program built from the code to the one that asks most. Of a choice, Volsa takes the first
/// of these on offer.
///
/// A public-domain dedication asks nothing. The Boost and zlib licences ask nothing of a
/// program shipped as machine code. MIT and ISC ask that their notice go with every copy; the
/// BSD licences ask it of the documentation, the three-clause one also that no one's name be
/// used to endorse it; Unicode's and the Independent JPEG Group's ask for a notice too. The
/// Apache licence adds its NOTICE file, marked changes and its terms on patents, and the
/// Mozilla licence keeps changed files under itself. The two font licences are never on offer
/// as a choice. A licence not here, such as 0BSD or the Unlicense, is never taken, even where
/// it asks less, until `deny.toml` allows it.
pub const PERMISSIVE: [&str; 13] = [
    "CC0-1.0",
    "BSL-1.0",
    "Zlib",
    "MIT",
    "ISC",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Unicode-3.0",
    "IJG",
    "Apache-2.0",
    "MPL-2.0",
    "OFL-1.1",
    "Ubuntu-font-1.0",
];

/// Words found only in the full text of a licence, by how its SPDX name starts in capitals:
/// they tell what a text is whose file name does not say.
const TELLS: [(&str, &str); 6] = [
    ("MIT", "to deal in the Software without restriction"),
    (
        "APACHE",
        "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION",
    ),
    ("ZLIB", "Altered source versions must be plainly marked"),
    ("BSD", "Redistribution and use in source and binary forms"),
    ("BSL", "Boost Software License"),
    ("GPL", "GNU General Public License"),
];

/// A licence expression, as SPDX writes one.
enum Expression<'a> {
    /// One licence, by its SPDX name.
    One(&'a str),
    /// Every one of these.
    All(Vec<Expression<'a>>),
    /// Any one of these.
    Any(Vec<Expression<'a>>),
}

/// The licences Volsa takes a package under, by the SPDX expression its manifest gives: of
/// each choice, the alternative whose least permissive licence comes first in
/// [`PERMISSIVE`], and every licence the package must be taken under besides. `None` when
/// a choice offers nothing Volsa accepts, or the expression cannot be read.
pub fn taken(expression: &str) -> Option<Vec<&str>> {
    let mut reader = Reader {
        tokens: tokens(expression),
        at: 0,
    };
    let read = reader.any()?;
    if reader.at != reader.tokens.len() {
        return None;
    }
    chosen(&read)
}

/// Which of `offered` a file is the text of, by its name where the name says, as
/// `LICENSE-MIT` does, and otherwise by the words of `text`. None for a text that is neither,
/// such as a note of which licences apply.
pub fn covers<'a>(name: &str, text: &str, offered: &[&'a str]) -> Vec<&'a str> {
    let name = name.to_ascii_uppercase();
    let rest = LICENCE_FILES
        .iter()
        .find_map(|start| name.strip_prefix(start));
    let words: Vec<&str> = rest
        .unwrap_or(&name)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let named = |id: &&str| words.iter().any(|word| word.starts_with(&start(id)));
    let by_name: Vec<&str> = offered.iter().copied().filter(named).collect();
    if !by_name.is_empty() {
        return by_name;
    }
    let said = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let said = said.to_ascii_uppercase();
    let told = |id: &&str| {
        let tells = TELLS.iter().filter(|(of, _)| *of == start(id));
        tells
            .into_iter()
            .any(|(_, words)| said.contains(&words.to_ascii_uppercase()))
    };
    offered.iter().copied().filter(told).collect()
}

/// How a licence's SPDX name starts, in capitals: `APACHE` for `Apache-2.0`.
fn start(id: &str) -> String {
    id.split('-').next().unwrap_or(id).to_ascii_uppercase()
}

/// The words of an expression: names, `AND`, `OR`, `WITH`, brackets and the old `/` for OR.
fn tokens(expression: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut from = None;
    for (at, c) in expression.char_indices() {
        let alone = matches!(c, '(' | ')' | '/');
        if c.is_whitespace() || alone {
            if let Some(start) = from.take() {
                tokens.extend(expression.get(start..at));
            }
            if alone {
                tokens.extend(expression.get(at..at + c.len_utf8()));
            }
        } else if from.is_none() {
            from = Some(at);
        }
    }
    if let Some(start) = from {
        tokens.extend(expression.get(start..));
    }
    tokens
}

/// Reads an expression's words: OR joins less tightly than AND, which joins less tightly than
/// WITH, and brackets group.
struct Reader<'a> {
    /// The words.
    tokens: Vec<&'a str>,
    /// The next word to read.
    at: usize,
}

impl<'a> Reader<'a> {
    /// The next word, read.
    fn next(&mut self) -> Option<&'a str> {
        let token = self.tokens.get(self.at).copied();
        self.at += 1;
        token
    }

    /// Whether the next word is one of `words`; it is read if it is.
    fn next_is(&mut self, words: &[&str]) -> bool {
        let is = self.tokens.get(self.at).is_some_and(|t| words.contains(t));
        if is {
            self.at += 1;
        }
        is
    }

    /// Alternatives joined by OR.
    fn any(&mut self) -> Option<Expression<'a>> {
        let mut alternatives = vec![self.all()?];
        while self.next_is(&["OR", "/"]) {
            alternatives.push(self.all()?);
        }
        Some(Self::joined(alternatives, Expression::Any))
    }

    /// Licences joined by AND.
    fn all(&mut self) -> Option<Expression<'a>> {
        let mut parts = vec![self.one()?];
        while self.next_is(&["AND"]) {
            parts.push(self.one()?);
        }
        Some(Self::joined(parts, Expression::All))
    }

    /// One licence, perhaps with an exception, or an expression in brackets.
    fn one(&mut self) -> Option<Expression<'a>> {
        match self.next()? {
            "(" => {
                let inside = self.any()?;
                (self.next()? == ")").then_some(inside)
            }
            ")" | "/" | "AND" | "OR" | "WITH" => None,
            id => {
                if self.next_is(&["WITH"]) {
                    self.next()?;
                }
                Some(Expression::One(id))
            }
        }
    }

    /// The one expression of `each`, or `join` of them when there are more.
    fn joined(
        each: Vec<Expression<'a>>,
        join: fn(Vec<Expression<'a>>) -> Expression<'a>,
    ) -> Expression<'a> {
        match <[_; 1]>::try_from(each) {
            Ok([only]) => only,
            Err(each) => join(each),
        }
    }
}

/// The licences an expression is taken under, each once, or `None` if it offers nothing
/// Volsa accepts.
fn chosen<'a>(expression: &Expression<'a>) -> Option<Vec<&'a str>> {
    match expression {
        Expression::One(id) => rank(id).map(|_| vec![*id]),
        Expression::All(parts) => {
            let mut ids = Vec::new();
            for part in parts {
                for id in chosen(part)? {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
            Some(ids)
        }
        Expression::Any(alternatives) => alternatives
            .iter()
            .filter_map(chosen)
            .min_by_key(|ids| (ids.iter().filter_map(|id| rank(id)).max(), ids.len())),
    }
}

/// Where a licence stands in [`PERMISSIVE`], if Volsa accepts it.
fn rank(id: &str) -> Option<usize> {
    PERMISSIVE.iter().position(|each| *each == id)
}
