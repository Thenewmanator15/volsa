//! Where a run can go: what the computer has, read through the labeller at the moment a run
//! starts; the words for a run that fits nowhere, or not where it was asked to go; and the
//! paths taken out of the runtime's words before they go to the log.

use std::path::Path;

use voxa_models::{
    Card, CardId, CardKind, Cut, Device, FreeMemory, Model, Role, Room, Rung, RunsOn, Wanted, asks,
    needed_gb, needs, rungs, spare_gb, why_not,
};
use voxa_segment::Sizes;

use super::Labeller;

/// The processor, as a run's place is written.
pub(super) const PROCESSOR: &str = "the processor";
/// The notes on a result that ran on the processor after a card gave way.
const NO_ROOM: &str = "Run on the processor: the graphics card did not have room.";
const LOST: &str = "Run on the processor: the graphics card stopped answering.";
/// What is said of a card chosen by hand that the computer no longer lists, or lists as
/// software standing in for a card.
const CARD_GONE: &str = "The graphics card chosen for labelling is not one Voxa can run on now. \
                         Choose where to run it again.";
/// What stands in the log in place of a path.
const A_PATH: &str = "(a path)";

/// What the computer has now, as the labeller reads it: the cards and what the system lets
/// this program use on each, the card Voxa draws on found by its name with what Voxa holds
/// there, and the ordinary memory the system can still give.
pub(super) fn room_of(labeller: &dyn Labeller, drawing_on: Option<&str>, held: u64) -> Room {
    let cards = labeller.cards();
    let budgets = cards.iter().map(|card| labeller.budget(card.id)).collect();
    let drawing_on = drawing_on.and_then(|name| card_named(&cards, name));
    let free_memory = free_bytes(labeller.free_memory());
    Room {
        cards,
        budgets,
        drawing_on,
        held,
        free_memory,
    }
}

/// The first card of this name that is a card and not the processor drawing in its place.
pub(super) fn card_named(cards: &[Card], name: &str) -> Option<CardId> {
    let real = |card: &&Card| card.kind != CardKind::Software && card.name == name;
    cards.iter().find(real).map(|card| card.id)
}

/// The bytes of ordinary memory a run may count on. Where the system does not say what it can
/// still promise, the free physical memory is taken, and the log says so; where nothing could
/// be read, none.
pub(super) fn free_bytes(free: FreeMemory) -> u64 {
    match free {
        FreeMemory::Promised(bytes) => bytes,
        FreeMemory::Physical(bytes) => {
            tracing::info!(
                "labelling goes by the free physical memory: what the system can still promise \
                 could not be read"
            );
            bytes
        }
        FreeMemory::Unknown => {
            tracing::warn!(
                "labelling counts on no ordinary memory: what is free could not be read"
            );
            0
        }
    }
}

/// A card's name, as the computer lists it.
pub(super) fn card_name(room: &Room, id: CardId) -> String {
    let card = room.cards.iter().find(|card| card.id == id);
    card.map_or_else(|| "a graphics card".to_owned(), |card| card.name.clone())
}

/// Where a run went, in words: the processor, or a graphics card by its name.
pub(super) fn place(device: Device, room: &Room) -> String {
    match device {
        Device::Processor => PROCESSOR.to_owned(),
        Device::Card(id) => format!("the graphics card ({})", card_name(room, id)),
    }
}

/// The note on a result that ran elsewhere than on the card that last gave way, `gave_way`,
/// and why that card gave way: it stopped answering, or it did not have room. On the processor
/// the words are those a person with one card reads; on another card, both cards are named.
/// `None` when no card gave way, or the result ran on the card that did (cut to the head, which
/// the result says apart).
pub(super) fn fall(
    gave_way: Option<(Device, bool)>,
    ran_on: Device,
    room: &Room,
) -> Option<String> {
    let (card, lost) = gave_way.filter(|(card, _)| *card != ran_on)?;
    let note = match (ran_on, lost) {
        (Device::Processor, true) => LOST.to_owned(),
        (Device::Processor, false) => NO_ROOM.to_owned(),
        (Device::Card(_), lost) => {
            let why = if lost {
                "stopped answering"
            } else {
                "did not have room"
            };
            format!(
                "Run on {}: {} {why}.",
                place(ran_on, room),
                place(card, room)
            )
        }
    };
    Some(note)
}

/// Why a run fits nowhere it may go. Left to Voxa, what it needs and what the computer has.
/// Asked to go on one place where it does not fit, what the smallest run needs of that place
/// and what the place has, and that it would fit were Voxa to choose where; and a card asked
/// for that the computer does not list, or that is software standing in for one, is not one
/// Voxa can run on.
pub(super) fn does_not_fit(sizes: Sizes, wanted: Wanted, room: &Room, runs_on: RunsOn) -> String {
    let chosen = match runs_on {
        RunsOn::Automatic => return why_not(sizes.whole, sizes.head, wanted, room),
        RunsOn::Processor => Device::Processor,
        RunsOn::Card(id) if room.space_on(Device::Card(id)).is_some() => Device::Card(id),
        RunsOn::Card(_) => return CARD_GONE.to_owned(),
    };
    let smallest = sizes.whole.min(sizes.head);
    let short = short_of(chosen, smallest, wanted, room);
    let place = place(chosen, room);
    let mut words = format!("This scan does not fit on {place}, which was chosen for it.{short}");
    let left_to_voxa = rungs(sizes.whole, sizes.head, wanted, room, RunsOn::Automatic);
    if !left_to_voxa.is_empty() {
        words.push_str(" It would fit if Voxa chose where to run it.");
    }
    words
}

/// What the smallest run, of `smallest` voxels, needs of a place and what the place has, in the
/// units and the rounding `why_not` uses. For a card: what its networks need there against its
/// room, or, when it has room, what the run asks of ordinary memory against what is free. For
/// the processor: the second.
fn short_of(chosen: Device, smallest: u64, wanted: Wanted, room: &Room) -> String {
    let mut words = String::new();
    if let Device::Card(_) = chosen {
        let needed = needs(smallest, wanted, chosen);
        let has = room.space_on(chosen).unwrap_or_default();
        if needed > has {
            return format!(
                " At its smallest it needs about {} GB there, and the card has {} GB to spare.",
                needed_gb(needed),
                spare_gb(has)
            );
        }
        // The scores network may not fit the card: it would go to the processor.
        words.push_str(
            " The card has room for the networks that find the parts, but the computer's \
             ordinary memory is short.",
        );
    }
    // The least a run asks of ordinary memory has the scores network on the processor. What a
    // run asks goes by its voxels, so the cut of the rung makes no difference.
    let (device, cut, scores_on) = (chosen, Cut::Whole, Device::Processor);
    let asked = asks(
        smallest,
        wanted,
        Rung {
            device,
            cut,
            scores_on,
        },
    );
    format!(
        "{words} At its smallest the run asks the computer for about {} GB of ordinary memory, \
         and {} GB is free.",
        needed_gb(asked),
        spare_gb(room.free_memory)
    )
}

/// Words with every path taken out. Each of the `known` paths goes wherever it stands, the
/// first first. Then, in every line, nothing stays from the first sign of a path to the end
/// of the line: a path may hold spaces, and any fragment of one tells something of whose
/// computer it is.
pub(super) fn without_paths(words: &str, known: &[&Path]) -> String {
    let mut words = words.to_owned();
    for path in known {
        let path = path.to_string_lossy();
        if !path.is_empty() {
            words = words.replace(path.as_ref(), A_PATH);
        }
    }
    let cut = words.lines().map(|line| match first_sign(line) {
        Some(at) => format!("{}{A_PATH}", line.get(..at).unwrap_or_default()),
        None => line.to_owned(),
    });
    cut.collect::<Vec<_>>().join("\n")
}

/// Where in a line the first sign of a path stands: a backslash; a slash that starts a word,
/// at the line's start or after anything but a letter or a digit; or a drive letter that
/// starts a word, with its colon and a slash of either kind (the runtime's own `[E:onnxruntime:`
/// is not a drive).
fn first_sign(line: &str) -> Option<usize> {
    let mut before: Option<char> = None;
    for (at, c) in line.char_indices() {
        let starts_a_word = before.is_none_or(|before| !before.is_alphanumeric());
        let sign = match c {
            '\\' => true,
            '/' => starts_a_word,
            letter if letter.is_ascii_alphabetic() && starts_a_word => {
                let after = line.get(at + letter.len_utf8()..).unwrap_or_default();
                let next = after.strip_prefix(':').and_then(|rest| rest.chars().next());
                matches!(next, Some('\\' | '/'))
            }
            _ => false,
        };
        if sign {
            return Some(at);
        }
        before = Some(c);
    }
    None
}

/// The files of a model's networks, and the folders they are in: paths the runtime may name.
fn files_of(model: &Model) -> Vec<&Path> {
    let files = Role::ALL.map(|role| model.file(role));
    let folders = files.iter().filter_map(|file| file.parent());
    files.iter().copied().chain(folders).collect()
}

/// What the log is handed for a network of `model` that could not be run: the runtime's own
/// words, with every path taken out.
pub(super) fn logged_words(words: &str, model: &Model) -> String {
    without_paths(words, &files_of(model))
}
