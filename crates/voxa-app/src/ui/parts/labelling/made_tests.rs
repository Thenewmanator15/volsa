//! What the Labels tab says of labels that Voxa made: who made them, with what, where and when,
//! what the model's own check said, and the notes beneath.

use voxa_labels::{Verdict, verdict_in_words};

use super::made::made_in_words;
use super::tests::{at, by_voxa, lines_of, made_by_voxa};
use super::*;
use crate::labels::NamesFrom;
use crate::ui::commands::Request;
use crate::ui::parts::{Asked, MADE_ELSEWHERE};
use crate::ui::tabs::made_up::{labelled_by, labels_written as written, sweep_labels as swept};

#[test]
fn labels_made_by_voxa_say_with_what_where_and_when_and_each_note_on_its_own_line() {
    let state = made_by_voxa(None);
    let said = written(&state);
    let lines = lines_of(&said);
    let made = "Labels made by Voxa with Made-up model, robust, on Made-up card A, 2099-01-02. \
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
    let state = labelled_by(None, NamesFrom::Model, Some(by_voxa()));
    let said = written(&state);
    let lines = lines_of(&said);
    assert!(
        lines.contains(&"Names from the model's own list."),
        "{said}"
    );
    assert!(!said.contains("Names from the table"), "{said}");
}

#[test]
fn labels_made_by_voxa_with_parts_left_unwritten_say_only_what_is_known() {
    let mut bare = by_voxa();
    bare.model.clear();
    bare.device.clear();
    bare.when.clear();
    assert_eq!(
        made_in_words(&bare),
        "Labels made by Voxa. Not for diagnostic use."
    );
    let mut no_model = by_voxa();
    no_model.model.clear();
    assert_eq!(
        made_in_words(&no_model),
        "Labels made by Voxa, on Made-up card A, 2099-01-02. Not for diagnostic use."
    );
    assert_eq!(
        solids_in_words(&by_voxa()),
        "Solids are labels made by Voxa with Made-up model, robust, on Made-up card A, \
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
    let said = written(&made_by_voxa(Some(verdict.clone())));
    let lines = lines_of(&said);
    let failed = at(&lines, |line| line == verdict_in_words(&verdict));
    let caption = at(&lines, |line| line.starts_with("Labels made by Voxa"));
    assert!(failed < caption, "{said}");
    // Labels from a file keep the order of 0.4.0: the check after where the names came from.
    let from_a_file = labelled_by(Some(verdict.clone()), NamesFrom::Table, None);
    let said = written(&from_a_file);
    let lines = lines_of(&said);
    let failed = at(&lines, |line| line == verdict_in_words(&verdict));
    let named = at(&lines, |line| line.starts_with("Names from"));
    assert!(named < failed, "{said}");
}
