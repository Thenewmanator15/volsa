//! What Voxa says in place of opening its window, when the window cannot be opened, or when it
//! has closed unexpectedly: the help, the version, a mistake on the command line, a failure to
//! start, a crash. Where Voxa has a
//! console these are printed; a program built for Windows without one says them in a small
//! message window, or they would be said to nobody.

use std::path::Path;

use crate::args::{Command, USAGE, version};

/// Whether the program was built without a console to print to: a release build for Windows,
/// which says what it has to say in a message window instead.
pub const WINDOWED: bool = cfg!(all(windows, not(debug_assertions)));

/// Something to tell the person, and how the program should then end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What to say.
    pub text: String,
    /// Whether it is a failure.
    pub failed: bool,
    /// The code the program ends with.
    pub code: u8,
}

impl Outcome {
    /// That the window could not be opened, and why.
    pub fn could_not_start(why: &impl std::fmt::Display) -> Self {
        Self {
            text: format!("Voxa could not start: {why}"),
            failed: true,
            code: 1,
        }
    }
}

/// What to tell the person when Voxa stops unexpectedly, the program having panicked: where
/// its `log` is, if it keeps one. Nothing for a picture of the window (`capturing`), which
/// no one is there to read.
pub fn after_a_crash(capturing: bool, log: Option<&Path>) -> Option<Outcome> {
    if capturing {
        return None;
    }
    let first = "Voxa stopped unexpectedly and has closed. Open it again to carry on.";
    let text = match log {
        Some(log) => format!(
            "{first}\n\nWhere it stopped is written in its log:\n{}",
            log.display()
        ),
        None => first.to_owned(),
    };
    Some(Outcome {
        text,
        failed: true,
        // What a Rust program that panicked ends with.
        code: 101,
    })
}

/// What the command line asks Voxa to say in place of running; `None` when it asks for a run.
pub fn outcome(command: &Result<Command, String>) -> Option<Outcome> {
    let said = |text: String| Outcome {
        text,
        failed: false,
        code: 0,
    };
    match command {
        Ok(Command::Help) => Some(said(USAGE.to_owned())),
        Ok(Command::Version) => Some(said(version())),
        Ok(Command::Run(_) | Command::RunWith { .. } | Command::PluginHelper) => None,
        Err(mistake) => Some(Outcome {
            text: format!("{mistake}\n\n{USAGE}"),
            failed: true,
            code: 2,
        }),
    }
}

/// Say it: in a message window when the program has no console (`windowed`), and otherwise
/// on standard output, or standard error for a failure.
pub fn tell(outcome: &Outcome, windowed: bool) {
    if windowed {
        let level = if outcome.failed {
            rfd::MessageLevel::Error
        } else {
            rfd::MessageLevel::Info
        };
        rfd::MessageDialog::new()
            .set_title("Voxa")
            .set_description(outcome.text.as_str())
            .set_level(level)
            .show();
    } else if outcome.failed {
        eprintln!("{}", outcome.text);
    } else {
        println!("{}", outcome.text);
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::args::{USAGE, version};

    #[test]
    fn the_help_and_the_version_are_said_and_are_not_failures() {
        let help = outcome(&Ok(Command::Help)).expect("something to say");
        assert_eq!(
            (help.text.as_str(), help.failed, help.code),
            (USAGE, false, 0)
        );
        let which = outcome(&Ok(Command::Version)).expect("something to say");
        assert_eq!(
            (which.text, which.failed, which.code),
            (version(), false, 0)
        );
    }

    #[test]
    fn a_mistake_on_the_command_line_is_said_with_the_help_after_it() {
        let mistake = outcome(&Err("Voxa has no option --sideways.".to_owned()));
        let mistake = mistake.expect("something to say");
        assert!(mistake.failed);
        assert_eq!(mistake.code, 2);
        assert_eq!(
            mistake.text,
            format!("Voxa has no option --sideways.\n\n{USAGE}")
        );
    }

    #[test]
    fn a_run_says_nothing_first() {
        assert_eq!(outcome(&Ok(Command::Run(None))), None);
        let with = Command::RunWith {
            scan: None,
            labels: PathBuf::from("labels.nii.gz"),
        };
        assert_eq!(outcome(&Ok(with)), None);
    }

    #[test]
    fn a_crash_is_said_with_where_the_log_is_but_never_in_a_picture_run() {
        let log = Path::new("C:/made/up/Voxa/data/voxa.log");
        let said = after_a_crash(false, Some(log)).expect("something to say");
        assert!(
            said.text
                .starts_with("Voxa stopped unexpectedly and has closed."),
            "{}",
            said.text
        );
        assert!(
            said.text.contains("C:/made/up/Voxa/data/voxa.log"),
            "{}",
            said.text
        );
        assert!(said.failed);
        assert_eq!(
            said.code, 101,
            "the code a Rust program that panicked ends with"
        );
        let without = after_a_crash(false, None).expect("something to say");
        assert!(without.text.starts_with("Voxa stopped unexpectedly"));
        assert!(!without.text.contains("log"), "{}", without.text);
        assert_eq!(
            after_a_crash(true, Some(log)),
            None,
            "no one reads a picture run"
        );
    }

    #[test]
    fn a_failure_to_start_is_a_failure_with_its_reason() {
        let failed = Outcome::could_not_start(&"no graphics device");
        assert_eq!(failed.text, "Voxa could not start: no graphics device");
        assert!(failed.failed);
        assert_eq!(failed.code, 1);
    }
}
