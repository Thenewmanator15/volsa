//! Stand-ins for the networks and for what the computer has: a made-up volume, a made-up model
//! folder and made-up cards. No network file is read or run. The lab that follows the worker
//! with them is in `labelling_lab.rs`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tempfile::TempDir;
use voxa_core::{Affine, Volume};
use voxa_models::testkit::{Canned, FolderOptions, model_folder};
use voxa_models::{
    Card, CardId, CardKind, Device, FreeMemory, Model, Network, Numbers, Role, RunError,
    RuntimeError, Stop, Tensor, Wanted,
};

use super::Labeller;

pub(super) const GIB: u64 = 1 << 30;
/// The made-up day every result is made on.
pub(super) const WHEN: &str = "2099-01-02";
/// The robust networks, without the cortex.
pub(super) const ROBUST: Wanted = Wanted {
    robust: true,
    cortex: false,
};
/// Long enough for any worker on any machine; a test that waits this long has failed.
pub(super) const PATIENCE: Duration = Duration::from_secs(120);
/// The structures a made-up model folder lists, the first so many of them in the first of
/// the two tensors that give their chances, and its score groups.
const STRUCTURES: usize = 33;
const FIRST: usize = 17;
const GROUPS: usize = 8;

/// The discrete card of these tests.
pub(super) fn card_a() -> Card {
    Card {
        id: CardId(2),
        index: 0,
        name: "Made-up card A".to_owned(),
        kind: CardKind::Discrete,
        own_bytes: 8 * GIB,
    }
}

/// Stand-ins for the runtime and for what the computer has. Every network opens and answers
/// unless the test says otherwise. One asked for once the run's stop is set is refused, as
/// the runtime refuses it; one already running runs to its end, as on a card.
pub(super) struct StandIn {
    pub(super) cards: Vec<Card>,
    /// What the system lets this program use on each card, beside `cards`.
    pub(super) budgets: Vec<Option<u64>>,
    /// What no program holds of each card, beside `cards`: none known unless a test says.
    pub(super) unheld: Vec<Option<u64>>,
    pub(super) free: FreeMemory,
    /// What a network asked for on a device says in place of opening, each once, in order.
    refusals: Mutex<Vec<(Device, RunError)>>,
    /// Every network asked for, and where, in order.
    opened: Mutex<Vec<(Role, Device)>>,
    /// The name of the thread each network was asked for on, in the same order.
    threads: Mutex<Vec<String>>,
    /// The name of the thread the runtime was asked for on, each time it was.
    looked_on: Mutex<Vec<String>>,
    /// For the first network opened: told when it starts to run, and then waited on.
    gate: Mutex<Option<Gate>>,
    /// The next network asked for panics.
    pub(super) panics: AtomicBool,
    /// How many networks are open now, and the most there have been together.
    open: Arc<AtomicUsize>,
    most: AtomicUsize,
}

/// Where a network running says it has started, and what it then waits for.
type Gate = (Sender<()>, Receiver<()>);

impl StandIn {
    /// One discrete card, with this budget, and this much ordinary memory to spare.
    pub(super) fn with(budget: u64, free: u64) -> Self {
        Self::of(vec![card_a()], vec![Some(budget)], free)
    }

    pub(super) fn of(cards: Vec<Card>, budgets: Vec<Option<u64>>, free: u64) -> Self {
        Self {
            cards,
            budgets,
            unheld: Vec::new(),
            free: FreeMemory::Promised(free),
            refusals: Mutex::new(Vec::new()),
            opened: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
            looked_on: Mutex::new(Vec::new()),
            gate: Mutex::new(None),
            panics: AtomicBool::new(false),
            open: Arc::new(AtomicUsize::new(0)),
            most: AtomicUsize::new(0),
        }
    }

    /// Room for everything on the card.
    pub(super) fn roomy() -> Self {
        Self::with(16 * GIB, 64 * GIB)
    }

    /// What no program holds of each card, beside `cards`.
    pub(super) fn unheld(self, unheld: Vec<Option<u64>>) -> Self {
        Self { unheld, ..self }
    }

    /// A network asked for on `device` says `why` in place of opening, once.
    pub(super) fn refusing(self, device: Device, why: RunError) -> Self {
        held(&self.refusals).push((device, why));
        self
    }

    /// The first network opened says when it starts to run, and then runs on only once what
    /// is returned is told, or dropped.
    pub(super) fn held_while_running(self) -> (Self, Receiver<()>, Sender<()>) {
        let ((started, running), (go, gate)) = (channel(), channel());
        *held(&self.gate) = Some((started, gate));
        (self, running, go)
    }

    pub(super) fn opened(&self) -> Vec<(Role, Device)> {
        held(&self.opened).clone()
    }

    /// The name of the thread each network was asked for on.
    pub(super) fn threads(&self) -> Vec<String> {
        held(&self.threads).clone()
    }

    /// The name of the thread the runtime was asked for on, each time.
    pub(super) fn looked_on(&self) -> Vec<String> {
        held(&self.looked_on).clone()
    }

    /// The most networks that were open together, at any time.
    pub(super) fn most_open(&self) -> usize {
        self.most.load(Ordering::SeqCst)
    }
}

/// A network that counts itself among those open for as long as it lives.
struct Counted {
    inside: Box<dyn Network>,
    open: Arc<AtomicUsize>,
}

impl Network for Counted {
    fn run(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError> {
        self.inside.run(inputs)
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.open.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Labeller for StandIn {
    fn load_runtime(&self, _folder: &Path) -> Result<(), RuntimeError> {
        let thread = std::thread::current().name().unwrap_or_default().to_owned();
        held(&self.looked_on).push(thread);
        Ok(())
    }

    fn open(
        &self,
        _model: &Model,
        role: Role,
        device: Device,
        sizes: [usize; 3],
        stop: &Stop,
    ) -> Result<Box<dyn Network>, RunError> {
        held(&self.opened).push((role, device));
        let thread = std::thread::current().name().unwrap_or_default().to_owned();
        held(&self.threads).push(thread);
        if stop.is_set() {
            return Err(RunError::Stopped);
        }
        assert!(
            !self.panics.swap(false, Ordering::SeqCst),
            "a made-up failure"
        );
        let mut refusals = held(&self.refusals);
        if let Some(at) = refusals.iter().position(|(on, _)| *on == device) {
            return Err(refusals.remove(at).1);
        }
        let inside = network(role, sizes, held(&self.gate).take());
        let now = self.open.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        let open = Arc::clone(&self.open);
        Ok(Box::new(Counted { inside, open }))
    }

    fn cards(&self) -> Vec<Card> {
        self.cards.clone()
    }

    fn budget(&self, card: CardId) -> Option<u64> {
        let place = self.cards.iter().position(|listed| listed.id == card)?;
        self.budgets.get(place).copied().flatten()
    }

    fn unheld(&self, card: CardId) -> Option<u64> {
        let place = self.cards.iter().position(|listed| listed.id == card)?;
        self.unheld.get(place).copied().flatten()
    }

    fn free_memory(&self) -> FreeMemory {
        self.free
    }

    fn today(&self) -> String {
        WHEN.to_owned()
    }
}

pub(super) fn held<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A stand-in for the network of `role`, answering as that network does for a grid of
/// `sizes`. Where it gives the chances of the structures, it is sure of the first, second or
/// third structure where the image is bright, by how bright, and of nothing where it is dark.
/// With a gate, it says when it starts to run and waits to be let go on.
fn network(role: Role, sizes: [usize; 3], mut gate: Option<Gate>) -> Box<dyn Network> {
    let [x, y, z] = sizes;
    let answer = move |inputs: &[Tensor]| {
        if let Some((started, go)) = gate.take() {
            started.send(()).expect("the test is waiting");
            let _ = go.recv();
        }
        let map = || Tensor::u8(vec![1, x, y, z], vec![1; x * y * z]).expect("a map");
        Ok(match role {
            Role::Robust1 | Role::Robust2 | Role::Cortex => vec![map()],
            Role::Standard | Role::Robust3 => chances(&inputs[0], sizes),
            Role::Scores => {
                let shape = vec![1, GROUPS + 1];
                vec![Tensor::f32(shape, vec![0.9; GROUPS + 1]).expect("scores")]
            }
        })
    };
    Box::new(Canned::new(answer))
}

fn chances(image: &Tensor, [x, y, z]: [usize; 3]) -> Vec<Tensor> {
    let Numbers::F32(values) = &image.numbers else {
        panic!("an image is numbers")
    };
    let mut first = vec![0.0; values.len() * FIRST];
    for (voxel, &value) in values.iter().enumerate() {
        let place = match value {
            dark if dark <= 0.0 => 0,
            dim if dim < 1.0 / 3.0 => 1,
            mid if mid < 2.0 / 3.0 => 2,
            _ => 3,
        };
        first[voxel * FIRST + place] = 1.0;
    }
    let rest = vec![0.0; values.len() * (STRUCTURES - FIRST)];
    vec![
        Tensor::f32(vec![1, x, y, z, FIRST], first).expect("the first tensor"),
        Tensor::f32(vec![1, x, y, z, STRUCTURES - FIRST], rest).expect("the second"),
    ]
}

/// A made-up head: 20 by 12 by 16 voxels 1 mm apart, brighter along every axis.
pub(super) fn head() -> Volume {
    let placement = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [-9.75, 6.25, 8.0],
    );
    let mut values = Vec::new();
    for slice in 0..16_u32 {
        for row in 0..12_u32 {
            values.extend((0..20_u32).map(|col| (10 + col + 8 * row + 64 * slice) as f32));
        }
    }
    Volume::new([20, 12, 16], values, placement).expect("a volume")
}

/// The made-up head in a longer grid: 144 by 12 by 16 voxels 1 mm apart, bright in the 20
/// columns in the middle and dark elsewhere. Cut to the head, its image is 128 voxels a side;
/// whole, it is 160 by 128 by 128.
pub(super) fn long_head() -> Volume {
    let placement = head().voxel_to_patient();
    let mut values = Vec::new();
    for slice in 0..16_u32 {
        for row in 0..12_u32 {
            let value = |col: u32| (10 + col + 8 * row + 64 * slice) as f32;
            let bright = |col: u32| {
                if (62..82).contains(&col) {
                    value(col)
                } else {
                    0.0
                }
            };
            values.extend((0..144_u32).map(bright));
        }
    }
    Volume::new([144, 12, 16], values, placement).expect("a volume")
}

/// A made-up model folder that reads as it should.
pub(super) fn models() -> TempDir {
    model_folder(&FolderOptions {
        box_side: 32,
        ..FolderOptions::default()
    })
}

/// A whole run holds a few hundred megabytes: the tests that run one to its end take turns.
pub(super) fn one_at_a_time() -> MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    held(&ONE)
}
