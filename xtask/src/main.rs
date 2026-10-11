//! Project tasks for Voxa.
//!
//! `cargo xtask check` runs every quality gate in order and stops at the first failure:
//! formatting, lints, file length, tests, licence and advisory checks, documentation.
//! `cargo xtask coverage` measures line coverage of the crates that need no GPU and fails
//! below the floor. The few files that cannot run without a graphics card or the networks'
//! runtime are left out of the count.
//! `cargo xtask check-models` holds the labelling of a scan against `SynthSeg`'s own answers,
//! on a computer that has the networks: it runs the tests the other two tasks leave out,
//! which need a model folder and the networks' runtime. Without them it does nothing.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

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
    "voxa-core",
    "voxa-dicom",
    "voxa-volume",
    "voxa-labels",
    "voxa-models",
    "voxa-segment",
];

/// Source files left out of a crate's count, as a pattern over their paths: what is in them
/// runs only with the networks' runtime loaded or a graphics card in the computer, and
/// neither is there when the gate runs. The paths are written with either slash.
const NOT_COUNTED: [(&str, &str); 1] = [(
    "voxa-models",
    r"voxa-models[/\\]src[/\\](runtime|cards)\.rs$",
)];

/// The crates with tests that need a model folder and the runtime. Such a test is marked
/// `ignore`, so that the other tasks pass it by.
const WITH_MODELS: [&str; 2] = ["voxa-models", "voxa-segment"];
/// The variables that name the model folder and the folder that holds the runtime.
const FOLDERS: [&str; 2] = ["VOXA_MODELS", "VOXA_RUNTIME"];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "coverage" => coverage(),
        "check-models" => check_models(),
        _ => Err("usage: cargo xtask <check|coverage|check-models>".to_owned()),
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
    run("cargo", &["test", "--workspace", "--all-features"], &[])?;
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
        println!("check-models: skipped, VOXA_MODELS and VOXA_RUNTIME are not both set");
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
mod tests {
    use super::{listed, passed_in, skipped_in, some_listed};

    #[test]
    fn a_listing_names_its_tests_and_nothing_else() {
        let listing = "network::tests::a_stop_set_beforehand_stops: test\n\
                       the_volume_is_prepared_as_python_prepares_it: test\n\
                       a_made_up_benchmark: bench\n\
                       \n\
                       2 tests, 1 benchmark\n";
        let names = [
            "network::tests::a_stop_set_beforehand_stops",
            "the_volume_is_prepared_as_python_prepares_it",
        ];
        assert_eq!(listed(listing), names);
        // A listing with no test in it names none: empty, or only lines that are not tests.
        let none: Vec<String> = Vec::new();
        assert_eq!(listed(""), none);
        assert_eq!(listed("\n0 tests, 0 benchmarks\n"), none);
        assert_eq!(listed("a name: test and more\n: testing\n"), none);
        // A crate that lists none has not been checked, and that is said, not passed over.
        let said = some_listed("voxa-models", none.clone());
        let why = "voxa-models lists no ignored test, so nothing would have been checked there";
        assert_eq!(said, Err(why.to_owned()));
        let some = vec!["a_made_up_test".to_owned()];
        assert_eq!(some_listed("voxa-models", some.clone()), Ok(some));
    }

    #[test]
    fn a_test_that_skipped_its_work_says_so_in_a_line_of_its_own_words() {
        let why = Some("this computer has no graphics card that is Integrated");
        let alone = "skipped: this computer has no graphics card that is Integrated";
        assert_eq!(skipped_in(alone), why);
        // The first line a test prints follows the test program's own words for the test.
        let first = format!("test the_standard_model_on_integrated_graphics ... {alone}");
        assert_eq!(skipped_in(&first), why);
        let others = [
            "",
            "test a_name ... ok",
            "the standard model on the processor: 40.2 s in all",
            "  against Python: 0 voxels differ; skipped: nothing",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out",
        ];
        for line in others {
            assert_eq!(skipped_in(line), None, "{line}");
        }
    }

    #[test]
    fn only_the_line_that_sums_up_a_run_says_how_many_tests_passed() {
        let one = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out";
        let none = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out";
        let failed = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured";
        let twelve = "test result: ok. 12 passed; 0 failed; 3 ignored; finished in 2.73s";
        let sums = [(one, 1), (none, 0), (failed, 0), (twelve, 12)];
        for (line, passed) in sums {
            assert_eq!(passed_in(line), passed, "{line}");
        }
        // What a test prints, and what stands round it, says nothing of how many passed.
        let others = [
            "",
            "running 1 test",
            "test network::tests::a_stop_set_beforehand_stops ... ok",
            "1 passed; 0 failed",
            "test result: ok. some passed",
            "  against Python: test result: ok. 1 passed",
        ];
        for line in others {
            assert_eq!(passed_in(line), 0, "{line}");
        }
    }
}
