//! Voxa: a viewer for MRI scans. Not a medical device; not for diagnostic use.

use std::process::ExitCode;

use voxa_app::{Command, USAGE, parse_args, run, version};

fn main() -> ExitCode {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(Command::Run(initial)) => match run(initial) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Voxa could not start: {error}");
                ExitCode::FAILURE
            }
        },
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
