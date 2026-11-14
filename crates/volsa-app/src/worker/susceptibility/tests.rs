//! A susceptibility map worked out on a worker from a made-up phase volume: kept in the phase's
//! place and said to be ready as a volume is, never taken for the series' own values, and
//! given up when asked; and what is said when it cannot be made.

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use volsa_core::{Affine, Grid as _, Volume};
use volsa_segment::PhaseFacts;

use crate::action::{Action, Effect};
use crate::cache::FrameCache;
use crate::testing::{THICK_SCAN, held_of, id};
use crate::volume_store::VolumeStore;
use crate::worker::Workers;

/// How long a test waits for the worker: the map takes a few seconds unoptimised.
const PATIENCE: Duration = Duration::from_mins(5);
/// The made-up phase: this many voxels a side, this many millimetres apart.
const SIDE: usize = 48;
const MM: f64 = 2.5;
const FACTS: PhaseFacts = PhaseFacts {
    echo_ms: Some(20.0),
    field_t: Some(3.0),
};

/// Workers with `phase` kept as the 30-slice series' volume, the store they keep it in, and
/// what they say.
fn workers_with(phase: &Volume) -> (Workers, Arc<Mutex<VolumeStore>>, Receiver<Action>) {
    let (sender, reports) = channel();
    let cache = Arc::new(Mutex::new(FrameCache::new(1 << 20)));
    let volumes = Arc::new(Mutex::new(VolumeStore::default()));
    let workers = Workers::new(sender, cache, Arc::clone(&volumes), None, None);
    held(&volumes).put(id(THICK_SCAN), Arc::new(held_of(phase)));
    (workers, volumes, reports)
}

fn held(volumes: &Mutex<VolumeStore>) -> std::sync::MutexGuard<'_, VolumeStore> {
    volumes.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A made-up head's phase, in radians: a ball 90 mm across whose phase changes smoothly, a
/// little more towards its edge and steadily from one side to the other, wrapped to a turn;
/// 0 outside, as a scanner blanks it.
fn smooth_phase() -> Volume {
    let centre = (SIDE / 2) as f64;
    let values = (0..SIDE * SIDE * SIDE)
        .map(|place| {
            let at = [place % SIDE, place / SIDE % SIDE, place / (SIDE * SIDE)]
                .map(|part| (part as f64 - centre) * MM);
            let squared: f64 = at.iter().map(|part| part * part).sum();
            if squared > 45.0 * 45.0 {
                return 0.0;
            }
            let angle = 0.02 * at[0] + 0.0004 * squared;
            ((angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI) as f32
        })
        .collect();
    let side = u32::try_from(SIDE).expect("small");
    let placement = Affine::from_columns([MM, 0.0, 0.0], [0.0, MM, 0.0], [0.0, 0.0, MM], [0.0; 3]);
    Volume::new([side; 3], values, placement).expect("adds up")
}

fn make(workers: &Workers) {
    let effect = Effect::MakeSusceptibility {
        series: id(THICK_SCAN),
        facts: FACTS,
    };
    workers.run(effect, None);
}

/// The first thing the workers say about a volume.
fn said_of_a_volume(reports: &Receiver<Action>) -> Action {
    loop {
        let report = reports.recv_timeout(PATIENCE).expect("the worker says");
        if matches!(
            report,
            Action::VolumeReady { .. } | Action::VolumeFailed { .. }
        ) {
            return report;
        }
    }
}

#[test]
fn a_map_is_kept_in_place_of_the_phase_and_said_to_be_ready_as_a_volume_is() {
    let phase = smooth_phase();
    let (workers, volumes, reports) = workers_with(&phase);
    make(&workers);
    let report = said_of_a_volume(&reports);
    assert!(
        matches!(&report, Action::VolumeReady { series, .. } if *series == id(THICK_SCAN)),
        "{report:?}"
    );
    let store = held(&volumes);
    assert!(
        store.own(&id(THICK_SCAN)).is_none(),
        "the map is never taken for the scan's own values"
    );
    let map = store
        .get(&id(THICK_SCAN))
        .expect("the map is what is drawn");
    drop(store);
    assert_eq!(map.dims(), phase.dims(), "on the phase's own grid");
    let values = map.to_volume(0..map.dims()[2]).expect("room");
    assert!(values.values().iter().all(|value| value.is_finite()));
    assert_eq!(map.value(0, 0, 0), Some(0.0), "0 outside the head");
    assert_ne!(values.values(), phase.values(), "the map, not the phase");
    workers.run(Effect::DropSusceptibility, None);
    assert!(held(&volumes).get(&id(THICK_SCAN)).is_none(), "given up");
}

#[test]
fn what_is_not_phase_is_refused_in_a_sentence() {
    let side = u32::try_from(SIDE).expect("small");
    let not_phase = Volume::new(
        [side; 3],
        vec![40_000.0; SIDE * SIDE * SIDE],
        Affine::identity(),
    )
    .expect("adds up");
    let (workers, volumes, reports) = workers_with(&not_phase);
    make(&workers);
    let why = volsa_segment::SusceptibilityError::NotPhase.to_string();
    let failed = Action::VolumeFailed {
        series: id(THICK_SCAN),
        reason: why,
    };
    assert_eq!(said_of_a_volume(&reports), failed);
    assert!(
        held(&volumes).own(&id(THICK_SCAN)).is_some(),
        "the phase stays"
    );
}

#[test]
fn with_no_phase_volume_kept_it_says_so() {
    let (workers, volumes, reports) = workers_with(&smooth_phase());
    held(&volumes).clear();
    make(&workers);
    let report = said_of_a_volume(&reports);
    let said = matches!(&report, Action::VolumeFailed { reason, .. } if reason.contains("no longer there"));
    assert!(said, "{report:?}");
}
