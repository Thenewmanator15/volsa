//! What a plugin was measured to need, kept beside it, and where a run fits by it.

use std::io::{self, PipeReader, PipeWriter};
use std::sync::{Arc, Mutex};
use std::thread;

use voxa_models::{Card, CardId, CardKind, Device, Room, RunsOn};

use super::{
    CardFigures, Fits, HelperPipes, HelperStarter, MEASURED, Measured, measure, own_bytes, places,
    read_measured, write_measured,
};
use crate::helper::{Client, OpenJob, Opener, Where, serve};

/// A GiB.
const GIB: u64 = 1 << 30;

/// A card of this name and kind.
fn card(id: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(id),
        index: 0,
        name: name.to_owned(),
        kind,
        own_bytes: 16 * GIB,
    }
}

/// A computer with one discrete card and one integrated, `free` bytes of ordinary memory to
/// promise, and the discrete card's budget and unheld memory as given.
fn room(free: u64, budget: Option<u64>, unheld: Option<u64>) -> Room {
    Room {
        cards: vec![
            card(1, "Big card", CardKind::Discrete),
            card(2, "Shared", CardKind::Integrated),
        ],
        budgets: vec![budget, None],
        unheld: vec![unheld, None],
        drawing_on: None,
        held: 0,
        free_memory: free,
    }
}

/// Figures for the processor and the big card.
fn measured() -> Measured {
    Measured {
        runtime_sum: "a".repeat(64),
        processor: Some(2 * GIB),
        cards: vec![CardFigures {
            name: "Big card".to_owned(),
            card_bytes: 4 * GIB,
            ordinary_bytes: GIB,
        }],
    }
}

#[test]
fn measured_figures_round_trip() {
    let folder = tempfile::tempdir().expect("a folder");
    assert_eq!(read_measured(folder.path()), None);
    write_measured(folder.path(), &measured()).expect("written");
    assert!(folder.path().join(MEASURED).is_file());
    assert_eq!(read_measured(folder.path()), Some(measured()));
    std::fs::write(folder.path().join(MEASURED), b"not figures").expect("written");
    assert_eq!(read_measured(folder.path()), None);
}

#[test]
fn a_card_fits_only_with_a_quarter_to_spare_within_its_budget_and_what_is_free() {
    let own = 0;
    let fits = places(
        &measured(),
        own,
        &room(64 * GIB, Some(5 * GIB), Some(8 * GIB)),
        RunsOn::Automatic,
    );
    assert_eq!(
        fits[0],
        Fits {
            device: Device::Card(CardId(1)),
            cap: GIB + GIB / 4
        }
    );
    // 4 GiB and a quarter is more than a budget of 4.9 GiB.
    let tight = room(64 * GIB, Some(49 * GIB / 10), Some(8 * GIB));
    assert_eq!(
        places(&measured(), own, &tight, RunsOn::Automatic)[0].device,
        Device::Processor
    );
    // The budget holds it, but another program holds all but 3 GiB of the card.
    let busy = room(64 * GIB, Some(8 * GIB), Some(3 * GIB));
    assert_eq!(
        places(&measured(), own, &busy, RunsOn::Automatic)[0].device,
        Device::Processor
    );
}

#[test]
fn the_processor_fits_when_the_system_can_promise_the_cap_and_voxas_own_part() {
    let own = GIB / 2;
    let enough = room(2 * GIB + GIB / 2 + GIB / 2, None, None);
    let fits = places(&measured(), own, &enough, RunsOn::Processor);
    assert_eq!(
        fits,
        [Fits {
            device: Device::Processor,
            cap: 2 * GIB + GIB / 2
        }]
    );
    let short = room(3 * GIB - 1, None, None);
    assert_eq!(places(&measured(), own, &short, RunsOn::Processor), []);
}

#[test]
fn automatic_tries_discrete_cards_then_the_processor() {
    let fits = places(
        &measured(),
        0,
        &room(64 * GIB, Some(8 * GIB), None),
        RunsOn::Automatic,
    );
    let devices: Vec<Device> = fits.iter().map(|fits| fits.device).collect();
    assert_eq!(devices, [Device::Card(CardId(1)), Device::Processor]);
}

#[test]
fn a_card_not_measured_is_offered_to_be_measured_first() {
    let mut figures = measured();
    figures.cards.clear();
    figures.processor = None;
    let fits = places(
        &figures,
        0,
        &room(64 * GIB, Some(8 * GIB), None),
        RunsOn::Automatic,
    );
    assert_eq!(
        fits,
        [
            Fits {
                device: Device::Card(CardId(1)),
                cap: 0
            },
            Fits {
                device: Device::Processor,
                cap: 0
            }
        ]
    );
    let asked = places(
        &figures,
        0,
        &room(64 * GIB, None, None),
        RunsOn::Card(CardId(2)),
    );
    assert_eq!(
        asked,
        [Fits {
            device: Device::Card(CardId(2)),
            cap: 0
        }]
    );
}

#[test]
fn voxas_own_part_is_a_block_the_labels_and_the_statistics() {
    assert_eq!(own_bytes([10, 10, 10], 1000), 4000 + 2000 + (2 << 20));
}

/// A stand-in for the runtime that opens a network answering two channels.
struct Halves;

impl Opener for Halves {
    fn load(&self, _runtime: &std::path::Path) -> Result<(), voxa_models::RuntimeError> {
        Ok(())
    }

    fn open(
        &self,
        _job: &OpenJob,
        _stop: &voxa_models::Stop,
    ) -> Result<Box<dyn voxa_models::Network>, voxa_models::RunError> {
        Ok(Box::new(voxa_models::testkit::Canned::new(|inputs| {
            let mut shape = inputs[0].shape.clone();
            shape[1] = 2;
            let count = shape.iter().product();
            Ok(vec![
                voxa_models::Tensor::f32(shape, vec![0.0; count]).expect("whole"),
            ])
        })))
    }

    fn card_in_use(&self, _place: &Where) -> Option<u64> {
        Some(77)
    }
}

/// A helper served in this process, whose peak is said to be 99 bytes.
struct InProcess {
    pipes: Option<(PipeReader, PipeWriter)>,
}

impl HelperPipes for InProcess {
    fn client(&mut self) -> Option<Client<Box<dyn io::Read + Send>, Box<dyn io::Write + Send>>> {
        let (read, write) = self.pipes.take()?;
        Some(Client::new(Box::new(read), Box::new(write)))
    }

    fn stop(&mut self) {}

    fn peak_memory(&self) -> Option<u64> {
        Some(99)
    }
}

/// Starts in-process helpers, and remembers the caps they were given.
#[derive(Default)]
struct Starter {
    caps: Arc<Mutex<Vec<Option<u64>>>>,
}

impl HelperStarter for Starter {
    fn start(&self, cap: Option<u64>) -> io::Result<Box<dyn HelperPipes>> {
        self.caps.lock().expect("caps").push(cap);
        let (mut from_voxa, to_helper) = io::pipe()?;
        let (from_helper, mut to_voxa) = io::pipe()?;
        thread::spawn(move || {
            let _ = serve(&mut from_voxa, &mut to_voxa, &Halves);
        });
        Ok(Box::new(InProcess {
            pipes: Some((from_helper, to_helper)),
        }))
    }
}

#[test]
fn measuring_runs_one_made_up_block_and_reports_the_jobs_peak() {
    let folder = tempfile::tempdir().expect("a folder");
    let network = folder.path().join("net.onnx");
    std::fs::write(&network, b"net").expect("written");
    let job = OpenJob {
        runtime: folder.path().to_owned(),
        network,
        sha256: crate::testkit::sum(b"net"),
        device: Where::Processor,
        free: Vec::new(),
        output: None,
        layout: crate::Layout::ChannelsFirst,
        block: [4, 4, 4],
        finish: crate::LabelOutput {
            gives: crate::Gives::Scores {
                channels: vec![0, 1],
            },
            from: None,
            keep_largest_piece: false,
            names: vec![crate::Name {
                id: 1,
                name: "Part".to_owned(),
                side: crate::Side::Midline,
                group: String::new(),
                colour: None,
            }],
        },
    };
    let starter = Starter::default();
    let figures = measure(&starter, &job, Some(8 * GIB)).expect("measured");
    assert_eq!(figures, (99, Some(77)));
    assert_eq!(*starter.caps.lock().expect("caps"), [Some(8 * GIB)]);
}
