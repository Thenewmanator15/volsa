//! What a reader would refuse or cut short is not written, and words cut at a space are kept
//! the same however often they are written and read.

use proptest::prelude::*;

use super::table_tests::{entry, head, made};
use super::{WriteError, write_table};
use crate::table::{Entry, Side, Table, Trust, colour_for, kept, read_table};

/// A table of names with no group, three to a name: left, right and midline. Nothing in it
/// is written but the names and their numbers.
fn named(count: u32) -> Table {
    let sides = [Side::Left, Side::Right, Side::Midline];
    let entries = (0..count)
        .zip(sides.into_iter().cycle())
        .map(|(at, side)| Entry {
            name: (at / 3).to_string(),
            group: String::new(),
            side,
            ids: vec![at + 1],
            colour: colour_for(at + 1),
            trust: Trust::NotSaid,
        });
    Table {
        title: String::new(),
        entries: entries.collect(),
        verdict: None,
        made: None,
    }
}

#[test]
fn a_table_that_names_nothing_is_refused() {
    let nothing = Table {
        entries: Vec::new(),
        ..head()
    };
    assert_eq!(write_table(&nothing), Err(WriteError::NoNames));
    // An entry with no number names nothing either.
    let numberless = Entry {
        ids: Vec::new(),
        ..entry("none", Side::Left, 1, Trust::Model)
    };
    let table = Table {
        entries: vec![numberless.clone(), numberless],
        ..head()
    };
    assert_eq!(write_table(&table), Err(WriteError::NoNames));
    let said = WriteError::NoNames.to_string();
    assert!(said.contains("no part"), "{said}");
}

#[test]
fn a_table_of_more_names_than_a_reader_takes_is_refused_and_one_of_as_many_is_not() {
    let most = named(65_535);
    let text = write_table(&most).expect("writes");
    assert!(
        text.len() < 4 << 20,
        "the number of names is what is too many"
    );
    assert_eq!(read_table("a.json", &text).expect("reads"), most);
    assert_eq!(write_table(&named(65_536)), Err(WriteError::TooLarge));
    let said = WriteError::TooLarge.to_string();
    assert!(said.contains("too many names"), "{said}");
}

/// A name whose 80th character is a space, with more after it.
fn spaced(letter: &str) -> String {
    format!("{}  {}", letter.repeat(79), "z".repeat(10))
}

#[test]
fn words_cut_at_a_space_are_kept_the_same_when_written_and_read_again() {
    let table = Table {
        title: spaced("t"),
        entries: vec![Entry {
            group: spaced("g"),
            trust: Trust::Approximate(format!("{}  tail", "h".repeat(298))),
            ..entry(&spaced("a"), Side::Left, 5, Trust::NotSaid)
        }],
        verdict: None,
        made: None,
    };
    let text = write_table(&table).expect("writes");
    let once = read_table("a.json", &text).expect("reads");
    assert_eq!(once.title, "t".repeat(79));
    assert_eq!(once.entries[0].name, "a".repeat(79));
    assert_eq!(once.entries[0].group, "g".repeat(79));
    assert_eq!(once.entries[0].trust, Trust::Approximate("h".repeat(298)));
    // Written and read again, nothing changes: not the table, and not the text.
    assert_eq!(write_table(&once).expect("writes"), text);
    assert_eq!(read_table("a.json", &text).expect("reads"), once);
}

proptest! {
    #[test]
    fn words_kept_are_kept_again_unchanged_and_have_no_space_at_either_end(
        words in "[a-z \t]{0,200}",
        most in 1_usize..100,
    ) {
        let once = kept(&words, most);
        prop_assert_eq!(kept(&once, most), once.clone());
        prop_assert_eq!(once.trim(), once.as_str());
        prop_assert!(once.chars().count() <= most);
    }
}

#[test]
fn two_names_alike_up_to_the_cut_are_one_name() {
    let near = |tail: &str| {
        entry(
            &format!("{} {tail}", "a".repeat(79)),
            Side::Left,
            1,
            Trust::Model,
        )
    };
    let table = Table {
        entries: vec![near("x"), near("y")],
        made: Some(made()),
        ..head()
    };
    assert_eq!(write_table(&table), Err(WriteError::NameTwice));
}
