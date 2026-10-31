//! Whose scans the rail shows: how many people an export holds, each one's studies under a
//! heading, and no patient's name while their details are hidden.

use super::tests::{Drawn, export, pass, series, top_of};
use super::*;
use crate::ui::seen;

#[test]
fn two_people_are_said_and_grouped() {
    let state = export(vec![
        (
            "MADE-UP-1",
            "Made^Up^One",
            vec![series(1, "MR", "made-up t1 ax", 20)],
        ),
        (
            "MADE-UP-2",
            "Made^Up^Two",
            vec![series(2, "MR", "made-up t2 ax", 20)],
        ),
    ]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let at = |text: &str| {
        let found = drawn.seen.words.iter().find(|(word, _)| word == text);
        let missing = || panic!("{text}: {:?}", drawn.seen.words);
        found.map_or_else(missing, |(_, rect)| rect.top())
    };
    let said = at("This export holds scans of 2 people.");
    let (first, second) = (at("Person 1"), at("Person 2"));
    let cards = [1, 2].map(|number| {
        let info = &state.index.as_ref().expect("open").series[number - 1].info;
        top_of(&drawn.seen, info).expect("drawn")
    });
    assert!(said < first && first < cards[0], "{said} {first} {cards:?}");
    assert!(cards[0] < second && second < cards[1], "{second} {cards:?}");
    // Each person's study is headed too, by its date and description.
    at("1 January 2020 · made-up study 1");
    at("2 January 2020 · made-up study 2");
}

#[test]
fn one_study_of_one_person_has_no_headings() {
    let state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![series(1, "MR", "t1 ax", 20)],
    )]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let words: Vec<&str> = drawn
        .seen
        .words
        .iter()
        .map(|(word, _)| word.as_str())
        .collect();
    assert!(
        !words
            .iter()
            .any(|word| word.contains("2020") || word.starts_with("Person")),
        "{words:?}"
    );
}

#[test]
fn the_patients_name_is_nowhere_in_the_rail_while_details_are_off() {
    let page = series(3, "SC", "made-up page", 1);
    let mut state = export(vec![
        (
            "MADE-UP-7",
            "Made^Up^Seven",
            vec![series(1, "MR", "t1 ax", 20), page],
        ),
        (
            "MADE-UP-8",
            "Made^Up^Eight",
            vec![series(2, "MR", "t2 ax", 20)],
        ),
    ]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let all_said = |drawn: &Drawn| {
        let words = drawn.seen.words.iter().map(|(word, _)| word.clone());
        let names = drawn.seen.names.iter().map(|(name, _)| name.clone());
        words.chain(names).collect::<Vec<_>>()
    };
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    for said in all_said(&drawn) {
        assert!(
            !said.contains("Made^Up") && !said.contains("MADE-UP"),
            "{said}"
        );
    }
    // Shown, the name and identifier head that person's studies.
    state.show_patient = true;
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let said = all_said(&drawn);
    assert!(
        said.iter().any(|said| said == "Made^Up^Seven · MADE-UP-7"),
        "{said:?}"
    );
}
