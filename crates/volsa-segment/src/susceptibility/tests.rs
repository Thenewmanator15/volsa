//! The susceptibility map: its transforms and grid, how it reads phase, and a made-up head
//! holding a small ball drawn into the field, whose phase wraps, given back as the ball.

use rustfft::num_complex::Complex32;
use volsa_core::{Affine, Volume};

use super::fft::{Way, frequency, smooth_length, transform};
use super::*;

const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;
const FACTS: PhaseFacts = PhaseFacts {
    echo_ms: Some(20.0),
    field_t: Some(3.0),
};
/// The made-up head: 64 voxels a side, 2 mm apart.
const SIDE: usize = 64;
const MM: f64 = 2.0;
/// The ball's susceptibility, in parts per billion.
const BALL_PPB: f64 = 300.0;

fn axial(mm: f64) -> Affine<volsa_core::Voxel, volsa_core::Patient> {
    Affine::from_columns([mm, 0.0, 0.0], [0.0, mm, 0.0], [0.0, 0.0, mm], [0.0; 3])
}

#[test]
fn a_transform_and_its_undoing_give_back_what_was_given() {
    let sides = [6, 5, 4];
    let given: Vec<Complex32> = (0..120)
        .map(|at| Complex32::new(at as f32, -(at % 7) as f32))
        .collect();
    let mut values = given.clone();
    transform(&mut values, sides, Way::Forward).expect("room");
    assert!((values[0].re - given.iter().map(|v| v.re).sum::<f32>()).abs() < 1e-2);
    transform(&mut values, sides, Way::Back).expect("room");
    for (back, was) in values.iter().zip(&given) {
        assert!((back - was).norm() < 1e-3, "{back} against {was}");
    }
}

#[test]
fn lengths_and_frequencies() {
    assert_eq!(smooth_length(97), 100);
    assert_eq!(smooth_length(128), 128);
    assert_eq!(smooth_length(7), 8);
    assert!((frequency(1, 10, 2.0) - 0.05).abs() < 1e-12);
    assert!((frequency(9, 10, 2.0) + 0.05).abs() < 1e-12);
}

#[test]
fn phase_is_read_as_radians_thousandths_or_twelve_bit_turns() {
    let spread = |top: f32| (0..2000).map(move |at| (at as f32 / 1000.0 - 1.0) * top);
    let radians = to_radians(&spread(3.1).collect::<Vec<_>>()).expect("radians");
    assert!((radians(1.0) - 1.0).abs() < 1e-6);
    let thousandths = to_radians(&spread(3100.0).collect::<Vec<_>>()).expect("thousandths");
    assert!((thousandths(1000.0) - 1.0).abs() < 1e-6);
    let twelve_bit: Vec<f32> = (0..4096).map(|at| at as f32).collect();
    let turns = to_radians(&twelve_bit).expect("twelve-bit");
    assert!(turns(2048.0).abs() < 1e-6 && (turns(4096.0) - std::f32::consts::PI).abs() < 1e-5);
    assert_eq!(
        to_radians(&spread(40_000.0).collect::<Vec<_>>()).err(),
        Some(SusceptibilityError::NotPhase)
    );
    assert_eq!(
        to_radians(&[0.0; 100]).err(),
        Some(SusceptibilityError::NotPhase)
    );
}

#[test]
fn the_main_field_runs_head_to_foot_along_whichever_side_lies_so() {
    let axial = main_field_along(
        [[0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 1.5]],
        [0.5, 0.5, 1.5],
    );
    assert!((axial[2] - 1.0).abs() < 1e-12);
    // A sagittal stack: its rows run head to foot.
    let sagittal = main_field_along(
        [[0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]],
        [1.0; 3],
    );
    assert!((sagittal[1] + 1.0).abs() < 1e-12, "{sagittal:?}");
}

/// The made-up head's phase, in radians: a head 40 mm across with a ball of [`BALL_PPB`]
/// 8 mm across at its middle, the field the ball makes, a field from outside that grows
/// steadily across the head, and 0 outside the head, as a scanner blanks it.
fn made_up_phase(sign: f64) -> Volume {
    let centre = (SIDE / 2) as f64;
    let at = |place: usize| {
        let (x, y, z) = (place % SIDE, place / SIDE % SIDE, place / (SIDE * SIDE));
        [x, y, z].map(|part| (part as f64 - centre) * MM)
    };
    let radius = |p: [f64; 3]| p.iter().map(|part| part * part).sum::<f64>().sqrt();
    let count = SIDE * SIDE * SIDE;
    let mut field: Vec<Complex32> = (0..count)
        .map(|place| Complex32::from(f32::from(u8::from(radius(at(place)) <= 8.0))))
        .collect();
    let sides = [SIDE; 3];
    transform(&mut field, sides, Way::Forward).expect("room");
    for (place, value) in field.iter_mut().enumerate() {
        let k = [place % SIDE, place / SIDE % SIDE, place / (SIDE * SIDE)]
            .map(|part| frequency(part, SIDE, MM));
        let squared: f64 = k.iter().map(|part| part * part).sum();
        let kernel = if squared > 0.0 {
            1.0 / 3.0 - k[2] * k[2] / squared
        } else {
            0.0
        };
        *value *= kernel as f32;
    }
    transform(&mut field, sides, Way::Back).expect("room");
    let per_ppb = GAMMA * 3.0 * 0.02 * 1e-9;
    let values = field
        .iter()
        .enumerate()
        .map(|(place, value)| {
            let p = at(place);
            if radius(p) > 40.0 {
                return 0.0;
            }
            let angle = sign * (f64::from(value.re) * BALL_PPB * per_ppb + 0.03 * p[0]);
            (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) as f32
                - std::f32::consts::PI
        })
        .collect();
    Volume::new([SIDE as u32; 3], values, axial(MM)).expect("a volume")
}

/// The mean of `map` within `within` millimetres of the middle, and beyond `beyond` but
/// inside the head.
fn means(map: &Volume, within: f64, beyond: f64) -> (f64, f64) {
    let centre = (SIDE / 2) as f64;
    let (mut inner, mut outer) = ((0.0_f64, 0.0_f64), (0.0_f64, 0.0_f64));
    for (place, value) in map.values().iter().enumerate() {
        let p = [place % SIDE, place / SIDE % SIDE, place / (SIDE * SIDE)]
            .map(|part| (part as f64 - centre) * MM);
        let r = p.iter().map(|part| part * part).sum::<f64>().sqrt();
        let into = if r <= within {
            &mut inner
        } else if r >= beyond && r <= 30.0 {
            &mut outer
        } else {
            continue;
        };
        into.0 += f64::from(*value);
        into.1 += 1.0;
    }
    (inner.0 / inner.1.max(1.0), outer.0 / outer.1.max(1.0))
}

#[test]
fn a_ball_drawn_into_the_field_is_given_back_whichever_way_the_phase_turns() {
    for sign in [1.0, -1.0] {
        let map = susceptibility(&made_up_phase(sign), FACTS, NEVER).expect("a map");
        assert_eq!(map.dims(), [SIDE as u32; 3]);
        let (ball, round) = means(&map, 5.0, 14.0);
        assert!(
            ball > 0.4 * BALL_PPB && ball < 1.6 * BALL_PPB,
            "the ball: {ball:.0} ppb against {BALL_PPB}"
        );
        assert!(round.abs() < 0.1 * BALL_PPB, "round it: {round:.0} ppb");
        assert_eq!(map.values()[0], 0.0, "nothing outside the head");
    }
}

#[test]
fn what_cannot_be_mapped_is_refused_in_a_sentence() {
    let phase = made_up_phase(1.0);
    let no_facts = PhaseFacts {
        echo_ms: None,
        field_t: Some(3.0),
    };
    assert_eq!(
        susceptibility(&phase, no_facts, NEVER).err(),
        Some(SusceptibilityError::NoFacts)
    );
    assert_eq!(
        susceptibility(&phase, FACTS, &|| true).err(),
        Some(SusceptibilityError::Stopped)
    );
    let thin = Volume::new([64, 64, 4], vec![0.5; 64 * 64 * 4], axial(1.0)).expect("a volume");
    assert_eq!(
        susceptibility(&thin, FACTS, NEVER).err(),
        Some(SusceptibilityError::TooThin)
    );
    // Noise from one voxel to the next: no tissue anywhere.
    let noise: Vec<f32> = (0..32 * 32 * 32_u32)
        .map(|at| {
            ((at.wrapping_mul(2_654_435_761) >> 7) % 6283) as f32 / 1000.0 - std::f32::consts::PI
        })
        .collect();
    let noise = Volume::new([32; 3], noise, axial(2.0)).expect("a volume");
    assert_eq!(
        susceptibility(&noise, FACTS, NEVER).err(),
        Some(SusceptibilityError::NoHead)
    );
    for error in [SusceptibilityError::NotPhase, SusceptibilityError::NoHead] {
        assert!(error.to_string().contains("phase series"), "{error}");
    }
}
