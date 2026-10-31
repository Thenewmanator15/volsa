//! Times the window's own work: the bar across the top, the line along the bottom, the rail of
//! scans and the tab panel, laid out with no screen behind them and no pictures, for a made-up
//! study of 500 series, at 1400 by 900 points. It prints the median time of one frame.
//!
//! ```text
//! cargo run --release -p voxa-app --example measure_window
//! ```
//!
//! The design (spec 7.17) holds this to what 0.6.0's bars and list took on the same computer,
//! so the same example is run on 0.6.0, whose window had no rail and panel but a bar and a
//! list of series. 0.6.0 has no `BareWindow`: the run there is of a temporary copy of this
//! example adapted to lay out 0.6.0's bar and list, which was not kept (see the usability file,
//! `docs/quality/usability.md`, section 6). Run it with nothing else heavy going on: the numbers
//! are of one computer at one time, and are worth only beside each other.
//!
//! Every series is one made-up slice with a made-up description; nothing in the study is from
//! anyone's scan.

use std::time::{Duration, Instant};

use voxa_app::{Action, AppState, BareWindow, reduce};
use voxa_dicom::index;
use voxa_dicom::testkit::{MemorySource, SliceOptions, classic_slice};

/// How many series the made-up study holds.
const SERIES: usize = 500;
/// How many frames are timed.
const FRAMES: usize = 200;
/// How many frames are drawn first and not timed: the first frames build the fonts and find
/// the panels' sizes.
const WARM_UP: usize = 10;
/// The window, in points.
const WINDOW: [f32; 2] = [1400.0, 900.0];
/// The descriptions the series are given in turn.
const DESCRIPTIONS: [&str; 5] = [
    "Made-up T1 sagittal",
    "Made-up T2 axial",
    "Made-up FLAIR axial",
    "Made-up localiser",
    "Made-up diffusion",
];

/// One made-up series of a single slice, number `number`.
fn series(number: usize) -> Vec<(String, Vec<u8>)> {
    let description = DESCRIPTIONS
        .get(number % DESCRIPTIONS.len())
        .unwrap_or(&"Made-up");
    let options = SliceOptions {
        series_uid: format!("1.2.3.{number}"),
        series_number: i32::try_from(number).unwrap_or(i32::MAX),
        series_description: (*description).to_owned(),
        instance_number: 1,
        rows: 4,
        cols: 4,
        position: Some([0.0, 0.0, 0.0]),
        ..SliceOptions::default()
    };
    vec![(format!("S{number}/I0001.dcm"), classic_slice(&options))]
}

/// A state that has read the made-up study.
fn state() -> AppState {
    let files: Vec<(String, Vec<u8>)> = (1..=SERIES).flat_map(series).collect();
    let listed = index(&mut MemorySource::new(files), &mut |_| {}).expect("the study is indexed");
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(listed));
    state
}

fn main() {
    let state = state();
    let listed = state.index.as_ref().map_or(0, |index| index.series.len());
    let mut window = BareWindow::new(state, WINDOW);
    for _ in 0..WARM_UP {
        let _ = window.lay_out();
    }
    let mut times: Vec<Duration> = (0..FRAMES)
        .map(|_| {
            let begun = Instant::now();
            let _ = window.lay_out();
            begun.elapsed()
        })
        .collect();
    times.sort();
    let at = |place: usize| times.get(place).copied().unwrap_or_default();
    println!(
        "{listed} series, {FRAMES} frames at {} by {} points: median {:.3} ms (fastest {:.3} ms, slowest {:.3} ms)",
        WINDOW[0],
        WINDOW[1],
        at(FRAMES / 2).as_secs_f64() * 1000.0,
        at(0).as_secs_f64() * 1000.0,
        at(FRAMES - 1).as_secs_f64() * 1000.0,
    );
}
