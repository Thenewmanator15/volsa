//! The log: what goes into it, what never does, and how it is kept small.

use std::time::{Duration, UNIX_EPOCH};

use volsa_dicom::testkit::{SliceOptions, classic_slice};

use super::*;
use crate::state::NoticeKind;
use crate::testing::indexed;

/// Run `work` with everything it logs going to `volsa.log` in `folder`; what the file then holds.
fn logged(folder: &Path, work: impl FnOnce()) -> String {
    let logger = to_file(folder).expect("the log opens");
    tracing::subscriber::with_default(logger, work);
    std::fs::read_to_string(folder.join(FILE)).expect("the log is there")
}

/// Things a log must never hold, in a made-up export: who, which number, when, what the
/// scanner called the study and the series.
const NEVER: [&str; 6] = [
    "Quillfeather",
    "Zebedee",
    "ZQ-99731",
    "19870623",
    "Wobblewort",
    "frumious",
];

fn made_up_export() -> volsa_dicom::Index {
    let image = |study: &str, series: &str, name: &str| {
        let options = SliceOptions {
            study_uid: study.to_owned(),
            series_uid: series.to_owned(),
            patient_name: "Quillfeather^Zebedee".to_owned(),
            patient_id: "ZQ-99731".to_owned(),
            study_date: Some("19870623".to_owned()),
            study_description: "Wobblewort study".to_owned(),
            series_description: "frumious series".to_owned(),
            ..SliceOptions::default()
        };
        (name.to_owned(), classic_slice(&options))
    };
    let mut files = vec![
        image("9.1", "9.1.1", "a"),
        image("9.1", "9.1.2", "b"),
        image("9.2", "9.2.1", "c"),
    ];
    files.push(("Zebedee notes.txt".to_owned(), b"not an image".to_vec()));
    indexed(files)
}

fn notice(kind: NoticeKind, key: &'static str, text: &str) -> Notice {
    Notice {
        kind,
        text: text.to_owned(),
        details: vec!["Zebedee notes.txt".to_owned()],
        key: Some(key),
    }
}

#[test]
fn the_log_says_what_happened_and_never_who_it_happened_to() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let own = "Saved views could not be written, so they will not be there next time";
    let failed = "The file could not be read: C:\\exports\\Zebedee Quillfeather\\scan.zip.";
    let text = logged(folder.path(), || {
        opened(&made_up_export());
        said(&notice(NoticeKind::Warning, "views", own));
        said(&notice(NoticeKind::Error, "opening", failed));
    });
    assert!(
        text.contains("2 studies, 3 series, 1 file skipped"),
        "{text}"
    );
    assert!(text.contains(&format!("warning [views]: {own}")), "{text}");
    assert!(text.contains("error [opening]"), "{text}");
    for never in NEVER.iter().chain(&["exports", "scan.zip", "notes.txt"]) {
        assert!(!text.contains(never), "{never} is in the log: {text}");
    }
}

#[test]
fn a_notice_is_written_in_full_only_when_its_words_are_volsas_own() {
    let own = notice(NoticeKind::Info, "skipped", "1 file skipped");
    assert_eq!(line_for(&own), "notice [skipped]: 1 file skipped");
    // The names of the files not read, and why, are its details, which are never written.
    let refused = notice(NoticeKind::Warning, "refused", "1 file was not read");
    assert_eq!(line_for(&refused), "warning [refused]: 1 file was not read");
    let saved = notice(NoticeKind::Info, "saving", "Saved Quillfeather.png");
    assert!(!line_for(&saved).contains("Quillfeather"));
    assert!(line_for(&saved).starts_with("notice [saving]"));
    let unkeyed = Notice { key: None, ..saved };
    assert!(line_for(&unkeyed).starts_with("notice [no key]"));
}

#[test]
fn each_line_carries_when_and_how_grave() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let text = logged(folder.path(), || tracing::warn!("something to note"));
    let line = text.lines().next().expect("one line");
    let (when, rest) = line.split_at(20);
    assert!(
        when.ends_with('Z') && when.contains('-') && when.contains(':'),
        "{line}"
    );
    assert_eq!(rest.trim(), "WARN  something to note");
}

#[test]
fn what_other_peoples_code_logs_is_left_out() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let text = logged(folder.path(), || {
        tracing::warn!(target: "dicom_object", "a value read from a file: Quillfeather");
        tracing::debug!("too fine to keep");
        tracing::info!("kept");
    });
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("kept"));
}

#[test]
fn two_runs_write_to_the_same_log_one_after_the_other() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    logged(folder.path(), || tracing::info!("the first run"));
    let text = logged(folder.path(), || tracing::info!("the second run"));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("the first run") && lines[1].ends_with("the second run"));
}

#[test]
fn a_log_grown_too_long_is_cut_back_to_its_last_lines() {
    let mut lines = String::new();
    for n in 0..200_000 {
        writeln!(lines, "line {n:07}").expect("writes to a string");
    }
    assert!(lines.len() as u64 > MOST_BYTES);
    let kept = cut_back(&lines, 1000);
    assert!(kept.len() <= 1000 && kept.len() > 980, "{}", kept.len());
    assert!(
        kept.starts_with("line "),
        "whole lines only: {:?}",
        &kept[..20]
    );
    assert!(kept.ends_with("line 0199999\n"));
    assert_eq!(cut_back("short\n", 1000), "short\n");

    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(folder.path().join(FILE), &lines).expect("writes");
    let text = logged(folder.path(), || tracing::info!("after"));
    assert!((text.len() as u64) < MOST_BYTES, "{}", text.len());
    assert!(text.contains("line 0199999\n") && text.ends_with("after\n"));
}

#[test]
fn with_nowhere_to_keep_a_log_there_is_none() {
    assert_eq!(start(None), None);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let not_a_folder = folder.path().join("in the way");
    std::fs::write(&not_a_folder, "a file").expect("writes");
    assert!(to_file(&not_a_folder).is_none());
}

#[test]
fn the_time_on_a_line_is_the_date_and_time_in_utc() {
    assert_eq!(civil(0), (1970, 1, 1));
    assert_eq!(civil(11_017), (2000, 3, 1));
    assert_eq!(civil(19_782), (2024, 2, 29));
    let then = UNIX_EPOCH + Duration::from_secs(19_782 * 86_400 + 3661);
    assert_eq!(stamp(then), "2024-02-29 01:01:01Z");
}

#[test]
fn a_crash_is_logged_by_where_it_happened_and_not_by_what_was_said() {
    let here = std::panic::Location::caller();
    let line = crash_line(Some(here));
    assert!(
        line.starts_with("Volsa stopped at ") && line.contains("log"),
        "{line}"
    );
    assert_eq!(
        crash_line(None),
        "Volsa stopped at a place that is not known"
    );
}
