//! The worker that labels a scan: looking for the model and the runtime, finding where a run
//! fits, and the small pieces a run is made of. Stand-ins and made-up folders only.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::channel;

use voxa_models::{
    Card, CardId, CardKind, FreeMemory, Role, Room, RunsOn, read_model, rungs, why_not,
};

use super::lab::{Lab, failed_run};
use super::labeller::date_of;
use super::stand_ins::{GIB, ROBUST, StandIn, card_a, head, models};
use super::{Kit, card_named, free_bytes, logged_words, room_of, without_paths};
use crate::action::Effect;
use crate::cache::FrameCache;
use crate::labelling::{LabellingAction, Ready};
use crate::volume_store::VolumeStore;
use crate::worker::Workers;

const WHERE: &str =
    "MODELS.md beside the program (docs/models.md in the source) says what goes where.";

fn card(id: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(id),
        index: 0,
        name: name.to_owned(),
        kind,
        own_bytes: 8 * GIB,
    }
}

#[test]
fn a_model_folder_with_a_network_that_is_not_the_one_listed_is_not_ready_and_no_path_is_said() {
    let lab = Lab::unlooked(StandIn::roomy());
    let standard = lab.folder.path().join("standard.onnx");
    let mut bytes = std::fs::read(&standard).expect("a made-up network");
    bytes[0] ^= 1;
    std::fs::write(&standard, bytes).expect("damaged");
    let Ready::No(why) = lab.look(1, Some(lab.folder.path())) else {
        panic!("not ready");
    };
    assert_eq!(
        why,
        format!(
            "The standard network is not the one the list names, so it may be damaged. {WHERE}"
        )
    );
    let here = lab.folder.path().to_string_lossy().into_owned();
    assert!(
        !why.contains(&here) && !why.contains("standard.onnx"),
        "{why}"
    );
    // No model folder at all, where one was chosen or beside the program.
    let gone = lab.folder.path().join("not here");
    let Ready::No(why) = lab.look(2, Some(&gone)) else {
        panic!("not ready");
    };
    assert_eq!(
        why,
        format!("There is no model folder at the place given. {WHERE}")
    );
}

#[test]
fn with_no_model_beside_the_program_and_none_chosen_the_words_say_to_choose_its_folder() {
    let lab = Lab::unlooked(StandIn::roomy());
    let Ready::No(why) = lab.look(1, None) else {
        panic!("not ready");
    };
    let none = "Voxa found no model: there is no folder called models beside it, and none has \
                been chosen. Choose the folder that holds the model, under Options.";
    assert_eq!(why, format!("{none} {WHERE}"));
    // A folder chosen that holds nothing is said to be no model folder.
    let empty = tempfile::tempdir().expect("a folder");
    let Ready::No(why) = lab.look(2, Some(empty.path())) else {
        panic!("not ready");
    };
    let not_one = "That is not a model folder: its list is missing, unreadable or names a bad \
                   file.";
    assert_eq!(why, format!("{not_one} {WHERE}"));
}

/// What a look finds of the runtime, looked for as it is when Voxa runs (no stand-ins), with a
/// model folder that reads and this runtime folder chosen, if one is.
fn runtime_looked_for(runtime: Option<&Path>) -> Ready {
    let (sender, reports) = channel();
    let cache = Arc::new(std::sync::Mutex::new(FrameCache::new(1 << 20)));
    let volumes = Arc::new(std::sync::Mutex::new(VolumeStore::default()));
    let workers = Workers::new(sender, cache, volumes, None, None);
    let models = models();
    let look = Effect::LookForLabelling {
        look: 4,
        models: Some(models.path().to_owned()),
        runtime: runtime.map(Path::to_path_buf),
    };
    workers.run(look, None);
    let said = reports.recv_timeout(std::time::Duration::from_secs(120));
    let Ok(crate::Action::Labelling(LabellingAction::LabellingLooked { look, ready, .. })) = said
    else {
        panic!("a look says what it found: {said:?}");
    };
    assert_eq!(look, 4);
    ready
}

#[test]
fn with_no_runtime_beside_the_program_or_in_the_folder_chosen_this_computer_is_not_ready() {
    let empty = tempfile::tempdir().expect("a folder");
    let runtime = "The runtime is not in the folder given.";
    assert_eq!(
        runtime_looked_for(Some(empty.path())),
        Ready::No(format!("{runtime} {WHERE}"))
    );
    // With none chosen, the words say what to do, and not that a folder was given.
    let none = "Voxa found no runtime: there is no folder called runtime beside it, and none \
                has been chosen. Choose the folder that holds the runtime, under Options.";
    assert_eq!(
        runtime_looked_for(None),
        Ready::No(format!("{none} {WHERE}"))
    );
}

#[test]
fn what_an_older_look_found_is_not_kept_in_place_of_the_newest() {
    let kit = Kit::new(Some(Arc::new(StandIn::roomy())));
    let (first, second) = (models(), models());
    let read = |folder: &Path| Arc::new(read_model(folder).expect("reads"));
    kit.asked_to_look(1);
    kit.asked_to_look(2);
    assert!(!kit.found(1, Some(read(first.path()))), "old news");
    assert!(kit.model().is_none());
    let newest = read(second.path());
    assert!(kit.found(2, Some(Arc::clone(&newest))));
    assert!(!kit.found(1, Some(read(first.path()))));
    assert!(kit.model().is_some_and(|kept| Arc::ptr_eq(&kept, &newest)));
    // A newest look that finds the computer unable takes the model away.
    kit.asked_to_look(3);
    assert!(kit.found(3, None));
    assert!(kit.model().is_none());
}

#[test]
fn with_no_room_anywhere_a_run_fails_with_what_it_needs_and_what_there_is() {
    let lab = Lab::new(StandIn::of(Vec::new(), Vec::new(), GIB));
    lab.label(1, RunsOn::Automatic);
    let (_, why) = failed_run(&lab.until_the_end());
    let sizes = voxa_segment::sizes(&head()).expect("sizes");
    let room = Room {
        cards: Vec::new(),
        budgets: Vec::new(),
        drawing_on: None,
        held: 0,
        free_memory: GIB,
    };
    assert_eq!(why, why_not(sizes.whole, sizes.head, ROBUST, &room));
    assert_eq!(lab.stand_in.opened(), [], "nothing was tried");
}

/// Why a run asked to go on `runs_on` failed, with these cards, budgets and ordinary memory.
fn refused(cards: Vec<Card>, budgets: Vec<Option<u64>>, free: u64, runs_on: RunsOn) -> String {
    let lab = Lab::new(StandIn::of(cards, budgets, free));
    lab.label(1, runs_on);
    let (_, why) = failed_run(&lab.until_the_end());
    assert_eq!(lab.stand_in.opened(), [], "nothing was tried");
    why
}

#[test]
fn a_card_chosen_by_hand_that_is_too_small_says_what_the_run_needs_and_what_the_card_has() {
    let small = card(3, "Made-up card B", CardKind::Discrete);
    let cards = vec![card_a(), small.clone()];
    let budgets = vec![Some(16 * GIB), Some(3 * GIB)];
    let sentence = "This scan does not fit on the graphics card (Made-up card B), which was \
                    chosen for it. At its smallest it needs about 3.5 GB there, and the card \
                    has 3.0 GB to spare.";
    // Left to Voxa, it would fit on the other card; and where it fits nowhere nothing is added.
    let would = " It would fit if Voxa chose where to run it.";
    let why = refused(
        cards.clone(),
        budgets.clone(),
        64 * GIB,
        RunsOn::Card(small.id),
    );
    assert_eq!(why, format!("{sentence}{would}"));
    let why = refused(cards, budgets, GIB, RunsOn::Card(small.id));
    assert_eq!(why, sentence);
}

#[test]
fn a_card_chosen_by_hand_with_room_but_short_of_ordinary_memory_says_what_the_run_asks_for() {
    // The card has room for the networks that find the parts, and not for the scores one.
    let why = refused(
        vec![card_a()],
        vec![Some(6 * GIB)],
        5 * GIB,
        RunsOn::Card(card_a().id),
    );
    assert_eq!(
        why,
        "This scan does not fit on the graphics card (Made-up card A), which was chosen for \
         it. The card has room for the networks that find the parts, but the computer's \
         ordinary memory is short. At its smallest the run asks the computer for about 5.2 GB \
         of ordinary memory, and 5.0 GB is free. It would fit if Voxa chose where to run it."
    );
}

#[test]
fn the_processor_chosen_by_hand_says_what_the_run_asks_the_computer_for_and_what_is_free() {
    let why = refused(vec![card_a()], vec![Some(16 * GIB)], GIB, RunsOn::Processor);
    assert_eq!(
        why,
        "This scan does not fit on the processor, which was chosen for it. At its smallest the \
         run asks the computer for about 4.4 GB of ordinary memory, and 1.0 GB is free."
    );
}

#[test]
fn a_card_chosen_by_hand_that_the_computer_no_longer_lists_is_said_to_be_gone() {
    let gone = "The graphics card chosen for labelling is not one Voxa can run on now. Choose \
                where to run it again.";
    let why = refused(
        vec![card_a()],
        vec![Some(16 * GIB)],
        64 * GIB,
        RunsOn::Card(CardId(99)),
    );
    assert_eq!(why, gone);
    // Nor is one the computer lists as software standing in for a card.
    let software = card(7, "Made-up card S", CardKind::Software);
    let cards = vec![card_a(), software.clone()];
    let why = refused(
        cards,
        vec![Some(16 * GIB), None],
        64 * GIB,
        RunsOn::Card(software.id),
    );
    assert_eq!(why, gone);
}

#[test]
fn what_the_computer_has_is_read_with_the_card_voxa_draws_on_found_by_its_name() {
    let software = card(7, "Made-up card A", CardKind::Software);
    let cards = vec![
        software,
        card_a(),
        card(3, "Made-up card B", CardKind::Discrete),
    ];
    assert_eq!(card_named(&cards, "Made-up card A"), Some(card_a().id));
    assert_eq!(card_named(&cards, "Made-up card B"), Some(CardId(3)));
    assert_eq!(card_named(&cards, "Made-up card C"), None);
    let budgets = vec![None, Some(16 * GIB), Some(3 * GIB)];
    let stand_in = StandIn::of(cards.clone(), budgets.clone(), 64 * GIB);
    let room = room_of(&stand_in, Some("Made-up card A"), 2 * GIB);
    let expected = Room {
        cards,
        budgets,
        drawing_on: Some(card_a().id),
        held: 2 * GIB,
        free_memory: 64 * GIB,
    };
    assert_eq!(room, expected);
    assert_eq!(room_of(&stand_in, None, 0).drawing_on, None);
    // The rungs that room gives start on the card the run is asked to go on.
    let sizes = voxa_segment::sizes(&head()).expect("sizes");
    let first = rungs(sizes.whole, sizes.head, ROBUST, &room, RunsOn::Automatic);
    assert_eq!(
        first.first().map(|rung| rung.device),
        Some(voxa_models::Device::Card(card_a().id))
    );
}

#[test]
fn the_free_memory_is_what_the_system_can_promise_or_failing_that_the_physical() {
    assert_eq!(free_bytes(FreeMemory::Promised(12 * GIB)), 12 * GIB);
    assert_eq!(free_bytes(FreeMemory::Physical(4 * GIB)), 4 * GIB);
    assert_eq!(free_bytes(FreeMemory::Unknown), 0, "nothing is promised");
}

#[test]
fn the_paths_the_runtime_is_known_to_name_are_taken_out_wherever_they_stand() {
    let folder = Path::new(r"C:\Users\Made Up\models");
    let file = folder.join("standard.onnx");
    let words = format!("Load model from {} failed: no such file", file.display());
    let cleaned = without_paths(&words, &[file.as_path(), folder]);
    assert_eq!(cleaned, "Load model from (a path) failed: no such file");
}

#[test]
fn from_the_first_sign_of_a_path_the_rest_of_the_line_is_taken_out_spaces_and_all() {
    let cleaned = |words: &str| without_paths(words, &[]);
    // A drive and its colon, with either slash, and folders with spaces in their names.
    let words = r"cannot open C:\Users\Made Up\Data Files\head.nii now";
    assert_eq!(cleaned(words), "cannot open (a path)");
    assert_eq!(
        cleaned("cannot open D:/export dir/head.nii now"),
        "cannot open (a path)"
    );
    // A backslash anywhere: a share, or a path in the extended form.
    assert_eq!(
        cleaned(r"no access to \\server\My Share\x"),
        "no access to (a path)"
    );
    assert_eq!(cleaned(r"\\?\C:\Made Up\x"), "(a path)");
    // A slash that starts a word, with a fragment of the folder's name after a space.
    assert_eq!(
        cleaned("cannot open /home/My Data Files/x.nii now"),
        "cannot open (a path)"
    );
    assert_eq!(cleaned("/home/My Data Files/x.nii"), "(a path)");
    // A slash after a quote, a bracket, an equals sign or a colon starts a word too.
    assert_eq!(
        cleaned("failed to load '/opt/my models/x.onnx'"),
        "failed to load '(a path)"
    );
    assert_eq!(cleaned("file=/opt/my models/x (3)"), "file=(a path)");
    assert_eq!(
        cleaned("see https://example.test/some page"),
        "see https:(a path)"
    );
}

#[test]
fn only_the_line_that_holds_a_path_is_cut_and_words_with_no_path_in_them_are_left_alone() {
    let words = "first line\nsecond /a b/c d\nthird line";
    assert_eq!(
        without_paths(words, &[]),
        "first line\nsecond (a path)\nthird line"
    );
    for words in [
        "out of memory",
        "scores and/or tidying were skipped",
        "Error: step A: failed at 3/4",
        "[E:onnxruntime:, runner.cc:12] Exception during initialization",
        "the 32 bit size is 4 or 8",
        "",
    ] {
        assert_eq!(without_paths(words, &[]), words);
    }
}

#[test]
fn what_the_log_is_handed_for_a_network_that_failed_has_no_path_in_it() {
    let folder = models();
    let model = read_model(folder.path()).expect("reads");
    let file = model.file(Role::Standard).display().to_string();
    let words = format!(
        r"Load model from {file} failed: no such file; the runtime is in C:\Some One\Elsewhere\x.dll"
    );
    let logged = logged_words(&words, &model);
    assert_eq!(
        logged,
        "Load model from (a path) failed: no such file; the runtime is in (a path)"
    );
    let here = folder.path().to_string_lossy().into_owned();
    assert!(
        !logged.contains(&here) && !logged.contains("standard.onnx"),
        "{logged}"
    );
}

#[test]
fn a_day_is_written_as_its_year_month_and_day() {
    assert_eq!(date_of(47_117), "2099-01-01");
    assert_eq!(date_of(47_118), "2099-01-02");
    assert_eq!(date_of(47_175), "2099-02-28");
    assert_eq!(date_of(47_176), "2099-03-01", "2099 has no leap day");
    assert_eq!(date_of(47_481), "2099-12-31");
}
