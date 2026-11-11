//! A label set made by a plugin the person added says so, through a table written and read
//! back, and in the words shown first wherever the labels are.

use crate::{Added, Entry, Made, Side, Table, Trust, added_in_words, read_table, write_table};

/// A made-up plugin.
fn added() -> Added {
    Added {
        id: "example.skull-strip".to_owned(),
        name: "Example skull strip".to_owned(),
        version: "1.0".to_owned(),
        made_by: "An example lab".to_owned(),
        source: "https://example.org/skull-strip".to_owned(),
        licence: "MIT".to_owned(),
        limits: vec!["Trained on adult heads only.".to_owned()],
        sum: "0123456789abcdef".repeat(4),
    }
}

/// A table of one part, made by the plugin.
fn table(added: Option<Added>) -> Table {
    Table {
        title: "Example skull strip".to_owned(),
        entries: vec![Entry {
            name: "Brain".to_owned(),
            group: "Brain".to_owned(),
            side: Side::Midline,
            ids: vec![1],
            colour: [200, 100, 50],
            trust: Trust::Model,
        }],
        verdict: None,
        made: Some(Made {
            by: "Volsa 9.8.7".to_owned(),
            model: "Example skull strip 1.0".to_owned(),
            notes: vec!["Contrast not confirmed: the scan's description does not say.".to_owned()],
            added,
            ..Made::default()
        }),
    }
}

#[test]
fn added_by_you_round_trips_through_a_written_table() {
    let text = write_table(&table(Some(added()))).expect("written");
    assert!(text.contains("added_by_you"));
    let read = read_table("labels.json", &text).expect("read");
    let made = read.made.expect("who made it");
    assert_eq!(made.added, Some(added()));
    assert_eq!(
        made.notes,
        ["Contrast not confirmed: the scan's description does not say."]
    );
}

#[test]
fn a_table_without_it_is_not_added() {
    let text = write_table(&table(None)).expect("written");
    assert!(!text.contains("added_by_you"));
    let read = read_table("labels.json", &text).expect("read");
    assert_eq!(read.made.expect("who made it").added, None);
}

#[test]
fn a_damaged_added_by_you_never_makes_a_table_unreadable() {
    let text = write_table(&table(Some(added()))).expect("written");
    for broken in ["\"added_by_you\": 7", "\"added_by_you\": [1, 2]"] {
        let mut value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        let replacement: serde_json::Value =
            serde_json::from_str(&format!("{{{broken}}}")).expect("JSON");
        value["volsa"]["added_by_you"] = replacement["added_by_you"].clone();
        let read = read_table("labels.json", &value.to_string()).expect("still a table");
        assert_eq!(read.made.expect("who made it").added, None);
    }
}

#[test]
fn text_too_long_is_cut_as_a_reader_cuts_it() {
    let long = Added {
        name: "n".repeat(500),
        limits: vec!["l".repeat(500); 12],
        sum: "not a sum".to_owned(),
        ..added()
    };
    let text = write_table(&table(Some(long))).expect("written");
    let read = read_table("labels.json", &text).expect("read");
    let added = read.made.expect("who made it").added.expect("added");
    assert_eq!(added.name.chars().count(), 60);
    assert_eq!(added.limits.len(), 8);
    assert!(
        added
            .limits
            .iter()
            .all(|limit| limit.chars().count() == 300)
    );
    assert_eq!(added.sum, "");
}

#[test]
fn the_banner_is_word_for_word() {
    let words = "Added by you, not part of Volsa. Example skull strip 1.0, made by An example lab. \
                 Source: https://example.org/skull-strip. Licence: MIT. Trained on adult heads \
                 only. Not for diagnostic use.";
    assert_eq!(added_in_words(&added()), words);
    let bare = Added {
        limits: Vec::new(),
        licence: "MIT.".to_owned(),
        ..added()
    };
    let words = "Added by you, not part of Volsa. Example skull strip 1.0, made by An example lab. \
                 Source: https://example.org/skull-strip. Licence: MIT. Not for diagnostic use.";
    assert_eq!(added_in_words(&bare), words);
}
