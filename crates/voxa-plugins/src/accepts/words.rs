//! The contrasts a series' description names, read exactly as the interface redesign's plain
//! names read them (`naming.rs` and `naming/words.rs` on its branch): the same words, the same
//! joining of `MP` and `RAGE`, and the same rule that a description naming two different ones of
//! T1, T2 and PD names none. One copy is kept when the two branches meet.

/// Weighting and sequence words, each with the plain name it is read as.
const WEIGHTINGS: &[(&str, &str)] = &[
    ("T1", "T1"),
    ("T1W", "T1"),
    ("T2", "T2"),
    ("T2W", "T2"),
    ("PD", "PD"),
    ("PDW", "PD"),
    ("FLAIR", "FLAIR"),
    ("T2*", "T2*"),
    ("DWI", "Diffusion"),
    ("TRACE", "Diffusion"),
    ("DIFFUSION", "Diffusion"),
    ("ADC", "ADC map"),
    ("SWI", "SWI"),
    ("SWAN", "SWI"),
    ("STIR", "STIR"),
    ("TOF", "Angiography (TOF)"),
    ("MPRAGE", "3D T1"),
    ("BRAVO", "3D T1"),
];

/// Pairs of words a scanner may write as two that are one word of the vocabulary joined.
const JOINED: &[(&str, &str)] = &[("MP", "RAGE")];

/// The three contrasts a weighting is made of, if it is one of them.
const CONTRASTS: [&str; 3] = ["T1", "T2", "PD"];

/// The entry of a table for a word.
fn lookup<T: Copy>(table: &[(&str, T)], word: &str) -> Option<T> {
    table
        .iter()
        .find(|(key, _)| *key == word)
        .map(|(_, value)| *value)
}

/// The words of a description, in capitals: letters and digits, and a `*` after them.
fn words_of(description: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for letter in description.chars() {
        if letter.is_alphanumeric() || (letter == '*' && !word.is_empty()) {
            word.push(letter.to_ascii_uppercase());
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    join_split(words)
}

/// The words with each pair the vocabulary reads as one joined: `MP` then `RAGE`.
fn join_split(words: Vec<String>) -> Vec<String> {
    let mut joined = Vec::with_capacity(words.len());
    let mut rest = words.into_iter().peekable();
    while let Some(word) = rest.next() {
        match lookup(JOINED, &word) {
            Some(second) if rest.peek().is_some_and(|next| next == second) => {
                rest.next();
                joined.push(format!("{word}{second}"));
            }
            _ => joined.push(word),
        }
    }
    joined
}

/// The one of T1, T2 and PD a weighting is made of, if it is.
fn contrast_of(weighting: &str) -> Option<&'static str> {
    CONTRASTS
        .into_iter()
        .find(|contrast| weighting.split(' ').any(|word| word == *contrast))
}

/// The plain names of the weightings a description names, each once, in the order met: none
/// when it names two different ones of T1, T2 and PD; and a name another holds at its end (T1
/// in 3D T1) left out.
pub(super) fn weightings_in(description: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for weighting in words_of(description)
        .iter()
        .filter_map(|word| lookup(WEIGHTINGS, word))
    {
        if !found.contains(&weighting) {
            found.push(weighting);
        }
    }
    let mut contrasts: Vec<&str> = found
        .iter()
        .filter_map(|weighting| contrast_of(weighting))
        .collect();
    contrasts.sort_unstable();
    contrasts.dedup();
    if contrasts.len() > 1 {
        return Vec::new();
    }
    let all = found.clone();
    found.retain(|weighting| {
        !all.iter().any(|other| {
            other
                .strip_suffix(*weighting)
                .is_some_and(|front| front.ends_with(' '))
        })
    });
    found
}
