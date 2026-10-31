//! The wording rule: nothing Voxa says suggests a finding, so that no label or measurement
//! reads as a verdict on a person's body. The topics are held to it, to the facts of the
//! window they describe, and to the risk register.

use voxa_segment::Stage;

use super::topics::Topic;
use super::{MOUSE, TOPICS};
use crate::profile::CT_WINDOWS;
use crate::state::AppState;
use crate::testing::{labelled_head, showing_a_scan_of, thick_scan_ready};
use crate::ui::theme::DISCLAIMER;
use crate::ui::{about, commands, document, parts, tabs};
use crate::{labels, measuring};
use voxa_volume::PRESETS;

/// The words no text may hold, whole and whatever their case: a verdict, a measure of one, or
/// a name for what one finds.
const BANNED: [&str; 9] = [
    "normal",
    "abnormal",
    "lesion",
    "tumour",
    "tumor",
    "finding",
    "findings",
    "score",
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
    texts
}

/// States to ask each action why it cannot be used in: nothing open, a scan with its 3D view
/// ready, labels open on it, and a CT scan whose values are not in Hounsfield units.
fn states() -> Vec<AppState> {
    vec![
        AppState::default(),
        thick_scan_ready(),
        labelled_head().0,
        showing_a_scan_of("CT", Some("OTHER")),
    ]
}

/// The stages a labelling run is in.
fn stages() -> [Stage; 6] {
    [
        Stage::Preparing,
        Stage::Finding { step: 1, of: 2 },
        Stage::Finding { step: 3, of: 3 },
        Stage::Cortex,
        Stage::Checking,
        Stage::Tidying,
    ]
}

/// The sentences the measuring, the labels, the reports and the windows keep in constants.
fn other_sentences() -> Vec<&'static str> {
    vec![
        DISCLAIMER,
        about::INTENDED_USE,
        measuring::NO_SCALE,
        measuring::NO_ONE_SCALE,
        measuring::NOT_SHOWN,
        measuring::IN_COLOUR,
        measuring::PLAIN_STACK,
        labels::NOWHERE_NEAR,
        labels::NOTHING_TO_LABEL,
        labels::LABELS_WAIT,
        parts::WHAT_LABELS_ARE,
        parts::MADE_ELSEWHERE,
        parts::NO_VOLUME,
        document::HEADING,
        document::CUT_SHORT,
        document::IS_A_PDF,
        document::CANNOT_SHOW,
    ]
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

#[test]
fn the_measure_topic_says_where_millimetres_come_from() {
    let body = topic("measure").body;
    // Where the files say where the slices lie: the patient's own millimetres.
    assert!(body.contains("where each slice lies"), "{body}");
    assert!(body.contains("patient's millimetres"), "{body}");
    // Where they give a pixel spacing alone: within the picture, and the measurement says so,
    // in the words it says it in.
    let said = measuring::Scale::PixelSpacing.said_after();
    let said = said.trim().trim_matches(['(', ')']);
    assert_eq!(said, "from the file's pixel spacing");
    assert!(body.contains(said), "{body}");
    // Where they give none: no millimetres, and the tools say why, in their own words.
    assert!(body.contains(measuring::NO_SCALE), "{body}");
    assert!(body.contains("not measured in millimetres"), "{body}");
    // What a measurement is worth.
    assert!(body.contains("not a clinical result"), "{body}");
}

#[test]
fn the_not_for_topic_carries_the_disclaimer_and_sends_worries_to_a_clinician() {
    let body = topic("not-for").body;
    assert!(body.contains(DISCLAIMER), "{body}");
    for sense in [
        "does not diagnose, screen for or rule out anything",
        "not clinical results",
        "ask a clinician",
        "Do not wait for, or rely on, anything Voxa shows",
    ] {
        assert!(body.contains(sense), "{sense:?} is not in {body}");
    }
}

#[test]
fn the_labels_topic_says_who_makes_labels_and_that_a_passed_check_proves_nothing() {
    let body = topic("labels").body;
    for sense in [
        "a guess that looks certain",
        "does not make the labels right",
        "Made by another program",
        tabs::LABEL_TOOL,
    ] {
        assert!(body.contains(sense), "{sense:?} is not in {body}");
    }
}

#[test]
fn the_topics_name_the_controls_as_the_window_does() {
    let name = |id: &str| commands::find(id).expect("listed").label;
    let says = |id: &str, words: &[&str]| {
        let body = topic(id).body;
        for word in words {
            assert!(body.contains(word), "{id} does not say {word:?}: {body}");
        }
    };
    says("open", &[name("open.folder"), name("open.file"), "Open"]);
    says(
        "find",
        &[
            name("rail.other"),
            "Person 1",
            "plain name",
            document::HEADING,
        ],
    );
    let ct = CT_WINDOWS.map(|window| window.name);
    let mr = PRESETS.map(|preset| preset.name);
    says("brightness", &ct);
    says("brightness", &mr);
    says("brightness", &["Window width", "Window level", "View tab"]);
    let tools = [
        "measure.ruler",
        "measure.angle",
        "measure.region",
        "measure.marker",
    ];
    says("measure", &tools.map(name));
    let side = commands::find("view.radiological").expect("listed").name;
    says("sides", &[side, "R and L", "A for the front"]);
    says("labels", &[name("labels.open"), "Labels tab", "Tools tab"]);
}

#[test]
fn every_topic_has_a_title_some_words_and_a_few_sentences() {
    for topic in &TOPICS {
        assert!(
            !topic.title.is_empty() && topic.title.len() <= 28,
            "{}",
            topic.id
        );
        assert!(topic.words.len() >= 4, "{} has too few words", topic.id);
        let sentences = topic.body.matches(['.', '?']).count();
        assert!((3..=12).contains(&sentences), "{}: {sentences}", topic.id);
        assert!(topic.body.len() <= 1_500, "{} runs on", topic.id);
        assert!(!topic.body.contains("  "), "{}", topic.id);
    }
}
