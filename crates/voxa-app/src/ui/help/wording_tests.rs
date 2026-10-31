//! The wording rule: nothing Voxa says suggests a finding, so that no label or measurement
//! reads as a verdict on a person's body. The topics are held to it, to the facts of the
//! window they describe, and to the risk register.

use super::topics::Topic;
use super::wording_sources::{other_sentences, stages, states};
use super::{MOUSE, TOPICS};
use crate::measuring;
use crate::ui::theme::DISCLAIMER;
use crate::ui::{commands, parts, tabs};

/// The words no text may hold, whole and whatever their case: a verdict, a measure of one, or
/// a name for what one finds.
const BANNED: [&str; 13] = [
    "normal",
    "abnormal",
    "lesion",
    "lesions",
    "tumour",
    "tumours",
    "tumor",
    "tumors",
    "finding",
    "findings",
    "score",
    "scores",
    "diagnosis",
];

/// The word that is held only in the disclaimer's sentence or in `not for diagnostic use`.
const DIAGNOSTIC: &str = "diagnostic";

/// The banned words `text` holds, each as written in lower case.
fn suggestions_in(text: &str) -> Vec<String> {
    let mut lower = text.to_lowercase();
    // The two places where "diagnostic" belongs are taken out before the words are read.
    for allowed in [DISCLAIMER, "not for diagnostic use"] {
        lower = lower.replace(&allowed.to_lowercase(), " ");
    }
    let words = lower.split(|letter: char| !letter.is_alphanumeric());
    let banned = words.filter(|word| BANNED.contains(word) || *word == DIAGNOSTIC);
    banned.map(str::to_owned).collect()
}

#[test]
fn the_check_catches_each_banned_word_and_lets_the_disclaimer_through() {
    for word in BANNED {
        for text in [
            format!("A {word} here"),
            format!("{}.", word.to_uppercase()),
            format!("not-{word}"),
        ] {
            assert_eq!(suggestions_in(&text), [word], "{text}");
        }
    }
    // Only the whole word counts: "normals" and "scored" are other words.
    assert_eq!(
        suggestions_in("The scored, abnormally normalised rescoring"),
        Vec::<String>::new()
    );
    assert_eq!(suggestions_in(DISCLAIMER), Vec::<String>::new());
    assert_eq!(
        suggestions_in("It is NOT FOR DIAGNOSTIC USE."),
        Vec::<String>::new()
    );
    assert_eq!(suggestions_in("A diagnostic tool"), ["diagnostic"]);
    assert_eq!(
        suggestions_in("Not for diagnostic purposes"),
        ["diagnostic"]
    );
}

/// Every text the window says that is written once and kept, with where it is from: each
/// action's name, label and hint, the reason it cannot be used in each of a few states, every
/// topic's title and body, the fixed sentences of the tabs and of the labelling and the
/// measuring, and the mouse's rows. The words search finds a topic by are not here: they are
/// never drawn, and a person who types "diagnosis" should reach "What Voxa is not for".
fn texts() -> Vec<(String, String)> {
    let mut texts = Vec::new();
    let mut add = |from: &str, text: &str| texts.push((from.to_owned(), text.to_owned()));
    let states = states();
    for command in commands::all() {
        let from = |what: &str| format!("the {what} of {}", command.id);
        add(&from("name"), command.name);
        add(&from("label"), command.label);
        add(&from("hint"), command.explains);
        for state in &states {
            if let Some(why) = (command.why_not)(state) {
                add(&from("reason it cannot be used"), &why);
            }
        }
    }
    for topic in &TOPICS {
        add(&format!("the title of {}", topic.id), topic.title);
        add(&format!("the body of {}", topic.id), topic.body);
    }
    for (input, does) in MOUSE {
        add("the mouse's rows", input);
        add("the mouse's rows", does);
    }
    for text in tabs::fixed_texts() {
        add("a tab", text);
    }
    for text in other_sentences() {
        add("a fixed sentence", text);
    }
    for stage in stages() {
        add(
            "a stage of labelling",
            &parts::labelling::stage_in_words(stage),
        );
    }
    // The model's own check, in the plain words the Labels tab says it in.
    let low = vec![("shell".to_owned(), 0.52), ("general csf".to_owned(), 0.6)];
    for (passed, low) in [
        (true, Vec::new()),
        (false, low.clone()),
        (false, Vec::new()),
    ] {
        let verdict = voxa_labels::Verdict {
            passed,
            fail_below: 0.65,
            low,
        };
        add(
            "the model's check",
            &parts::labelling::verdict_in_plain_words(&verdict),
        );
    }
    texts
}

#[test]
fn no_text_suggests_a_finding() {
    let texts = texts();
    assert!(texts.len() > 400, "only {} texts are checked", texts.len());
    let mut found = Vec::new();
    for (from, text) in &texts {
        for word in suggestions_in(text) {
            found.push(format!("{word:?} in {from}: {text}"));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// The topic with this id.
fn topic(id: &str) -> &'static Topic {
    TOPICS.iter().find(|topic| topic.id == id).expect("listed")
}

/// The sentences of `text`: each ends at a full stop or a question mark, except that the full
/// stop in a quotation, just before its closing mark, ends the quotation's sentence and no more.
fn sentences(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut letters = text.chars().peekable();
    while let Some(letter) = letters.next() {
        current.push(letter);
        if matches!(letter, '.' | '?') && letters.peek() != Some(&'"') {
            found.push(current.trim().to_owned());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        found.push(current.trim().to_owned());
    }
    found
}

#[test]
fn sentences_end_at_full_stops_and_question_marks_but_not_inside_a_quotation() {
    let said = "One. Two? It says \"Not here.\" and goes on. Three";
    assert_eq!(
        sentences(said),
        [
            "One.",
            "Two?",
            "It says \"Not here.\" and goes on.",
            "Three"
        ]
    );
}

#[test]
fn the_measure_topic_says_where_millimetres_come_from() {
    let body = topic("measure").body;
    // Where the files give the pixel spacing and say where the slices lie: the patient's own.
    for sense in [
        "the pixel spacing",
        "where each slice lies in the patient",
        "patient's millimetres",
    ] {
        assert!(body.contains(sense), "{sense:?} is not in {body}");
    }
    // Where they give the pixel spacing alone: within the picture, and the measurement says so,
    // in the words it says it in, and nothing of the distance between slices.
    let said = measuring::Scale::PixelSpacing.said_after();
    let said = said.trim().trim_matches(['(', ')']);
    assert_eq!(said, "from the file's pixel spacing");
    assert!(body.contains(said), "{body}");
    assert!(
        body.contains("nothing about the distance between slices"),
        "{body}"
    );
    // Where they give none, or the slices differ in size or in the way they face, which the
    // tools say in the words of `NO_SCALE` and `NO_ONE_SCALE`: no millimetres, in one sentence.
    assert!(measuring::NO_SCALE.contains("scale in its files"));
    assert!(measuring::NO_ONE_SCALE.contains("one size and plane"));
    let none = sentences(body)
        .into_iter()
        .find(|sentence| sentence.contains("no pixel spacing"))
        .expect("a sentence says what happens with no pixel spacing");
    for sense in [
        "not all the same size or do not all face the same way",
        "not measured in millimetres",
        "the tools say why",
    ] {
        assert!(none.contains(sense), "{sense:?} is not in {none}");
    }
    // A scan shown as a stack is rough between its slices, and is measured only where measuring
    // is allowed at all, so that slices that differ are not said to be measurable.
    let stack = sentences(body)
        .into_iter()
        .find(|sentence| sentence.contains("stack of slices"))
        .expect("a sentence says what a stack is worth");
    assert!(stack.contains("where it can be measured at all"), "{stack}");
    assert!(stack.contains("only rough"), "{stack}");
    // What a measurement is worth.
    assert!(body.contains("not a clinical result"), "{body}");
}

#[test]
fn the_not_for_topic_carries_the_disclaimer_and_sends_worries_to_a_doctor() {
    let body = topic("not-for").body;
    assert!(body.contains(DISCLAIMER), "{body}");
    for sense in [
        "does not diagnose, screen for or rule out anything",
        "not clinical results",
        "is meant to be written by a qualified person",
        "ask your doctor or another clinician",
        "Do not wait for, or rely on, anything Voxa shows",
    ] {
        assert!(body.contains(sense), "{sense:?} is not in {body}");
    }
}

#[test]
fn the_labels_topic_says_who_makes_labels_and_that_a_passed_check_proves_nothing() {
    let body = topic("labels").body;
    for sense in [
        "A label is a guess that looks certain.",
        "the model's opinion of its own work",
        "does not make the labels right",
        "Made by another program",
        "press Label this scan in the Tools tab",
    ] {
        assert!(body.contains(sense), "{sense:?} is not in {body}");
    }
    // The screen says "opinion of its own work" of a passed check: the topic uses its words.
    let check = voxa_labels::Verdict {
        passed: true,
        fail_below: 0.5,
        low: Vec::new(),
    };
    let screen = crate::ui::parts::labelling::verdict_in_plain_words(&check);
    assert!(
        screen.contains("the model's opinion of its own work"),
        "{screen}"
    );
}

#[test]
fn the_sides_topic_names_the_buttons_of_the_3d_tab_as_they_say_it() {
    let body = topic("sides").body;
    for (_, label, _) in tabs::VIEWS {
        let quoted = format!("\"{label}\"");
        assert!(body.contains(&quoted), "{label:?} is not in {body}");
    }
}

#[test]
fn the_mouse_and_the_topics_call_the_second_number_of_the_window_its_level() {
    let right_drag = MOUSE.iter().find(|(input, _)| *input == "Right drag");
    let (_, does) = right_drag.expect("listed");
    assert!(does.contains("down for level"), "{does}");
    assert!(!does.contains("centre"), "{does}");
    assert!(topic("brightness").body.contains("down changes the level"));
}

#[test]
fn every_topic_has_a_title_some_words_and_at_most_six_short_sentences() {
    for topic in &TOPICS {
        assert!(
            !topic.title.is_empty() && topic.title.len() <= 28,
            "{}",
            topic.id
        );
        assert!(topic.words.len() >= 4, "{} has too few words", topic.id);
        let sentences = sentences(topic.body);
        assert!(
            (3..=6).contains(&sentences.len()),
            "{}: {sentences:?}",
            topic.id
        );
        for sentence in &sentences {
            let words = sentence.split_whitespace().count();
            assert!(words <= 40, "{} has {words} words in: {sentence}", topic.id);
        }
        assert!(topic.body.len() <= 1_500, "{} runs on", topic.id);
        assert!(!topic.body.contains("  "), "{}", topic.id);
    }
}

/// A word of the trade a topic uses, and the plain words that explain it, which must be in the
/// sentence that first uses it.
const EXPLAINED: [(&str, &str, &str); 6] = [
    ("open", "DICOMDIR", "a file that lists the scans"),
    ("measure", "pixel spacing", "the size of each dot"),
    ("measure", "stack of slices", "not a regular 3D volume"),
    ("brightness", "Hounsfield", "a standard scale of density"),
    ("labels", "model", "a computer program"),
    ("not-for", "clinician", "such as a nurse"),
];

/// Words of the trade that no topic uses, since a reader would have to look them up.
const LEFT_OUT: [&str; 6] = [
    "locating image",
    "X-ray",
    "detector",
    "plain stack",
    "radiological",
    "neurological",
];

#[test]
fn every_word_of_the_trade_is_explained_in_its_sentence_or_left_out() {
    for (id, term, gloss) in EXPLAINED {
        let found = sentences(topic(id).body)
            .into_iter()
            .find(|sentence| sentence.contains(term));
        let first = found.unwrap_or_else(|| panic!("{id} does not use {term:?}"));
        assert!(
            first.contains(gloss),
            "{term:?} is not explained in: {first}"
        );
    }
    for term in LEFT_OUT {
        for topic in &TOPICS {
            assert!(!topic.body.contains(term), "{} uses {term:?}", topic.id);
        }
    }
}
