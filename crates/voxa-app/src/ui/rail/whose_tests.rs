//! Whose scans the rail shows: how many people an export holds, each one's studies under a
//! heading, the newest first, and no patient's name while their details are hidden.

use super::tests::{Drawn, export, pass, series, top_of};
use super::*;
use crate::ui::seen;

/// Where the first piece of text that is `text` was drawn, from the top.
fn top_of_words(drawn: &Drawn, text: &str) -> f32 {
    let found = drawn.seen.words.iter().find(|(word, _)| word == text);
    let missing = || panic!("{text}: {:?}", drawn.seen.words);
    found.map_or_else(missing, |(_, rect)| rect.top())
}

/// The state with the studies of its export given these dates, in the index's order.
fn dated(mut state: AppState, dates: &[Option<&str>]) -> AppState {
    let studies = state.index.as_mut().map(|index| index.studies.iter_mut());
    for (study, date) in studies.into_iter().flatten().zip(dates) {
        study.date = date.map(str::to_owned);
    }
    state
}

#[test]
fn two_people_are_said_and_grouped() {
    // The first person's study is the earlier: the second person, of the newer, comes first.
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
    let at = |text: &str| top_of_words(&drawn, text);
    let said = at("This export holds scans of 2 people.");
    let (first, second) = (at("Person 1"), at("Person 2"));
    let card = |number: usize| {
        let info = &state.index.as_ref().expect("open").series[number - 1].info;
        top_of(&drawn.seen, info).expect("drawn")
    };
    let (newer, earlier) = (card(2), card(1));
    assert!(said < first && first < newer, "{said} {first} {newer}");
    assert!(
        newer < second && second < earlier,
        "{second} {newer} {earlier}"
    );
    // Each person's study is headed too, by its date and description.
    let headed = [
        at("2 January 2020 · made-up study 2"),
        at("1 January 2020 · made-up study 1"),
    ];
    assert!(
        first < headed[0] && headed[0] < newer && headed[1] < earlier,
        "{headed:?}"
    );
}

#[test]
fn studies_are_listed_newest_first_and_the_undated_last() {
    // Each of a number of slices of its own, so that its card has a name of its own.
    let one = |number: u8| {
        series(
            i32::from(number),
            "MR",
            "made-up t1",
            20 + usize::from(number),
        )
    };
    let study = |number| ("MADE-UP-1", "Made^Up", vec![one(number)]);
    let state = export(vec![study(1), study(2), study(3), study(4)]);
    let dates = [Some("20190412"), None, Some("20210830"), Some("20200102")];
    let state = dated(state, &dates);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let at = |text: &str| top_of_words(&drawn, text);
    let tops = [
        at("30 August 2021 · made-up study 3"),
        at("2 January 2020 · made-up study 4"),
        at("12 April 2019 · made-up study 1"),
        at("Undated study · made-up study 2"),
    ];
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    // Each card is under its own study's heading.
    let infos = &state.index.as_ref().expect("open").series;
    let card = |number: usize| top_of(&drawn.seen, &infos[number - 1].info).expect("drawn");
    assert!(
        tops[0] < card(3) && card(3) < tops[1],
        "{tops:?} {}",
        card(3)
    );
    assert!(tops[3] < card(2), "{tops:?} {}", card(2));
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
