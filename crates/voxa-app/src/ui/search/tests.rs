//! What search finds and in what order: the matching, and the entries it is made from.

use super::entries::{Entry, Heading, Target, entries_for};
use super::matching::{fold, ranked, score};
use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::testing::{labelled_head, thick_scan_ready};
use crate::ui::commands;
use crate::ui::help::TOPICS;

/// The id of the action the entry is for, if it is one.
fn command_of(entry: &Entry) -> Option<&'static str> {
    match entry.target {
        Target::Command(id) => Some(id),
        _ => None,
    }
}

/// How well `query` finds `entry`.
fn score_of(query: &str, entry: &Entry) -> Option<u32> {
    score(query, &entry.name, &entry.words)
}

/// An entry that is found by nothing but its name.
fn named(heading: Heading, name: &str, place: usize) -> Entry {
    Entry {
        heading,
        name: name.to_owned(),
        words: Vec::new(),
        target: Target::Part(place),
    }
}

#[test]
fn every_command_is_found_by_its_name_and_each_of_its_words() {
    let entries = entries_for(&AppState::default());
    for command in commands::all() {
        for query in std::iter::once(command.name).chain(command.words.iter().copied()) {
            let top = ranked(query, &entries);
            let among_first_eight = top
                .iter()
                .take(8)
                .any(|&place| command_of(&entries[place]) == Some(command.id));
            assert!(among_first_eight, "{query:?} does not find {}", command.id);
        }
    }
}

#[test]
fn a_command_is_first_for_its_own_name() {
    let entries = entries_for(&AppState::default());
    for command in commands::all() {
        let top = ranked(command.name, &entries);
        let first = top.first().map(|&place| &entries[place]);
        let said_so = first.and_then(command_of) == Some(command.id);
        assert!(said_so, "{} is first for its own name", command.id);
    }
}

#[test]
fn starts_of_words_beat_scattered_letters() {
    let length = Entry {
        words: vec!["ruler".to_owned()],
        ..named(Heading::Actions, "Measure a length", 0)
    };
    let scattered = named(Heading::Actions, "Mark the surface line", 1);
    let (near, far) = (score_of("rul", &length), score_of("rul", &scattered));
    assert!(far.is_some(), "r, u and l are in it, in order");
    assert!(near > far, "{near:?} against {far:?}");
    let found = [scattered, length];
    assert_eq!(ranked("rul", &found), [1, 0]);
    // And in the list of actions: the ruler is first.
    let entries = entries_for(&AppState::default());
    let top = ranked("rul", &entries);
    assert_eq!(command_of(&entries[top[0]]), Some("measure.ruler"));
}

#[test]
fn case_and_accents_are_ignored() {
    assert_eq!(fold("Éé Ü"), "ee u");
    assert_eq!(fold("Ångström Straße"), "angstrom strasse");
    assert_eq!(fold("Cre\u{301}me"), "creme", "an accent written apart");
    let scan = named(Heading::Scans, "Café scan, T2", 0);
    for query in ["CAFE", "café", "cafÉ", "t2"] {
        assert!(score_of(query, &scan).is_some(), "{query}");
    }
}

#[test]
fn the_kinds_of_match_rank_in_the_order_the_design_gives() {
    let words = |words: &[&str]| {
        words
            .iter()
            .map(|word| (*word).to_owned())
            .collect::<Vec<_>>()
    };
    let name = "Tags of the file";
    let scores = [
        score("tags of the file", name, &[]),
        score("tag", name, &[]),
        score("fil", name, &[]),
        score("header", name, &words(&["metadata", "header"])),
        score("tgs", name, &[]),
    ];
    let scores: Vec<u32> = scores.into_iter().map(|one| one.expect("found")).collect();
    let mut best_first = scores.clone();
    best_first.sort_by(|a, b| b.cmp(a));
    assert_eq!(
        scores, best_first,
        "whole name, start, word, other word, scattered"
    );
    let distinct: std::collections::HashSet<_> = scores.iter().collect();
    assert_eq!(
        distinct.len(),
        scores.len(),
        "each kind is a step of its own"
    );
    assert_eq!(score("zzz", name, &[]), None);
}

#[test]
fn every_word_of_the_query_must_match_and_the_weakest_sets_the_score() {
    let name = "Open a folder of DICOM files";
    let both = score("open fol", name, &[]).expect("both words match");
    let word = score("fol", name, &[]).expect("a start of a word");
    assert_eq!(both, word, "the weakest word's score: a start of a word");
    assert_eq!(score("open nothing", name, &[]), None);
    assert!(score("folder open", name, &[]).is_some(), "in any order");
    let start = score("open a fol", name, &[]).expect("the name starts with it");
    assert!(start > word, "the whole query starts the name");
}

#[test]
fn the_fewer_letters_skipped_the_higher_the_score() {
    let tight = score("mgn", "Magnify", &[]).expect("found");
    let loose = score("mgn", "Make a great new thing", &[]).expect("found");
    assert!(tight > loose, "{tight} against {loose}");
}

#[test]
fn ties_go_to_the_shorter_name_then_the_table_order() {
    let entries = [
        named(Heading::Actions, "Open the long one", 0),
        named(Heading::Actions, "Open two", 1),
        named(Heading::Actions, "Open six", 2),
        named(Heading::Actions, "Open", 3),
    ];
    // "Open" is the whole name of the last; the others start with it, the shortest first,
    // and two of the same length keep the order they are listed in.
    assert_eq!(ranked("open", &entries), [3, 1, 2, 0]);
}

#[test]
fn at_most_eight_of_each_heading_are_listed_in_the_headings_order() {
    let mut entries = Vec::new();
    for heading in [
        Heading::Help,
        Heading::Scans,
        Heading::Parts,
        Heading::Actions,
    ] {
        for place in 0..12 {
            entries.push(named(heading, &format!("Thing {place}"), place));
        }
    }
    let listed = ranked("thing", &entries);
    let headings: Vec<Heading> = listed.iter().map(|&place| entries[place].heading).collect();
    assert_eq!(headings.len(), 32);
    let mut in_order = headings.clone();
    in_order.sort();
    assert_eq!(headings, in_order, "Actions, Parts, Scans, Help");
    for heading in [
        Heading::Actions,
        Heading::Parts,
        Heading::Scans,
        Heading::Help,
    ] {
        assert_eq!(headings.iter().filter(|one| **one == heading).count(), 8);
    }
}

#[test]
fn an_empty_query_finds_nothing() {
    let entries = entries_for(&AppState::default());
    for query in ["", "  ", "%"] {
        assert_eq!(ranked(query, &entries), Vec::<usize>::new(), "{query:?}");
    }
}

#[test]
fn parts_and_scans_are_found() {
    let (state, _) = labelled_head();
    let entries = entries_for(&state);
    let listed = ranked("left", &entries);
    let parts: Vec<&Target> = listed
        .iter()
        .map(|&place| &entries[place])
        .filter(|entry| entry.heading == Heading::Parts)
        .map(|entry| &entry.target)
        .collect();
    // The two parts are both called "half"; the second is on the left.
    assert_eq!(parts, [&Target::Part(1)]);
    let by_group = ranked("halves", &entries);
    assert!(
        by_group
            .iter()
            .any(|&place| entries[place].target == Target::Part(0))
    );
    // Every scan of the export is found by its plain name.
    let index = state.index.as_ref().expect("an export is open");
    assert_eq!(index.series.len(), 3);
    for series in &index.series {
        let name = crate::naming::plain_name(&series.info);
        let found = ranked(&name, &entries);
        let scan = Target::Scan(series.info.id.clone());
        assert!(
            found.iter().any(|&place| entries[place].target == scan),
            "{name}"
        );
    }
}

#[test]
fn parts_are_listed_only_while_labels_are_open() {
    let (mut state, _) = labelled_head();
    let parts = |state: &AppState| {
        let entries = entries_for(state);
        entries
            .iter()
            .filter(|entry| entry.heading == Heading::Parts)
            .count()
    };
    assert_eq!(parts(&state), 2);
    reduce(&mut state, Action::CloseLabels);
    assert_eq!(parts(&state), 0);
    assert_eq!(parts(&AppState::default()), 0);
}

#[test]
fn there_are_seven_topics_each_found_by_search() {
    let entries = entries_for(&thick_scan_ready());
    let help: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.heading == Heading::Help)
        .collect();
    let ids: Vec<&Target> = help.iter().map(|entry| &entry.target).collect();
    let wanted = [
        "open",
        "find",
        "brightness",
        "measure",
        "sides",
        "labels",
        "not-for",
    ];
    let wanted: Vec<Target> = wanted.into_iter().map(Target::Help).collect();
    assert_eq!(ids, wanted.iter().collect::<Vec<_>>());
    // Each is found by its title, in first place among the help, and by each of its words.
    for topic in &TOPICS {
        let target = Target::Help(topic.id);
        let place = entries.iter().position(|entry| entry.target == target);
        let place = place.expect("listed");
        let found_by = |query: &str| ranked(query, &entries).contains(&place);
        assert!(!topic.title.is_empty(), "{}", topic.id);
        assert!(
            found_by(topic.title),
            "{} is not found by its title",
            topic.id
        );
        let top = ranked(topic.title, &entries);
        let first_help = top
            .iter()
            .find(|&&found| entries[found].heading == Heading::Help);
        assert_eq!(first_help, Some(&place), "{} is not first", topic.id);
        for word in topic.words {
            assert!(found_by(word), "{} is not found by {word:?}", topic.id);
        }
    }
}
