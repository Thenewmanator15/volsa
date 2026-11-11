//! Volsa: a viewer for MRI scans. Not a medical device; not for diagnostic use.

// A release build for Windows opens no console window beside its own. What it would have
// printed there is said in a message window instead.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;

use volsa_app::{Command, Outcome, WINDOWED, outcome, parse_args, run, tell};

fn main() -> ExitCode {
    let command = parse_args(std::env::args_os().skip(1));
    if let Ok(Command::PluginHelper) = command {
        return volsa_plugins::helper::serve_stdio();
    }
    if let Some(said) = outcome(&command) {
        tell(&said, WINDOWED);
        return ExitCode::from(said.code);
    }
    let (scan, labels) = match command {
        Ok(Command::RunWith { scan, labels }) => (scan, Some(labels)),
        Ok(Command::Run(scan)) => (scan, None),
        _ => (None, None),
    };
    match run(scan, labels) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let failed = Outcome::could_not_start(&error);
            tell(&failed, WINDOWED);
            ExitCode::from(failed.code)
        }
    }
}
