//! What the command line asks for.

use std::ffi::OsString;
use std::path::PathBuf;

/// What to say for `--help`.
pub const USAGE: &str = "\
Volsa, a viewer for MRI and CT scans.

Usage: volsa [PATH] [--labels FILE]

  PATH           a folder of DICOM files, a zip file of them, or a DICOMDIR to open
  --labels FILE  labels made for that scan, to show its parts: a NIfTI file (.nii, .nii.gz)
  -h, --help     show this help
  -V, --version  show the version

A scan can also be opened by dropping it on the window.

Volsa is not a medical device and is not for diagnostic use.";

/// What the command line asks Volsa to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Open the window, with this scan in it if one was named.
    Run(Option<PathBuf>),
    /// Open the window with a scan, if one was named, and these labels on it.
    RunWith {
        /// The scan to open.
        scan: Option<PathBuf>,
        /// The labels to open on it once it can be shown in 3D.
        labels: PathBuf,
    },
    /// Say which version this is.
    Version,
    /// Say how Volsa is started.
    Help,
    /// Serve one plugin run as Volsa's helper, over standard input and output, with no window:
    /// `--plugin-helper` and nothing else. Volsa starts itself so; a person never needs to.
    PluginHelper,
}

/// The argument Volsa starts its plugin helper with.
pub const HELPER: &str = "--plugin-helper";

/// What to say for `--version`.
pub fn version() -> String {
    format!("Volsa {}", env!("CARGO_PKG_VERSION"))
}

/// Read the arguments that follow the program's name: nothing, or one scan to open, or a
/// request for the version or the help. After `--` everything is taken as a path.
///
/// # Errors
///
/// A sentence for the person at the keyboard when more than one scan is named (it lists the
/// ones to leave out) or an option is given that Volsa does not have.
pub fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let args: Vec<OsString> = args.collect();
    if let [only] = args.as_slice()
        && only == HELPER
    {
        return Ok(Command::PluginHelper);
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut asked = None;
    let mut only_paths = false;
    let mut labels = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            _ if only_paths => paths.push(PathBuf::from(&arg)),
            "--" => only_paths = true,
            "-h" | "--help" => asked = Some(Command::Help),
            "-V" | "--version" => asked = asked.or(Some(Command::Version)),
            "--labels" => {
                let named = args.next().map(PathBuf::from);
                labels = Some(named.ok_or("--labels needs the label file to open after it.")?);
            }
            option if option.starts_with('-') && option.len() > 1 => {
                return Err(format!("Volsa has no option {option}"));
            }
            _ => paths.push(PathBuf::from(&arg)),
        }
    }
    if let Some(asked) = asked {
        return Ok(asked);
    }
    let mut paths = paths.into_iter();
    let first = paths.next();
    let extras: Vec<String> = paths.map(|path| path.display().to_string()).collect();
    if let (true, Some(labels)) = (extras.is_empty(), labels) {
        Ok(Command::RunWith {
            scan: first,
            labels,
        })
    } else if extras.is_empty() {
        Ok(Command::Run(first))
    } else {
        Err(format!(
            "Volsa opens one scan at a time. Leave out: {}",
            extras.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    fn parsed(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn labels_are_named_with_their_own_option() {
        let both = Command::RunWith {
            scan: Some(PathBuf::from("scan")),
            labels: PathBuf::from("l.nii.gz"),
        };
        assert_eq!(parsed(&["scan", "--labels", "l.nii.gz"]), Ok(both.clone()));
        assert_eq!(parsed(&["--labels", "l.nii.gz", "scan"]), Ok(both));
        let alone = Command::RunWith {
            scan: None,
            labels: PathBuf::from("l.nii.gz"),
        };
        assert_eq!(parsed(&["--labels", "l.nii.gz"]), Ok(alone));
        let nothing_after = parsed(&["scan", "--labels"]).expect_err("needs a file");
        assert!(
            nothing_after.contains("--labels") && nothing_after.ends_with('.'),
            "{nothing_after}"
        );
        assert!(USAGE.contains("--labels FILE"));
    }

    #[test]
    fn plugin_helper_is_its_own_command() {
        assert_eq!(parsed(&["--plugin-helper"]), Ok(Command::PluginHelper));
        assert!(parsed(&["--plugin-helper", "scan"]).is_err());
        assert!(parsed(&["scan", "--plugin-helper"]).is_err());
    }

    #[test]
    fn the_help_does_not_mention_the_helper() {
        assert!(!USAGE.contains("plugin-helper"));
    }

    #[test]
    fn with_no_arguments_the_window_opens_empty() {
        assert_eq!(parsed(&[]), Ok(Command::Run(None)));
    }

    #[test]
    fn one_path_is_what_to_open_whatever_characters_it_has() {
        let path = "C:/scans/Köpf scan 東京/my export.zip";
        assert_eq!(parsed(&[path]), Ok(Command::Run(Some(PathBuf::from(path)))));
    }

    #[test]
    fn version_and_help_are_asked_for_by_flag_and_win_over_a_path() {
        assert_eq!(parsed(&["--version"]), Ok(Command::Version));
        assert_eq!(parsed(&["-V"]), Ok(Command::Version));
        assert_eq!(parsed(&["--help"]), Ok(Command::Help));
        assert_eq!(parsed(&["-h"]), Ok(Command::Help));
        assert_eq!(parsed(&["scan.zip", "--help"]), Ok(Command::Help));
    }

    #[test]
    fn more_than_one_thing_to_open_is_refused_naming_the_extras() {
        let refused = parsed(&["first.zip", "second.zip", "third folder"]).expect_err("too many");
        assert_eq!(
            refused,
            "Volsa opens one scan at a time. Leave out: second.zip, third folder"
        );
    }

    #[test]
    fn a_flag_volsa_does_not_have_is_refused_by_name() {
        let refused = parsed(&["--frobnicate", "scan.zip"]).expect_err("no such flag");
        assert_eq!(refused, "Volsa has no option --frobnicate");
    }

    #[test]
    fn after_two_dashes_everything_is_a_path() {
        let odd = parsed(&["--", "--version"]);
        assert_eq!(odd, Ok(Command::Run(Some(PathBuf::from("--version")))));
    }

    #[test]
    fn the_help_says_how_to_open_a_scan_and_carries_the_disclaimer() {
        assert!(USAGE.contains("volsa [PATH]"), "{USAGE}");
        assert!(USAGE.contains("Volsa is not a medical device and is not for diagnostic use."));
        assert!(
            version().starts_with("Volsa ") && version().contains('.'),
            "{}",
            version()
        );
    }
}
