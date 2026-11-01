//! How a query is matched to what search can find: case and accents ignored, whole words and
//! starts of words before letters scattered through a name. Nothing here draws or keeps
//! anything.

use std::cmp::Reverse;
use std::collections::HashMap;

use super::entries::Entry;

/// The most results listed under one heading.
pub const MOST_PER_HEADING: usize = 8;

/// The score of a name that is the whole query.
const WHOLE_NAME: u32 = 5_000;
/// The score of a name the query starts.
const NAME_STARTS: u32 = 4_000;
/// The score of a name with a word the query starts.
const NAME_WORD: u32 = 3_000;
/// The score of something one of whose other words, or phrases, is the whole query (or, of a
/// query of several words, one of them): a word it is searched by, spelt out, is as good a
/// match as a word of its name.
const OTHER_IS_QUERY: u32 = NAME_WORD;
/// The score of something found by one of its other words.
const OTHER_WORD: u32 = 2_000;
/// The score of a name holding the query's letters in order, less one for each letter
/// skipped between the first and the last.
const SCATTERED: u32 = 1_000;

/// Lower case, with the accents of the Latin letters taken off: `Éé Ü` becomes `ee u`.
pub fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for letter in text.chars().flat_map(char::to_lowercase) {
        // An accent written apart from its letter goes the same way.
        if ('\u{300}'..='\u{36f}').contains(&letter) {
            continue;
        }
        match unaccented(letter) {
            Some(plain) => folded.push_str(plain),
            None => folded.push(letter),
        }
    }
    folded
}

/// What a lower-case letter with an accent, or one the alphabet spells with two letters, is
/// written as; `None` for any other.
fn unaccented(letter: char) -> Option<&'static str> {
    Some(match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'þ' => "th",
        _ => return None,
    })
}

/// The words of a text, folded: runs of letters and digits, whatever is between them.
fn words_of(text: &str) -> Vec<String> {
    let folded = fold(text);
    let words = folded.split(|letter: char| !letter.is_alphanumeric());
    words
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// How well `query` matches something called `name` that can also be found by `words`, the
/// higher the better; `None` if it does not match.
///
/// In falling order, the kinds of match are: the whole name is the query; the name starts with
/// it; a word of the name starts with it, or one of `words` is the query; one of `words` starts
/// with it; the query's letters are in the name in order, the better the fewer letters they skip.
///
/// A query of several words matches only if each word does, and scores as its weakest word
/// does, unless the whole query is, or starts, the name, or is one of `words`.
pub fn score(query: &str, name: &str, words: &[String]) -> Option<u32> {
    let wanted = words_of(query);
    if wanted.is_empty() {
        return None;
    }
    let whole_query = wanted.join(" ");
    let name_words = words_of(name);
    let whole_name = name_words.join(" ");
    let others: Vec<String> = words.iter().flat_map(|other| words_of(other)).collect();
    let is_query = |other: &String| words_of(other).join(" ") == whole_query;
    let whole = if whole_name == whole_query {
        Some(WHOLE_NAME)
    } else if whole_name.starts_with(&whole_query) {
        Some(NAME_STARTS)
    } else if words.iter().any(is_query) {
        Some(OTHER_IS_QUERY)
    } else {
        None
    };
    let by_word = || {
        let scores = wanted
            .iter()
            .map(|word| word_score(word, &whole_name, &name_words, &others));
        let scores: Option<Vec<u32>> = scores.collect();
        scores?.into_iter().min()
    };
    whole.into_iter().chain(by_word()).max()
}

/// How well one word of a query matches a name, given as it is whole, as its words, and the
/// other words the thing can be found by.
fn word_score(word: &str, name: &str, name_words: &[String], others: &[String]) -> Option<u32> {
    if name.starts_with(word) {
        Some(NAME_STARTS)
    } else if name_words.iter().any(|one| one.starts_with(word)) {
        Some(NAME_WORD)
    } else if others.iter().any(|one| one == word) {
        Some(OTHER_IS_QUERY)
    } else if others.iter().any(|one| one.starts_with(word)) {
        Some(OTHER_WORD)
    } else {
        let skipped = letters_skipped(word, name)?;
        let skipped = u32::try_from(skipped).unwrap_or(SCATTERED);
        Some(SCATTERED.saturating_sub(skipped).max(1))
    }
}

/// How many letters of `name` lie between the first and the last of the letters of `word`,
/// found in order, where they lie closest together; `None` if they are not all in it.
fn letters_skipped(word: &str, name: &str) -> Option<usize> {
    let wanted: Vec<char> = word.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let first = *wanted.first()?;
    let starts = name
        .iter()
        .enumerate()
        .filter(|(_, letter)| **letter == first)
        .map(|(start, _)| start);
    let span = |start: usize| {
        let mut rest = name.iter().enumerate().skip(start);
        let mut last = start;
        for letter in &wanted {
            last = rest.find(|(_, one)| *one == letter)?.0;
        }
        Some(last + 1 - start - wanted.len())
    };
    starts.filter_map(span).min()
}

/// The entries `query` finds, as places in `entries`: grouped by heading in the headings'
/// order, the best match first in each group and at most [`MOST_PER_HEADING`] of them. Of two
/// matches as good, the shorter name goes first, then the one that comes first in `entries`.
pub fn ranked(query: &str, entries: &[Entry]) -> Vec<usize> {
    let mut found: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(place, entry)| {
            let score = score(query, &entry.name, &entry.words)?;
            Some((
                entry.heading,
                Reverse(score),
                entry.name.chars().count(),
                place,
            ))
        })
        .collect();
    found.sort_unstable();
    let mut listed = HashMap::new();
    let mut places = Vec::new();
    for (heading, _, _, place) in found {
        let count = listed.entry(heading).or_insert(0_usize);
        if *count < MOST_PER_HEADING {
            *count += 1;
            places.push(place);
        }
    }
    places
}
