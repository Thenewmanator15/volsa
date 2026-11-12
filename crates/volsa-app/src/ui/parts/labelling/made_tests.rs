//! What the Labels tab says of labels that Volsa made: who made them, with what, where and when,
//! what the model's own check said, and the notes beneath.

use volsa_labels::Verdict;

use super::made::made_in_words;
use super::tests::{at, by_volsa, lines_of, made_by_volsa};
use super::*;
use crate::labels::NamesFrom;
use crate::ui::commands::Request;
use crate::ui::parts::{Asked, MADE_ELSEWHERE};
use crate::ui::tabs::made_up::{labelled_by, labels_written as written, sweep_labels as swept};

#[test]
fn labels_made_by_volsa_say_with_what_where_and_when_and_each_note_on_its_own_line() {
    let state = made_by_volsa(None);
    let said = written(&state);
    let lines = lines_of(&said);
    let made = "Labels made by Volsa with Made-up model, robust, on Made-up card A, 2099-01-02. \
                Not for diagnostic use.";
    assert!(lines.contains(&made), "{said}");
    assert!(!said.contains(MADE_ELSEWHERE), "in place of that: {said}");
    let caption = at(&lines, |line| line == made);
    let first = at(&lines, |line| line == "A first made-up note.");
    let second = at(&lines, |line| line == "A second made-up note.");
    assert!(caption < first && first < second, "beneath it: {said}");
    let (_, requests) = swept(&state);
    let save = Request::Choose(Asked::Labelling(Wants::SaveLabels));
    assert!(requests.contains(&save), "{requests:?}");
    assert!(lines.contains(&"Save as…"), "{said}");
}

#[test]
fn labels_from_a_run_say_their_names_are_the_models_own_and_not_a_tables_beside_them() {
    let state = labelled_by(None, NamesFrom::Model, Some(by_volsa()));
    let said = written(&state);
    let lines = lines_of(&said);
    assert!(
        lines.contains(&"Names from the model's own list."),
        "{said}"
    );
    assert!(!said.contains("Names from the table"), "{said}");
}

#[test]
fn labels_made_by_volsa_with_parts_left_unwritten_say_only_what_is_known() {
    let mut bare = by_volsa();
    bare.model.clear();
    bare.device.clear();
    bare.when.clear();
    assert_eq!(
        made_in_words(&bare),
        "Labels made by Volsa. Not for diagnostic use."
    );
    let mut no_model = by_volsa();
    no_model.model.clear();
    assert_eq!(
        made_in_words(&no_model),
        "Labels made by Volsa, on Made-up card A, 2099-01-02. Not for diagnostic use."
    );
    assert_eq!(
        solids_in_words(&by_volsa()),
        "Solids are labels made by Volsa with Made-up model, robust, on Made-up card A, \
         2099-01-02. Not for diagnostic use."
    );
}

#[test]
fn a_check_that_failed_is_said_before_who_made_the_labels() {
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("general csf".to_owned(), 0.6)],
    };
    let said = written(&made_by_volsa(Some(verdict.clone())));
    let lines = lines_of(&said);
    let failed = at(&lines, |line| line == verdict_in_plain_words(&verdict));
    let caption = at(&lines, |line| line.starts_with("Labels made by Volsa"));
    assert!(failed < caption, "{said}");
    // Labels from a file keep the order of 0.4.0: the check after where the names came from.
    let from_a_file = labelled_by(Some(verdict.clone()), NamesFrom::Table, None);
    let said = written(&from_a_file);
    let lines = lines_of(&said);
    let failed = at(&lines, |line| line == verdict_in_plain_words(&verdict));
    let named = at(&lines, |line| line.starts_with("Names from"));
    assert!(named < failed, "{said}");
}

/// A plugin the person added, as labels it made say it.
fn added() -> volsa_labels::Added {
    volsa_labels::Added {
        id: "example.strip".to_owned(),
        name: "Made-up strip".to_owned(),
        version: "1.2".to_owned(),
        made_by: "A made-up lab".to_owned(),
        source: "https://example.org/made-up-model".to_owned(),
        licence: "MIT".to_owned(),
        limits: vec!["Made for adults.".to_owned()],
        sum: String::new(),
    }
}

#[test]
fn labels_a_plugin_made_say_it_was_added_by_the_person_then_where_and_when_it_ran() {
    let mut made = by_volsa();
    made.added = Some(added());
    let banner = volsa_labels::added_in_words(&added());
    let said = made_in_words(&made);
    assert_eq!(said, format!("{banner} Run on Made-up card A, 2099-01-02."));
    assert!(!said.contains("made by Volsa"), "{said}");
    let mut state = made_by_volsa(None);
    if let Some(open) = state.labels.made.as_mut() {
        open.added = Some(added());
    }
    let written = written(&state);
    let lines = lines_of(&written);
    let first = at(&lines, |line| line == said);
    let note = at(&lines, |line| line == "A first made-up note.");
    assert!(first < note, "the banner, then the notes: {written}");
}

#[test]
fn the_models_check_is_said_in_plain_words_with_no_score_on_the_screen() {
    let low = vec![("general csf".to_owned(), 0.6), ("shell".to_owned(), 0.52)];
    let failed = Verdict {
        passed: false,
        fail_below: 0.65,
        low,
    };
    let passed = Verdict {
        passed: true,
        fail_below: 0.65,
        low: Vec::new(),
    };
    for verdict in [&failed, &passed] {
        let said = verdict_in_plain_words(verdict);
        assert!(
            !said.chars().any(|letter| letter.is_ascii_digit()),
            "{said}"
        );
    }
    let said = verdict_in_plain_words(&failed);
    assert!(
        said.contains("did not pass for general csf, shell"),
        "what it did not pass for, by name: {said}"
    );
    assert!(said.contains("Take every part here with caution"), "{said}");
    let said = verdict_in_plain_words(&passed);
    assert!(said.contains("passed for every part"), "{said}");
    assert!(
        said.contains("the model's opinion of its own work"),
        "{said}"
    );
    // The figures are the model's, and are in the hover text, not in the line.
    let figures = volsa_labels::verdict_in_words(&failed);
    assert!(
        figures.contains("0.52") && figures.contains("0.65"),
        "{figures}"
    );
    // And on the screen of the Labels tab, none of them is written.
    let drawn = written(&made_by_volsa(Some(failed)));
    assert!(!drawn.contains("0.52") && !drawn.contains("0.6"), "{drawn}");
    assert!(
        drawn.contains("did not pass for general csf, shell"),
        "{drawn}"
    );
}
