//! Tests of a network run on each graphics card of this computer. Each is ignored unless it is
//! asked for, and needs the real runtime and a model folder, named by `VOLSA_RUNTIME` and
//! `VOLSA_MODELS`. A computer with no card says so on a line of its own that `cargo xtask
//! check-models` shows, and passes. The card asked for must be the card that worked: what this
//! process holds on it must rise while the network is on it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::thread;
use std::time::Duration;

use super::tests::{GIB, REAL, scores_on, turn};
use super::{Device, Stop};
use crate::cards::{Card, CardId, CardKind, budget, cards, in_use};
use crate::model::Role;
use crate::tensor::{Numbers, Tensor};

/// What this process holds on a card must rise by at least this much while a network is on
/// it: 1 GiB. The networks of these tests take well over that of a card, and before a run the
/// process holds nothing there. A run that went to another card would leave this one where
/// it was.
const CARD_ROSE: u64 = GIB;
/// How often a card is asked how much of it this process holds.
const LOOK_EVERY: Duration = Duration::from_millis(20);

fn cards_that_are_not_software() -> Vec<Card> {
    let mut found = cards();
    found.retain(|card| card.kind != CardKind::Software);
    found
}

/// Says that a test has nothing to run on, in the line `cargo xtask check-models` looks for,
/// when this computer has no card to try; and whether it has none.
fn skip_without(found: &[Card]) -> bool {
    let none = found.is_empty();
    if none {
        println!("skipped: this computer has no graphics card that is not software");
    }
    none
}

#[test]
fn a_test_with_no_card_to_try_says_so_and_one_with_a_card_does_not() {
    let card = Card {
        id: CardId(1),
        index: 0,
        name: "a made-up card".to_owned(),
        kind: CardKind::Discrete,
        own_bytes: 0,
    };
    assert!(skip_without(&[]));
    assert!(!skip_without(&[card]));
}

/// Does some work and looks meanwhile at how much of a card this process holds: the work's
/// answer, and by how many bytes the most that was seen rose over what was held just before.
/// A count that cannot be read before the work is a failure to measure, and not a zero.
fn rise_of<T>(card: CardId, work: impl FnOnce() -> T) -> (T, u64) {
    /// Tells the watcher to end, also when the work panics.
    struct Done<'a>(&'a AtomicBool);
    impl Drop for Done<'_> {
        fn drop(&mut self) {
            self.0.store(true, Relaxed);
        }
    }
    let before = in_use(card).expect("what the process holds on the card can be read");
    let (done, most) = (AtomicBool::new(false), AtomicU64::new(0));
    let answer = thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Relaxed) {
                most.fetch_max(in_use(card).unwrap_or(0), Relaxed);
                thread::sleep(LOOK_EVERY);
            }
        });
        let _done = Done(&done);
        work()
    });
    (answer, most.into_inner().saturating_sub(before))
}

/// Does some work that puts a network on a card, and holds it to having been that card.
fn on_that_card<T>(card: &Card, work: impl FnOnce() -> T) -> T {
    let (answer, rose) = rise_of(card.id, work);
    let gib = rose as f64 / GIB as f64;
    println!(
        "{}: what the process holds on it rose by {gib:.2} GiB",
        card.name
    );
    assert!(
        rose >= CARD_ROSE,
        "{} held only {rose} bytes more: the network was not on it",
        card.name
    );
    answer
}

#[test]
#[ignore = "needs VOLSA_RUNTIME, VOLSA_MODELS and 9 GiB on each graphics card"]
fn the_scores_network_runs_on_each_card() {
    let _turn = turn();
    let found = cards_that_are_not_software();
    if skip_without(&found) {
        return;
    }
    for card in found {
        let (own, spare) = (card.own_bytes / GIB, budget(card.id));
        let spare = spare.map(|bytes| bytes / GIB);
        println!("{card:?}: {own} GiB of its own, {spare:?} GiB to spare");
        on_that_card(&card, || scores_on(Device::Card(card.id)));
    }
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
#[ignore = "needs VOLSA_RUNTIME, VOLSA_MODELS and a graphics card"]
fn a_card_gives_the_processors_answer_however_it_is_fetched() {
    let (runtime, model) = &*REAL;
    let _turn = turn();
    // With no card there is nothing to compare the processor with, and no reason to run it.
    let found = cards_that_are_not_software();
    if skip_without(&found) {
        return;
    }
    // The image is a cube of 96, which puts more than a GiB of the standard network on a card;
    // one of 64 puts less.
    let (side, stop, standard) = (96, Stop::new(), model.file(Role::Standard));
    let made_up = (0..side * side * side).map(|place| (place % 97) as f32 / 97.0);
    let image = Tensor::f32(vec![1, side, side, side, 1], made_up.collect()).expect("an image");
    let processor = runtime.open(standard, Device::Processor, [side; 3], &stop);
    let expected = processor.expect("it opens").run(vec![image.clone()]);
    let expected = expected.expect("it runs on the processor");
    let shapes: Vec<&[usize]> = expected.iter().map(|part| part.shape.as_slice()).collect();
    let (first, second) = ([1, side, side, side, 17], [1, side, side, side, 16]);
    assert_eq!(shapes, [first, second]);
    for card in found {
        let device = Device::Card(card.id);
        let (one_run, two_runs) = on_that_card(&card, || {
            let together = runtime.open(standard, device, [side; 3], &stop);
            let sides = crate::runtime::sides_named([side; 3]);
            let alone = runtime.open_with(standard, device, &sides, None, &stop, 0);
            let (mut together, mut alone) = (together.expect("it opens"), alone.expect("it opens"));
            let one_run = together.run(vec![image.clone()]).expect("one run for both");
            (
                one_run,
                alone.run(vec![image.clone()]).expect("a run for each"),
            )
        });
        let ways = furthest_apart(&two_runs, &one_run);
        let devices = furthest_apart(&one_run, &expected);
        let index = card.index;
        println!("card {index}: {ways:e} between the two ways, {devices:e} from the processor");
        assert!(ways < 1e-4 && devices < 1e-4, "{ways} and {devices}");
    }
}
