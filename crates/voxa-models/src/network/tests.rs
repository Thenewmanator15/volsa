//! Tests of everything a network is run through: tensors, the stand-in network, the runtime
//! and the cards. Those marked `ignore` need the real runtime and a real model folder, named
//! by `VOXA_RUNTIME` and `VOXA_MODELS`; the scores network needs some 9 GiB wherever it runs.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::{Device, Network, RunError, Stop, bytes_of, each_alone, right_build};
use crate::cards::{Card, CardId, CardKind, budget, cards};
use crate::model::{Model, Role, read_model};
use crate::runtime::{LIBRARY, Runtime, RuntimeError};
use crate::tensor::{Numbers, Tensor, copied};
use crate::testkit::Canned;

const GIB: u64 = 1 << 30;

#[test]
fn a_tensor_holds_as_many_numbers_as_its_shape_says() {
    assert_eq!(Tensor::f32(vec![2, 3], vec![0.0; 5]), None);
    let whole = Tensor::f32(vec![2, 3], vec![0.5; 6]).expect("six numbers fill two by three");
    assert_eq!(whole.shape, vec![2, 3]);
    assert_eq!(whole.numbers, Numbers::F32(vec![0.5; 6]));
    assert_eq!(Tensor::u8(vec![1, 2, 2], vec![7; 5]), None);
    let bytes = Tensor::u8(vec![1, 2, 2], vec![7; 4]).expect("four bytes fill two by two");
    assert_eq!(bytes.numbers, Numbers::U8(vec![7; 4]));
    // One number has no sides. A shape too large to count holds nothing.
    assert!(Tensor::f32(vec![], vec![1.0]).is_some());
    assert_eq!(Tensor::u8(vec![usize::MAX, 2], vec![]), None);
    // An answer is copied out of the runtime's memory number for number.
    assert_eq!(copied(&[0.25_f32, 0.75]), Ok(vec![0.25, 0.75]));
    assert_eq!(copied::<u8>(&[]), Ok(Vec::new()));
}

#[test]
fn a_canned_network_answers_and_remembers_what_it_was_given() {
    let image = Tensor::f32(vec![1, 2], vec![0.25, 0.75]).expect("a tensor");
    let classes = Tensor::u8(vec![2], vec![3, 4]).expect("a tensor");
    let canned = Canned::new(|inputs| Ok(inputs.last().cloned().into_iter().collect()));
    let given = canned.given();
    // As the rest of Voxa holds one: behind the trait, and on another thread.
    let mut network: Box<dyn Network> = Box::new(canned);
    let inputs = vec![image.clone(), classes.clone()];
    let answer = thread::spawn(move || network.run(inputs)).join();
    assert_eq!(answer.expect("the run"), Ok(vec![classes.clone()]));
    assert_eq!(given.runs(), vec![vec![image, classes]]);

    let mut refusing = Canned::new(|_| Err(RunError::NoRoom));
    assert_eq!(refusing.run(Vec::new()), Err(RunError::NoRoom));
    assert_eq!(refusing.run(Vec::new()), Err(RunError::NoRoom));
    assert_eq!(refusing.given().runs(), vec![Vec::new(), Vec::new()]);
}

#[test]
fn a_stop_is_shared_by_its_copies() {
    let stop = Stop::new();
    let copy = stop.clone();
    assert!(!stop.is_set() && !copy.is_set());
    copy.stop();
    assert!(stop.is_set() && copy.is_set());
    assert!(!Stop::default().is_set());
}

#[test]
fn a_folder_without_the_runtime_is_not_found() {
    let empty = tempfile::tempdir().expect("a temporary folder");
    let not_found = Some(RuntimeError::NotFound);
    assert_eq!(Runtime::load(empty.path()).err(), not_found);
    assert_eq!(
        Runtime::load(&empty.path().join("nowhere")).err(),
        not_found
    );
}

#[test]
fn ten_bytes_of_nothing_would_not_load() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    fs::write(folder.path().join(LIBRARY), [0_u8; 10]).expect("a made-up library");
    let loaded = Runtime::load(folder.path());
    assert_eq!(loaded.err(), Some(RuntimeError::WouldNotLoad));
}

#[test]
fn the_right_build_is_version_one_from_the_interface_asked_for() {
    for taken in ["1.24.4", "1.28.0", "1.17.0"] {
        assert!(right_build(taken), "{taken} should be taken");
    }
    for refused in ["1.16.3", "not a version", "", "24", "2.0.0", "1.x.0"] {
        assert!(!right_build(refused), "{refused:?} should be refused");
    }
}

#[test]
fn the_runtimes_words_are_sorted_into_what_went_wrong() {
    let sorted = RunError::from_words;
    let no_room = "D3D12 error 8007000E: Not enough memory resources are available";
    assert_eq!(sorted(no_room), RunError::NoRoom);
    for lost in ["887A0005", "887a0006", "0x887A0007"] {
        let words = format!("the device said {lost} and went");
        assert_eq!(sorted(&words), RunError::DeviceLost);
    }
    // The code beside those three that only says an adapter is not there is no loss.
    for other in ["Got invalid dimensions", "the device said 887A0002"] {
        assert_eq!(sorted(other), RunError::Failed(other.to_owned()));
    }
}

#[test]
fn memory_the_runtime_could_not_have_is_no_room_in_whatever_words_it_says_so() {
    let sorted = RunError::from_words;
    // The first is what the runtime said when the processor's memory ran out under a real
    // network. The rest are its other wordings for the same thing, and the one a C++ library
    // gives elsewhere than on Windows.
    let refused = [
        "Non-zero status code returned while running Conv node. Name:'blur' Status Message: \
         bad allocation",
        "std::bad_alloc",
        "Failed to allocate memory for requested buffer of size 4096000",
        "BFC Arena ran out of memory trying to allocate 4096000 bytes",
        "Available memory of 128 is smaller than requested bytes of 4096000",
        "Memory allocation failed. Size=4096000",
        "Output buffer allocation failed",
        "VirtualAlloc failed",
    ];
    for words in refused {
        assert_eq!(sorted(words), RunError::NoRoom, "{words}");
        assert_eq!(sorted(&words.to_lowercase()), RunError::NoRoom, "{words}");
    }
    // Words about memory that say nothing of running out stay what they are.
    let other = "Allocation of tensor types requires a shape.";
    assert_eq!(sorted(other), RunError::Failed(other.to_owned()));
}

#[test]
fn every_error_is_a_plain_sentence_without_the_runtimes_words() {
    use RuntimeError::{NotFound, WouldNotLoad, WrongBuild};
    let failed = RunError::Failed("Non-zero status code in C:\\models\\scores.onnx".to_owned());
    let run = [
        RunError::NoRoom,
        RunError::DeviceLost,
        RunError::Stopped,
        failed,
    ];
    let mut sentences: Vec<String> = run.iter().map(ToString::to_string).collect();
    let load = [NotFound, WrongBuild, WouldNotLoad];
    sentences.extend(load.iter().map(ToString::to_string));
    let never = "RunError RuntimeError NoRoom DeviceLost Failed WouldNotLoad WrongBuild NotFound \
                 :: \\ / .dll .onnx status";
    for sentence in &sentences {
        assert!(sentence.ends_with('.'), "not a sentence: {sentence}");
        for word in never.split_whitespace() {
            assert!(!sentence.contains(word), "`{word}` in: {sentence}");
        }
    }
    sentences.sort();
    sentences.dedup();
    assert_eq!(sentences.len(), 7, "two errors read the same");
}

#[test]
fn outputs_past_two_gib_on_a_card_are_fetched_each_alone() {
    let names = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();
    let (free, written) = (names(&["", "x", "y", "z", ""]), names(&[""; 4]));
    let chances = bytes_of(4, &[1, -1, -1, -1, 17], &free, [160, 192, 224]);
    assert_eq!(chances, Some(4 * 160 * 192 * 224 * 17));
    let scores = bytes_of(1, &[1, 224, 224, 224], &written, [8, 8, 8]);
    assert_eq!(scores, Some(224 * 224 * 224));
    // A free side that is none of the three cannot be known before the run.
    assert_eq!(bytes_of(4, &[1, -1], &names(&["", "batch"]), [8; 3]), None);
    assert_eq!(bytes_of(4, &[1, -1], &[], [8; 3]), None);
    assert_eq!(
        bytes_of(4, &[1, -1], &names(&["", "x"]), [u64::MAX; 3]),
        None
    );

    let most = 2 * GIB;
    let card = Device::Card(CardId(1));
    assert!(!each_alone(card, 2, Some(most), most));
    assert!(each_alone(card, 2, Some(most + 1), most));
    assert!(each_alone(Device::Card(CardId(0)), 2, None, most));
    // One output is alone already, and nothing is fetched from the processor.
    assert!(!each_alone(card, 1, Some(most + 1), most));
    assert!(!each_alone(Device::Processor, 2, Some(most + 1), most));
}

/// What the ignored tests need: the runtime loaded and the model read, once for all of them.
static REAL: LazyLock<(Runtime, Model)> = LazyLock::new(|| {
    let runtime = Runtime::load(&folder("VOXA_RUNTIME")).expect("the runtime loads");
    let model = read_model(&folder("VOXA_MODELS")).expect("the model folder reads");
    (runtime, model)
});

/// Held by every test that runs a network, so that no two want their gigabytes at once.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn folder(variable: &str) -> PathBuf {
    let needed = || panic!("This test needs {variable} to name a folder, and it is not set.");
    env::var_os(variable).map_or_else(needed, Into::into)
}

fn turn() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn cards_that_are_not_software() -> Vec<Card> {
    let mut found = cards();
    found.retain(|card| card.kind != CardKind::Software);
    found
}

/// The side of the cube the scores network takes.
fn side() -> usize {
    REAL.1.lists.box_side
}

/// A map of classes that is all background, of the size the scores network takes.
fn zeros() -> Tensor {
    let side = side();
    Tensor::u8(vec![1, side, side, side], vec![0; side * side * side]).expect("a map of zeros")
}

/// Opens the scores network of the real model on a device.
fn scores(device: Device, stop: &Stop) -> Result<Box<dyn Network>, RunError> {
    let (runtime, model) = &*REAL;
    runtime.open(model.file(Role::Scores), device, [side(); 3], stop)
}

/// Whether something failed in the runtime's own words: not a panic, and no other error.
fn failed<T>(outcome: &Result<T, RunError>) -> bool {
    matches!(outcome, Err(RunError::Failed(_)))
}

/// Opens the scores network on a device, runs it on zeros and checks what comes back.
fn scores_on(device: Device) {
    let started = Instant::now();
    let opened = scores(device, &Stop::new());
    let mut network = opened.unwrap_or_else(|error| panic!("{device:?} does not open: {error:?}"));
    let (open, started) = (started.elapsed(), Instant::now());
    let answer = network.run(vec![zeros()]);
    let run = started.elapsed();
    println!("{device:?}: opened in {open:.2?}, ran in {run:.2?}");
    let answer = answer.unwrap_or_else(|error| panic!("{device:?} does not run: {error:?}"));
    let groups = REAL.1.lists.score_groups.len() + 1;
    assert_eq!(answer.len(), 1);
    assert_eq!(answer[0].shape, vec![1, groups]);
    let Numbers::F32(scores) = &answer[0].numbers else {
        panic!("the scores are not floating point numbers")
    };
    assert_eq!(scores.len(), groups);
    assert!(scores.iter().all(|score| score.is_finite()), "{scores:?}");
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB of memory"]
fn the_scores_network_runs_on_the_processor() {
    let _turn = turn();
    scores_on(Device::Processor);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB on each graphics card"]
fn the_scores_network_runs_on_each_card() {
    let _turn = turn();
    for card in cards_that_are_not_software() {
        let (own, spare) = (card.own_bytes / GIB, budget(card.id));
        let spare = spare.map(|bytes| bytes / GIB);
        println!("{card:?}: {own} GiB of its own, {spare:?} GiB to spare");
        scores_on(Device::Card(card.id));
    }
}

#[test]
#[ignore = "needs VOXA_RUNTIME and VOXA_MODELS"]
fn what_cannot_be_run_or_opened_fails_without_a_panic() {
    let _turn = turn();
    let mut network = scores(Device::Processor, &Stop::new()).expect("it opens");
    let small = Tensor::u8(vec![1, 8, 8, 8], vec![0; 512]).expect("a small map");
    let floats = Tensor::f32(vec![1, 8, 8, 8], vec![0.0; 512]).expect("a small image");
    let mut torn = zeros();
    torn.numbers = Numbers::U8(vec![0; 5]);
    assert!(failed(&network.run(vec![small.clone()])), "the wrong shape");
    assert!(failed(&network.run(vec![small, zeros()])), "too many");
    assert!(failed(&network.run(Vec::new())), "no input");
    assert!(failed(&network.run(vec![floats])), "the wrong kind");
    assert!(failed(&network.run(vec![torn])), "not whole");

    let (runtime, stop) = (&REAL.0, Stop::new());
    let made_up = tempfile::tempdir().expect("a temporary folder");
    let nothing = made_up.path().join("nothing.onnx");
    let absent = made_up.path().join("absent");
    fs::write(&nothing, [0_u8; 10]).expect("a made-up file");
    for file in [nothing, absent] {
        let opened = runtime.open(&file, Device::Processor, [32; 3], &stop);
        assert!(failed(&opened), "{file:?} opened");
    }
    // A card the system does not list is a card that went.
    let nowhere = scores(Device::Card(CardId(u64::MAX)), &stop);
    assert_eq!(nowhere.err(), Some(RunError::DeviceLost), "no such card");
}

#[test]
#[ignore = "needs VOXA_RUNTIME and VOXA_MODELS"]
fn a_stop_set_beforehand_stops() {
    let _turn = turn();
    let stop = Stop::new();
    let mut network = scores(Device::Processor, &stop).expect("it opens");
    stop.stop();
    assert_eq!(network.run(vec![zeros()]), Err(RunError::Stopped));
    let opened = scores(Device::Processor, &stop);
    assert_eq!(opened.err(), Some(RunError::Stopped));
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB of memory"]
fn a_stop_set_during_a_run_on_the_processor_ends_it() {
    let _turn = turn();
    let stop = Stop::new();
    let mut network = scores(Device::Processor, &stop).expect("it opens");
    let later = stop.clone();
    let setter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        later.stop();
    });
    let started = Instant::now();
    let outcome = network.run(vec![zeros()]);
    println!("the run ended {:.2?} after it began", started.elapsed());
    setter.join().expect("the thread that stops");
    assert_eq!(outcome, Err(RunError::Stopped));
}

/// The largest difference between two answers of the same shapes, number by number.
fn furthest_apart(ours: &[Tensor], theirs: &[Tensor]) -> f32 {
    assert_eq!(ours.len(), theirs.len());
    let mut furthest = 0.0_f32;
    for (ours, theirs) in ours.iter().zip(theirs) {
        assert_eq!(ours.shape, theirs.shape);
        let (Numbers::F32(ours), Numbers::F32(theirs)) = (&ours.numbers, &theirs.numbers) else {
            panic!("not floating point numbers")
        };
        let apart = ours.iter().zip(theirs).map(|(a, b)| (a - b).abs());
        furthest = apart.fold(furthest, f32::max);
    }
    furthest
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and a graphics card"]
fn a_card_gives_the_processors_answer_however_it_is_fetched() {
    let (runtime, model) = &*REAL;
    let _turn = turn();
    let (side, stop, standard) = (64, Stop::new(), model.file(Role::Standard));
    let made_up = (0..side * side * side).map(|place| (place % 97) as f32 / 97.0);
    let image = Tensor::f32(vec![1, side, side, side, 1], made_up.collect()).expect("an image");
    let processor = runtime.open(standard, Device::Processor, [side; 3], &stop);
    let expected = processor.expect("it opens").run(vec![image.clone()]);
    let expected = expected.expect("it runs on the processor");
    let shapes: Vec<&[usize]> = expected.iter().map(|part| part.shape.as_slice()).collect();
    let (first, second) = ([1, side, side, side, 17], [1, side, side, side, 16]);
    assert_eq!(shapes, [first, second]);
    for Card { id, index, .. } in cards_that_are_not_software() {
        let device = Device::Card(id);
        let together = runtime.open(standard, device, [side; 3], &stop);
        let alone = runtime.open_with(standard, device, [side; 3], &stop, 0);
        let (mut together, mut alone) = (together.expect("it opens"), alone.expect("it opens"));
        let one_run = together.run(vec![image.clone()]).expect("one run for both");
        let two_runs = alone.run(vec![image.clone()]).expect("a run for each");
        let ways = furthest_apart(&two_runs, &one_run);
        let devices = furthest_apart(&one_run, &expected);
        println!("card {index}: {ways:e} between the two ways, {devices:e} from the processor");
        assert!(ways < 1e-4 && devices < 1e-4, "{ways} and {devices}");
    }
}

#[test]
#[ignore = "needs VOXA_RUNTIME and VOXA_MODELS"]
fn the_same_folder_loads_again_and_another_is_refused() {
    let _ = &*REAL;
    let first = folder("VOXA_RUNTIME");
    assert!(Runtime::load(&first).is_ok());
    let again = Runtime::load(&first.join("."));
    assert!(again.is_ok(), "the same folder, written another way");
    // A true copy of the runtime in another folder: the program already holds the first.
    let other = tempfile::tempdir().expect("a temporary folder");
    fs::copy(first.join(LIBRARY), other.path().join(LIBRARY)).expect("a copy of the runtime");
    let loaded = Runtime::load(other.path());
    assert_eq!(loaded.err(), Some(RuntimeError::WouldNotLoad));
}
