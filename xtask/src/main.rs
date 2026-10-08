//! Project tasks for Voxa.
//!
//! `cargo xtask check` runs every quality gate in order and stops at the first failure:
//! formatting, lints, file length, tests, licence and advisory checks, documentation.
//! `cargo xtask coverage` measures line coverage of the crates that need no GPU and fails
//! below the floor.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Longest a source file may be before it should be split.
const MAX_FILE_LINES: usize = 400;
/// Lowest acceptable line coverage, in percent, for each crate in [`COVERED_CRATES`].
const COVERAGE_FLOOR: &str = "80";
/// Crates whose tests need no GPU and so carry the coverage floor.
const COVERED_CRATES: [&str; 3] = ["voxa-core", "voxa-dicom", "voxa-volume"];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "check" => check(),
        "coverage" => coverage(),
        _ => Err("usage: cargo xtask <check|coverage>".to_owned()),
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
        run(
            "cargo",
            &[
                "llvm-cov",
                "--package",
                name,
                "--all-features",
                "--fail-under-lines",
                COVERAGE_FLOOR,
            ],
            &[],
        )?;
    }
    Ok(())
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
