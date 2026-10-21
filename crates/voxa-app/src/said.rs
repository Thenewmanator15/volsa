//! What Voxa says in place of opening its window, or when the window cannot be opened: the
//! help, the version, a mistake on the command line, a failure to start. Where Voxa has a
//! console these are printed; a program built for Windows without one says them in a small
//! message window, or they would be said to nobody.

use crate::args::{Command, USAGE, version};

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
        Ok(Command::Run(_) | Command::RunWith { .. }) => None,
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
    use std::path::PathBuf;

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
    fn a_failure_to_start_is_a_failure_with_its_reason() {
        let failed = Outcome::could_not_start(&"no graphics device");
        assert_eq!(failed.text, "Voxa could not start: no graphics device");
        assert!(failed.failed);
        assert_eq!(failed.code, 1);
    }
}
