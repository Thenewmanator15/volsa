//! The texts written once each at the end of the notices, and how every text is tidied.

use std::collections::HashMap;

use super::RULE;

/// A text as every text in the file is written: lines ending with `\n` and no spaces after
/// them, and no blank lines before or after.
pub fn tidy(text: &str) -> String {
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
pub struct Texts {
    /// Each text, after what it is called where it is first met.
    written: Vec<(String, String)>,
    /// The number of each text, by its words with one space between each two.
    numbers: HashMap<String, usize>,
}

impl Texts {
    /// The number of `text`, which is given one when it is new; `called` says what it is.
    pub fn number(&mut self, text: &str, called: impl FnOnce() -> String) -> usize {
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if let Some(&number) = self.numbers.get(&words) {
            return number;
        }
        self.written.push((called(), text.to_owned()));
        self.numbers.insert(words, self.written.len());
        self.written.len()
    }

    /// Every text, each under its number and what it is.
    pub fn written(&self) -> String {
        let each = self.written.iter().enumerate();
        let each = each.map(|(at, (called, text))| {
            format!("{RULE}\nText {}: {called}\n{RULE}\n\n{text}\n", at + 1)
        });
        each.collect()
    }
}
