//! A helper started in a job object: it holds memory up to its cap and is refused past it,
//! the job tells how much it held at most, and stopping or dropping it ends it at once.
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::ChildStdout;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use volsa_models::{Helper, start_helper};

/// A MiB.
const MIB: u64 = 1 << 20;

/// The test program that asks for memory.
fn eater() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_volsa-test-eater"))
}

/// Asks the eater for so many MiB, and gives its answer.
fn ask(helper: &mut Helper, mib: u64) -> (String, BufReader<ChildStdout>) {
    let (mut input, output) = helper.take_pipes().expect("the pipes");
    writeln!(input, "{mib}").expect("asked");
    input.flush().expect("sent");
    let mut output = BufReader::new(output);
    let mut answer = String::new();
    output.read_line(&mut answer).expect("an answer");
    // The eater waits for its input to close before it ends: keep it open until the end.
    std::mem::forget(input);
    (answer.trim().to_owned(), output)
}

#[test]
fn a_helper_under_its_cap_holds_what_it_asks() {
    let mut helper = start_helper(eater(), &[], Some(256 * MIB)).expect("started");
    let (answer, _output) = ask(&mut helper, 64);
    assert_eq!(answer, "held");
    let peak = helper.peak_memory().expect("the job's peak");
    assert!(peak >= 64 * MIB, "peak {peak}");
    assert!(peak <= 256 * MIB, "peak {peak}");
}

#[test]
fn a_helper_past_its_cap_is_refused() {
    let mut helper = start_helper(eater(), &[], Some(256 * MIB)).expect("started");
    let (answer, _output) = ask(&mut helper, 512);
    assert_eq!(answer, "refused");
}

#[test]
fn stop_ends_a_helper_at_once() {
    let mut helper = start_helper(eater(), &[], None).expect("started");
    let started = Instant::now();
    helper.stop();
    let status = helper.wait().expect("ended");
    assert!(!status.success());
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn dropping_the_helper_ends_its_process() {
    let mut helper = start_helper(eater(), &[], None).expect("started");
    let (input, output) = helper.take_pipes().expect("the pipes");
    drop(helper);
    let (said, heard) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let read = BufReader::new(output).read_line(&mut line);
        let _ = said.send(read.map(|count| count == 0));
    });
    let ended = heard
        .recv_timeout(Duration::from_secs(5))
        .expect("the pipe closed");
    assert!(ended.expect("read to its end"));
    drop(input);
}
