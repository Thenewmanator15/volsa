//! What is chosen for labelling, kept between runs: round trips, files that are not Voxa's,
//! and a card kept by its name. Made-up cards and folders, in temporary folders.

use std::path::{Path, PathBuf};

use voxa_models::{Card, CardId, CardKind, RunsOn};
use voxa_segment::ModelChoice;

use super::*;
use crate::labelling::Labelling;

fn card(number: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(number),
        index: u32::try_from(number).expect("a small number"),
        name: name.to_owned(),
        kind,
        own_bytes: 8 << 30,
    }
}

/// An integrated card, then two discrete ones.
fn cards() -> Vec<Card> {
    vec![
        card(1, "Made-up shared graphics", CardKind::Integrated),
        card(2, "Made-up card A", CardKind::Discrete),
        card(3, "Made-up card B", CardKind::Discrete),
    ]
}

/// Choices that differ from the usual in every way.
fn unusual() -> Choices {
    Choices {
        choice: ModelChoice::Robust,
        cortex: false,
        runs_on: RunsOnKept::Card("Made-up card B".to_owned()),
        keep: false,
        models: Some(PathBuf::from("made-up models folder")),
        runtime: Some(PathBuf::from("made-up runtime folder")),
    }
}

fn file(folder: &Path) -> PathBuf {
    folder.join("choices.json")
}

#[test]
fn choices_that_are_saved_are_loaded_again_exactly() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    save_choices(folder.path(), &unusual()).expect("they are saved");
    assert_eq!(load_choices(folder.path()), unusual());
    let usual = Choices::default();
    save_choices(folder.path(), &usual).expect("they are saved over");
    assert_eq!(
        load_choices(folder.path()),
        usual,
        "the newer ones are kept"
    );
}

#[test]
fn every_way_of_running_and_every_model_is_kept() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let ways = [
        RunsOnKept::Automatic,
        RunsOnKept::Processor,
        RunsOnKept::Card("A card with a name, \"quoted\", and accents: é".to_owned()),
    ];
    let models = [
        ModelChoice::Automatic,
        ModelChoice::Standard,
        ModelChoice::Robust,
    ];
    for (runs_on, choice) in ways.into_iter().zip(models) {
        let chosen = Choices {
            choice,
            runs_on,
            ..Choices::default()
        };
        save_choices(folder.path(), &chosen).expect("they are saved");
        assert_eq!(load_choices(folder.path()), chosen);
    }
}

#[test]
fn with_no_file_the_choices_are_the_usual_ones_and_they_are_the_states_own() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    assert_eq!(load_choices(folder.path()), Choices::default());
    assert_eq!(
        load_choices(&folder.path().join("never made")),
        Choices::default()
    );
    assert_eq!(Choices::of(&Labelling::default()), Choices::default());
}

#[test]
fn a_file_of_another_version_or_of_noise_gives_the_usual_choices() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let mut written = serde_json::json!({"version": 1, "cortex": false, "keep": false});
    std::fs::write(file(folder.path()), written.to_string()).expect("writes");
    let usual = Choices::default();
    let ours = load_choices(folder.path());
    assert_eq!(
        (ours.cortex, ours.keep),
        (false, false),
        "a file of version 1"
    );
    for version in [2, 0, 99] {
        written["version"] = version.into();
        std::fs::write(file(folder.path()), written.to_string()).expect("writes");
        assert_eq!(load_choices(folder.path()), usual, "version {version}");
    }
    let noise = [
        "",
        "not json",
        "[1, 2, 3]",
        "null",
        "{\"cortex\": false}",
        "{\"version\": \"1\"}",
        "{\"version\": 1, \"cortex\": \"yes\"}",
        "\u{0}\u{1}\u{2}",
    ];
    for text in noise {
        std::fs::write(file(folder.path()), text).expect("writes");
        assert_eq!(load_choices(folder.path()), usual, "{text:?}");
    }
}

#[test]
fn a_word_the_file_has_that_voxa_does_not_know_gives_the_usual_choice_for_that_one() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let text = serde_json::json!({
        "version": 1, "choice": "turbo", "runs_on": "quantum", "cortex": false, "keep": false,
        "card": "Made-up card B",
    });
    std::fs::write(file(folder.path()), text.to_string()).expect("writes");
    let loaded = load_choices(folder.path());
    assert_eq!(loaded.choice, ModelChoice::Automatic);
    assert_eq!(loaded.runs_on, RunsOnKept::Automatic);
    assert_eq!(
        (loaded.cortex, loaded.keep),
        (false, false),
        "and the rest stays"
    );
}

#[test]
fn a_card_with_no_name_in_the_file_is_not_a_card() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for card in [serde_json::json!(null), serde_json::json!("")] {
        let text = serde_json::json!({"version": 1, "runs_on": "card", "card": card});
        std::fs::write(file(folder.path()), text.to_string()).expect("writes");
        assert_eq!(load_choices(folder.path()).runs_on, RunsOnKept::Automatic);
    }
}

#[test]
fn the_file_carries_its_version_and_the_two_folders_are_the_only_paths_in_it() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    save_choices(folder.path(), &unusual()).expect("they are saved");
    let text = std::fs::read_to_string(file(folder.path())).expect("reads");
    let read: serde_json::Value = serde_json::from_str(&text).expect("is json");
    assert_eq!(read["version"], CHOICES_VERSION);
    assert_eq!(read["models"], "made-up models folder");
    assert_eq!(read["runtime"], "made-up runtime folder");
    assert_eq!(read["card"], "Made-up card B", "a card by its name");
    let keys: Vec<&String> = read.as_object().expect("an object").keys().collect();
    let expected = [
        "card", "choice", "cortex", "keep", "models", "runs_on", "runtime", "version",
    ];
    assert_eq!(keys, expected, "and nothing of any scan");
    // With no folders chosen there is no path in it at all.
    save_choices(folder.path(), &Choices::default()).expect("they are saved");
    let text = std::fs::read_to_string(file(folder.path())).expect("reads");
    let folder_text = folder.path().to_string_lossy().into_owned();
    assert!(
        !text.contains(&folder_text),
        "not even where it is kept: {text}"
    );
    let read: serde_json::Value = serde_json::from_str(&text).expect("is json");
    let nothing = [&read["models"], &read["runtime"], &read["card"]];
    assert!(nothing.iter().all(|one| one.is_null()), "{text}");
}

#[test]
fn a_folder_that_cannot_be_written_is_an_error() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let in_the_way = folder.path().join("settings");
    std::fs::write(&in_the_way, "a file where the folder should be").expect("writes");
    assert!(save_choices(&in_the_way, &unusual()).is_err());
    assert_eq!(load_choices(&in_the_way), Choices::default());
}

#[test]
fn a_card_is_kept_by_its_name_and_found_again_by_it_wherever_the_list_has_it_now() {
    let chosen = RunsOn::Card(CardId(3));
    let kept = RunsOnKept::of(chosen, &cards());
    assert_eq!(kept, RunsOnKept::Card("Made-up card B".to_owned()));
    assert_eq!(kept.find(&cards()), chosen);
    // After a restart the identities and the places are others; the name is not.
    let others = vec![
        card(70, "Made-up card B", CardKind::Discrete),
        card(71, "Made-up card A", CardKind::Discrete),
    ];
    assert_eq!(kept.find(&others), RunsOn::Card(CardId(70)));
}

#[test]
fn a_card_kept_by_a_name_no_card_has_now_gives_automatic() {
    let kept = RunsOnKept::Card("Made-up card C".to_owned());
    assert_eq!(kept.find(&cards()), RunsOn::Automatic);
    assert_eq!(kept.find(&[]), RunsOn::Automatic);
}

#[test]
fn of_two_cards_with_one_name_the_first_real_one_is_found_and_a_software_card_is_not_real() {
    let name = "Made-up card A";
    let twice = vec![
        card(5, name, CardKind::Software),
        card(6, name, CardKind::Discrete),
        card(7, name, CardKind::Discrete),
    ];
    let kept = RunsOnKept::Card(name.to_owned());
    assert_eq!(kept.find(&twice), RunsOn::Card(CardId(6)));
    assert_eq!(
        kept.find(&twice[..1]),
        RunsOn::Automatic,
        "software alone is none"
    );
}

#[test]
fn a_card_that_is_not_listed_cannot_be_kept_by_a_name_it_is_automatic() {
    assert_eq!(
        RunsOnKept::of(RunsOn::Card(CardId(99)), &cards()),
        RunsOnKept::Automatic
    );
    assert_eq!(
        RunsOnKept::of(RunsOn::Processor, &cards()),
        RunsOnKept::Processor
    );
    assert_eq!(
        RunsOnKept::of(RunsOn::Automatic, &[]),
        RunsOnKept::Automatic
    );
}

#[test]
fn choices_are_taken_from_the_state_and_put_back_in_it() {
    let mut labelling = Labelling {
        cards: cards(),
        ..Labelling::default()
    };
    unusual().put_in(&mut labelling);
    assert_eq!(labelling.choice, ModelChoice::Robust);
    assert_eq!((labelling.cortex, labelling.keep), (false, false));
    assert_eq!(
        labelling.models,
        Some(PathBuf::from("made-up models folder"))
    );
    assert_eq!(
        labelling.runtime,
        Some(PathBuf::from("made-up runtime folder"))
    );
    assert_eq!(Choices::of(&labelling), unusual());
}

#[test]
fn a_card_kept_by_name_waits_in_the_state_for_the_list_of_cards_and_is_not_forgotten() {
    // Put in as Voxa starts, before any card has been listed.
    let mut labelling = Labelling::default();
    unusual().put_in(&mut labelling);
    assert_eq!(labelling.runs_on, RunsOn::Automatic, "no card is known yet");
    assert_eq!(labelling.card_kept.as_deref(), Some("Made-up card B"));
    assert_eq!(
        Choices::of(&labelling),
        unusual(),
        "and what is kept is not lost"
    );
}

#[test]
fn a_card_chosen_in_the_state_is_kept_by_the_name_the_list_gives_it() {
    let labelling = Labelling {
        cards: cards(),
        runs_on: RunsOn::Card(CardId(2)),
        ..Labelling::default()
    };
    let kept = Choices::of(&labelling).runs_on;
    assert_eq!(kept, RunsOnKept::Card("Made-up card A".to_owned()));
}
