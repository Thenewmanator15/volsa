//! Voxa: a viewer for MRI scans. Not a medical device; not for diagnostic use.

use std::path::PathBuf;
use std::process::ExitCode;

use voxa_app::{Command, USAGE, parse_args, run, version};

/// Open the window and run until it is closed.
fn started(scan: Option<PathBuf>, labels: Option<PathBuf>) -> ExitCode {
    match run(scan, labels) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Voxa could not start: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(Command::Run(scan)) => started(scan, None),
        Ok(Command::RunWith { scan, labels }) => started(scan, Some(labels)),
        Ok(Command::Version) => {
            println!("{}", version());
            ExitCode::SUCCESS
        }
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{problem}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
