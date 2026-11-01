//! Project tasks for Volsa.
//!
//! `cargo xtask check` runs every quality gate in order: formatting, lints, file length, the
//! third-party notices, tests, licence and advisory checks, documentation. Every test program
//! runs even when one fails; any other failing step stops it there.
//! `cargo xtask coverage` measures line coverage of the crates that need no GPU and fails
//! below the floor. The few files that cannot run without a graphics card or the networks'
//! runtime are left out of the count.
//! `cargo xtask check-models` holds the labelling of a scan against `SynthSeg`'s own answers,
//! on a computer that has the networks: it runs the tests the other two tasks leave out,
//! which need a model folder and the networks' runtime. Without them it does nothing.
//! `cargo xtask notices` writes `THIRD-PARTY-NOTICES.txt`, the licences of everything built
//! into the program, and `cargo xtask check` fails when the committed file is out of date.
//! `cargo xtask dist` builds the program for 64-bit Windows with Microsoft's C runtime inside
//! it and makes the zip a release offers, in `target/dist`; `--runtime FOLDER` puts ONNX
//! Runtime from that folder in it too, with its licence and notices, and `--local` lets a
//! folder without them make a package that is not for release.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod dist;
mod notices;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

/// Longest a source file may be before it should be split.
const MAX_FILE_LINES: usize = 400;
/// Lowest acceptable line coverage, in percent, for each crate in [`COVERED_CRATES`].
const COVERAGE_FLOOR: &str = "80";
/// Crates whose tests need no GPU and so carry the coverage floor. See [`NOT_COUNTED`].
const COVERED_CRATES: [&str; 6] = [
    "volsa-core",
    "volsa-dicom",
    "volsa-volume",
    "volsa-labels",
    "volsa-models",
    "volsa-segment",
];

/// Source files left out of a crate's count, as a pattern over their paths: what is in them
/// runs only with the networks' runtime loaded or a graphics card in the computer, and
/// neither is there when the gate runs. The paths are written with either slash.
const NOT_COUNTED: [(&str, &str); 1] = [(
    "volsa-models",
    r"volsa-models[/\\]src[/\\](runtime|cards)\.rs$",
)];

/// What the gate has cargo test: every crate's tests, each test program run to its end
/// however many before it failed, so that one failure does not hide another. Cargo still
/// fails at the end if any did, and so does the gate.
const EVERY_TEST: [&str; 4] = ["test", "--workspace", "--all-features", "--no-fail-fast"];

/// The crates with tests that need a model folder and the runtime. Such a test is marked
/// `ignore`, so that the other tasks pass it by.
const WITH_MODELS: [&str; 2] = ["volsa-models", "volsa-segment"];
/// The variables that name the model folder and the folder that holds the runtime.
const FOLDERS: [&str; 2] = ["VOLSA_MODELS", "VOLSA_RUNTIME"];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "coverage" => coverage(),
        "check-models" => check_models(),
        "notices" => notices::write(),
        "dist" => dist::make(&std::env::args().skip(2).collect::<Vec<_>>()),
        _ => Err("usage: cargo xtask <check|coverage|check-models|notices|dist>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Every gate, in the order a contributor would want to hear about failures.
fn check() -> Result<(), String> {
    run("cargo", &["fmt", "--all", "--", "--check"], &[])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    file_lengths()?;
    notices::up_to_date()?;
    run("cargo", &EVERY_TEST, &[])?;
    run("cargo", &["deny", "check"], &[])?;
    run(
        "cargo",
        &["doc", "--workspace", "--no-deps", "--all-features"],
        &[("RUSTDOCFLAGS", "-D warnings")],
    )
}

/// Line coverage of each non-GPU crate, each held to the floor on its own.
fn coverage() -> Result<(), String> {
    for name in COVERED_CRATES {
        let mut args = vec![
            "llvm-cov",
            "--package",
            name,
            "--all-features",
            "--fail-under-lines",
            COVERAGE_FLOOR,
        ];
        if let Some((_, files)) = NOT_COUNTED.iter().find(|(owner, _)| *owner == name) {
            args.extend(["--ignore-filename-regex", files]);
        }
        run("cargo", &args, &[])?;
    }
    Ok(())
}

/// Every ignored test of the crates in [`WITH_MODELS`], one at a time and each in a process
/// of its own, so that no two hold their networks at once. Stops at the first that fails.
/// Says how each test went and how long it took, as it goes and again at the end. A crate
/// that lists no such test is a failure: nothing run must never look like a pass.
fn check_models() -> Result<(), String> {
    let named = |folder: &&str| std::env::var_os(folder).is_some_and(|name| !name.is_empty());
    if !FOLDERS.iter().all(named) {
        println!("check-models: skipped, VOLSA_MODELS and VOLSA_RUNTIME are not both set");
        return Ok(());
    }
    let mut tests = Vec::new();
    for package in WITH_MODELS {
        let listed = ignored_tests(package)?;
        tests.extend(listed.into_iter().map(|test| (package, test)));
    }
    let (count, mut went) = (tests.len(), Vec::new());
    let mut failed = None;
    for (package, test) in tests {
        let started = Instant::now();
        let how = one_test(package, &test)?;
        let seconds = started.elapsed().as_secs_f64();
        let line = format!("check-models: {package} {test}: {how} in {seconds:.1} s");
        println!("{line}");
        went.push(line);
        if how == "failed" {
            failed = Some(format!("`{test}` of {package} failed"));
            break;
        }
    }
    let run = went.len();
    println!("\n==> check-models: {run} of {count} tests run");
    for line in went {
        println!("{line}");
    }
    failed.map_or(Ok(()), Err)
}

/// The arguments that have cargo run the tests of one crate, but not those written in its
/// documentation: what stands before the test program's own arguments.
fn test_of(package: &str) -> [&str; 6] {
    let (lib, tests) = ("--lib", "--tests");
    ["test", "-p", package, "--all-features", lib, tests]
}

/// The names of the ignored tests of a crate, as its test programs list them. None at all
/// is an error: every crate asked has some, so an empty list means the listing went wrong.
fn ignored_tests(package: &str) -> Result<Vec<String>, String> {
    let shown = format!("cargo test of {package}, to list its ignored tests");
    println!("\n==> {shown}");
    let listing = Command::new("cargo")
        .args(test_of(package))
        .args(["--", "--ignored", "--list"])
        .current_dir(workspace_root())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not start {shown}: {error}"))?;
    if !listing.status.success() {
        return Err(format!("{shown} failed"));
    }
    some_listed(package, listed(&String::from_utf8_lossy(&listing.stdout)))
}

/// The tests a crate listed, or the failure that it listed none.
fn some_listed(package: &str, tests: Vec<String>) -> Result<Vec<String>, String> {
    if tests.is_empty() {
        let why = "so nothing would have been checked there";
        return Err(format!("{package} lists no ignored test, {why}"));
    }
    Ok(tests)
}

/// The names of the tests in what a test program prints when asked to list them: a line
/// for each, the name and then `: test`. Any other line names no test.
fn listed(listing: &str) -> Vec<String> {
    let tests = listing
        .lines()
        .filter_map(|line| line.strip_suffix(": test"));
    tests.map(str::to_owned).collect()
}

/// Runs one ignored test by its exact name, letting through what it prints, and says how it
/// went. It passed when cargo said so and the test programs between them ran one test and
/// no more. A test that passed after saying it skipped its work is told apart, with why.
fn one_test(package: &str, test: &str) -> Result<String, String> {
    let shown = format!("cargo test of {package}: {test}");
    println!("\n==> {shown}");
    let mut running = Command::new("cargo")
        .args(test_of(package))
        .args(["--", "--ignored", "--exact", test, "--nocapture"])
        .current_dir(workspace_root())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start {shown}: {error}"))?;
    let printed = running.stdout.take();
    let printed = printed.ok_or_else(|| format!("{shown} has nowhere to print"))?;
    let (mut passed, mut skipped) = (0, None);
    for line in BufReader::new(printed).lines() {
        let line = line.map_err(|error| format!("could not read from {shown}: {error}"))?;
        println!("{line}");
        passed += passed_in(&line);
        skipped = skipped.or_else(|| skipped_in(&line).map(str::to_owned));
    }
    let status = running
        .wait()
        .map_err(|error| format!("lost {shown}: {error}"))?;
    Ok(match (status.success() && passed == 1, skipped) {
        (false, _) => "failed".to_owned(),
        (true, None) => "passed".to_owned(),
        (true, Some(why)) => format!("skipped ({why})"),
    })
}

/// Why a test skipped its work, when a line of its output says so: the test prints
/// `skipped: ` and the reason. With nothing held back, a test's first line comes straight
/// after the test program's own `test NAME ... `.
fn skipped_in(line: &str) -> Option<&str> {
    let said = line
        .strip_prefix("test ")
        .and_then(|rest| rest.split_once(" ... "));
    let said = said.map_or(line, |(_, said)| said);
    said.strip_prefix("skipped: ").map(str::trim)
}

/// How many tests a line of a test program's output says passed: the line that sums up a
/// run reads `test result: ok. 1 passed; 0 failed;` and so on. Any other line says none.
fn passed_in(line: &str) -> u32 {
    let summed = line
        .strip_prefix("test result: ")
        .and_then(|rest| rest.split_once(". "));
    let count = summed.and_then(|(_, counts)| counts.split_once(" passed"));
    count.and_then(|(count, _)| count.parse().ok()).unwrap_or(0)
}

/// Print a command, run it, and turn a failure into an error naming the command.
fn run(program: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let shown = format!("{program} {}", args.join(" "));
    println!("\n==> {shown}");
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(workspace_root())
        .status()
        .map_err(|error| format!("could not start `{shown}`: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{shown}` failed"))
    }
}

/// Fail if any Rust source file is longer than [`MAX_FILE_LINES`].
fn file_lengths() -> Result<(), String> {
    println!("\n==> file lengths (at most {MAX_FILE_LINES} lines per source file)");
    let root = workspace_root();
    let mut files = Vec::new();
    for folder in ["crates", "xtask"] {
        rust_files(&root.join(folder), &mut files)?;
    }
    let mut too_long = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|error| format!("could not read {}: {error}", file.display()))?;
        let lines = text.lines().count();
        if lines > MAX_FILE_LINES {
            too_long.push(format!("{} has {lines} lines", file.display()));
        }
    }
    if too_long.is_empty() {
        Ok(())
    } else {
        Err(format!("files over the limit: {}", too_long.join("; ")))
    }
}

/// Collect every `.rs` file under `folder`, skipping build output.
fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|error| format!("could not list {}: {error}", folder.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                rust_files(&path, found)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// The repository root: the parent of this package's folder.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wiki_tests;
