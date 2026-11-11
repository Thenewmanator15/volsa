//! A plugin's entry and card, in the words the Tools tab shows.

use volsa_models::SUPPORTED;
use volsa_plugins::testkit::example_folder;
use volsa_plugins::{Output, Stage, check_folder};

use super::{OfferCard, PluginEntry, Plugins, Running, settings_lines, tool_entries};
use crate::testing::id;

/// The example plugin, offered.
fn offered() -> volsa_plugins::Offered {
    let folder = example_folder("1.0");
    check_folder(folder.path(), SUPPORTED).expect("offered")
}

#[test]
fn settings_lines_say_the_examples_words() {
    let offered = offered();
    let Output::Labels(output) = &offered.description.output else {
        panic!("labels");
    };
    let lines = settings_lines(&offered.description.input, output);
    let expected = [
        "Resampled to 1 × 1 × 1 mm.",
        "Turned towards the left, back, top.",
        "Fitted to 8 × 8 × 8 voxels, centred on the scan.",
        "Outside the scan: the scan's lowest value.",
        "Brightness: as stored; the model scales it itself.",
        "Answer: the label of the highest of 2 scores.",
        "Every labelled piece is kept.",
    ];
    assert_eq!(lines, expected);
}

#[test]
fn an_entry_says_what_the_plugin_accepts_and_gives() {
    let entry = PluginEntry::of_offered(&offered(), "the processor");
    assert_eq!(entry.accepts_words, "MR scans, T1, of the head");
    assert_eq!(entry.returns, "Labels: 1 named part.");
    assert_eq!(entry.source, "https://example.org/skull-strip");
}

#[test]
fn a_card_names_the_version_it_replaces() {
    let offered = offered();
    assert_eq!(OfferCard::of(&offered, &[]).replaces, None);
    let older = PluginEntry {
        version: "0.9".to_owned(),
        ..PluginEntry::of_offered(&offered, "the processor")
    };
    assert_eq!(
        OfferCard::of(&offered, &[older]).replaces.as_deref(),
        Some("0.9")
    );
}

#[test]
fn tool_entries_carry_every_field_the_tools_tab_draws() {
    let entry = PluginEntry::of_offered(&offered(), "the processor");
    let broken = PluginEntry::broken("0123456789abcdef", "It changed.".to_owned());
    let plugins = Plugins {
        entries: vec![entry, broken],
        running: Some(Running {
            id: "example.skull-strip".to_owned(),
            run: 1,
            stage: Stage::Running { block: 2, of: 5 },
            series: id("1.2.3"),
        }),
        ..Plugins::default()
    };
    let tools = tool_entries(&plugins);
    assert_eq!(tools[0].state, "Running block 2 of 5.");
    assert!(tools[0].running);
    assert_eq!(tools[0].run, "plugin.example.skull-strip.run");
    assert_eq!(tools[0].remove, "plugin.example.skull-strip.remove");
    assert_eq!(tools[0].entry.made_by, "An example lab");
    assert_eq!(tools[0].entry.licence, "MIT");
    assert_eq!(tools[1].state, "It changed.");
    assert!(!tools[1].running);
}
