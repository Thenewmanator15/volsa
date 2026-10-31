//! A structured report read as lines of text.

use proptest::prelude::*;

use super::tests::a_report;
use super::*;
use crate::testkit::{DocumentOptions, ReportItem, SliceOptions, classic_slice, structured_report};

const KEY_IMAGE_NOTE: &str = "1.2.840.10008.5.1.4.1.1.88.59";

fn line(depth: u8, heading: Option<&str>, text: &str) -> ReportLine {
    ReportLine {
        depth,
        heading: heading.map(str::to_owned),
        text: text.to_owned(),
        is_name: false,
    }
}

fn read(content: &[ReportItem]) -> Report {
    let bytes = structured_report(&DocumentOptions::default(), content);
    read_report(&bytes).expect("a made-up report reads")
}

/// How many characters the lines hold together.
fn characters(report: &Report) -> usize {
    let heading = |line: &ReportLine| line.heading.as_deref().map_or(0, |h| h.chars().count());
    let all = report.lines.iter();
    all.map(|line| heading(line) + line.text.chars().count())
        .sum()
}

/// A section `last` levels deep: each level holds a note and the next level, and the last
/// holds nothing. The level at depth `n` is called `Level n`.
fn nested(last: usize) -> ReportItem {
    fn section(level: usize, last: usize) -> ReportItem {
        let mut items = Vec::new();
        if level < last {
            items.push(ReportItem::text("Note", &format!("at {level}")));
            items.push(section(level + 1, last));
        }
        ReportItem::container(&format!("Level {level}"), items)
    }
    section(0, last)
}

#[test]
fn a_report_reads_as_lines_in_its_order() {
    let report = read_report(&a_report(&DocumentOptions::default())).expect("reads");
    assert_eq!(
        report,
        Report {
            lines: vec![
                line(0, Some("Part A"), ""),
                line(1, None, "Comment: A made-up remark."),
                line(1, None, "Width: 12 mm"),
                line(0, None, "Closing: That is all."),
            ],
            cut_short: false,
        }
    );
}

#[test]
fn every_kind_of_value_becomes_a_line_of_concept_and_value() {
    let report = read(&[
        ReportItem::code("Colour", "Red"),
        ReportItem::plain("DATE", "Made on", "20200102"),
        ReportItem::plain("TIME", "At", "143015"),
        ReportItem::plain("DATETIME", "Stamped", "20200102143015"),
        ReportItem::plain("PNAME", "Seen by", "Nobody^Madeup"),
        ReportItem::plain("UIDREF", "Refers to", "1.2.3.4"),
    ]);
    let texts: Vec<&str> = report.lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Colour: Red",
            "Made on: 2 January 2020",
            "At: 143015",
            "Stamped: 2 January 2020, 14:30",
            "Seen by: Nobody^Madeup",
            "Refers to: 1.2.3.4",
        ]
    );
    assert!(report.lines.iter().all(|line| line.heading.is_none()));
    assert!(!report.cut_short);
}

#[test]
fn a_date_is_said_in_words_and_one_that_is_not_such_a_date_is_as_written() {
    let dated = |kind: &str, value: &str| {
        let report = read(&[ReportItem::plain(kind, "Written on", value)]);
        report.lines[0].text.clone()
    };
    assert_eq!(dated("DATE", "20200102"), "Written on: 2 January 2020");
    for odd in ["2020-01-02", "202001", "20201302"] {
        let said = dated("DATE", odd);
        let wanted = format!("Written on: {}", odd.trim());
        assert_eq!(said, wanted, "{odd:?}");
    }
    // A date and time: the date in words, then the hour and minute when it gives them.
    let stamped = |value: &str| dated("DATETIME", value);
    assert_eq!(stamped("20200102"), "Written on: 2 January 2020");
    assert_eq!(
        stamped("20200102143015"),
        "Written on: 2 January 2020, 14:30"
    );
    assert_eq!(
        stamped("20200102143015.250000"),
        "Written on: 2 January 2020, 14:30"
    );
    assert_eq!(stamped("202001021430"), "Written on: 2 January 2020, 14:30");
    // What is cut off, out of range or carries an offset from universal time is as written.
    for odd in [
        "20200102+0100",
        "2020010214",
        "20200102143015+0100",
        "20200102256100",
        "x",
    ] {
        assert_eq!(stamped(odd), format!("Written on: {odd}"), "{odd:?}");
    }
}

#[test]
fn an_item_that_holds_a_persons_name_is_marked_as_one() {
    let report = read(&[
        ReportItem::plain("PNAME", "Seen by", "Nobody^Madeup"),
        // No name of its own: it is given one, so that its value can be told from it.
        ReportItem::plain("PNAME", "", "Nobody^Madeup"),
        ReportItem::text("Note", "Nobody Madeup was here"),
        ReportItem::plain("UIDREF", "Refers to", "1.2.3.4"),
        ReportItem::container(
            "Part A",
            vec![ReportItem::plain("PNAME", "Also", "Else^One")],
        ),
    ]);
    let marked: Vec<(&str, bool)> = report
        .lines
        .iter()
        .map(|line| (line.text.as_str(), line.is_name))
        .collect();
    assert_eq!(
        marked,
        [
            ("Seen by: Nobody^Madeup", true),
            ("Name: Nobody^Madeup", true),
            ("Note: Nobody Madeup was here", false),
            ("Refers to: 1.2.3.4", false),
            ("", false),
            ("Also: Else^One", true),
        ]
    );
}

#[test]
fn an_item_that_is_not_text_says_so_in_its_place() {
    let report = read(&[
        ReportItem::text("Before", "one"),
        ReportItem::unshown("IMAGE", "Key picture"),
        ReportItem::unshown("WAVEFORM", "Trace"),
        ReportItem::unshown("SOMETHING-NEW", "Odd"),
        ReportItem::text("After", "two"),
    ]);
    let texts: Vec<&str> = report.lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Before: one",
            "Key picture: an image, not shown",
            "Trace: a waveform, not shown",
            "Odd: an item of a kind Voxa does not read, not shown",
            "After: two",
        ]
    );
    assert!(!report.cut_short, "nothing was left out unsaid");
}

#[test]
fn an_item_without_a_name_or_a_value_is_still_a_line_in_its_place() {
    let report = read(&[
        ReportItem::text("", "no name"),
        ReportItem::text("No value", ""),
        ReportItem::container("", vec![ReportItem::text("Inside", "x")]),
    ]);
    assert_eq!(
        report.lines,
        [
            line(0, None, "no name"),
            line(0, None, "No value:"),
            line(0, Some(""), ""),
            line(1, None, "Inside: x"),
        ]
    );
}

#[test]
fn a_report_with_no_content_reads_as_no_lines() {
    assert_eq!(
        read(&[]),
        Report {
            lines: Vec::new(),
            cut_short: false
        }
    );
}

#[test]
fn a_report_nested_too_deep_is_cut_short_and_says_so() {
    let report = read(&[
        nested(MAX_REPORT_DEPTH + 4),
        ReportItem::text("After", "never read"),
    ]);
    assert!(report.cut_short);
    let deepest = report
        .lines
        .iter()
        .map(|line| usize::from(line.depth))
        .max();
    assert_eq!(deepest, Some(MAX_REPORT_DEPTH));
    // What is shown is the start of the report, whole and in order, and nothing after it.
    let last = report.lines.last().expect("some lines");
    assert_eq!(last, &line(16, Some("Level 16"), ""));
    assert!(
        report
            .lines
            .iter()
            .all(|line| !line.text.contains("never read"))
    );
    assert_eq!(
        report.lines.len(),
        17 + 16,
        "17 sections and the 16 notes before them"
    );
}

#[test]
fn a_report_exactly_as_deep_as_allowed_is_whole() {
    let report = read(&[nested(MAX_REPORT_DEPTH), ReportItem::text("After", "read")]);
    assert!(!report.cut_short);
    assert_eq!(report.lines.last(), Some(&line(0, None, "After: read")));
    assert_eq!(report.lines.len(), 17 + 16 + 1);
}

#[test]
fn a_report_of_too_many_lines_is_cut_short_after_the_last_it_may_hold() {
    let items = |count: usize| -> Vec<ReportItem> {
        (0..count)
            .map(|at| ReportItem::text("Item", &at.to_string()))
            .collect()
    };
    let whole = read(&items(MAX_REPORT_LINES));
    assert!(!whole.cut_short);
    assert_eq!(whole.lines.len(), MAX_REPORT_LINES);

    let cut = read(&items(MAX_REPORT_LINES + 1));
    assert!(cut.cut_short);
    assert_eq!(cut.lines.len(), MAX_REPORT_LINES);
    let last = format!("Item: {}", MAX_REPORT_LINES - 1);
    assert_eq!(
        cut.lines.last().map(|line| line.text.as_str()),
        Some(last.as_str())
    );
}

#[test]
fn a_report_of_too_many_characters_is_cut_short_at_the_limit() {
    let long = "a".repeat(600_000);
    let report = read(&[
        ReportItem::text("First", &long),
        ReportItem::text("Second", &long),
        ReportItem::text("Third", "never read"),
    ]);
    assert!(report.cut_short);
    assert_eq!(report.lines.len(), 2);
    assert_eq!(characters(&report), MAX_REPORT_CHARS);
    let first = report.lines.first().expect("a first line");
    assert_eq!(
        first.text.chars().count(),
        "First: ".len() + long.len(),
        "kept whole"
    );
    let second = report.lines.last().expect("a second line");
    assert!(second.text.ends_with('…'), "the cut line says it was cut");
}

#[test]
fn a_line_cut_at_the_character_limit_ends_in_an_ellipsis() {
    // Room is left for ten characters after the first line.
    let first = "a".repeat(MAX_REPORT_CHARS - 10 - "First: ".len());
    let report = read(&[
        ReportItem::text("First", &first),
        ReportItem::number("Width", "152", "mm"),
        ReportItem::text("Later", "never read"),
    ]);
    assert!(report.cut_short);
    assert_eq!(report.lines.len(), 2);
    let cut = report.lines.last().map(|line| line.text.as_str());
    assert_eq!(cut, Some("Width: 15…"), "not a number that looks whole");
    assert_eq!(characters(&report), MAX_REPORT_CHARS);
}

#[test]
fn a_heading_cut_at_the_character_limit_ends_in_an_ellipsis() {
    let first = "a".repeat(MAX_REPORT_CHARS - 5 - "First: ".len());
    let report = read(&[
        ReportItem::text("First", &first),
        ReportItem::container("Long heading", vec![ReportItem::text("Inside", "x")]),
    ]);
    assert!(report.cut_short);
    assert_eq!(report.lines.len(), 2);
    assert_eq!(report.lines.last(), Some(&line(0, Some("Long…"), "")));
    assert_eq!(characters(&report), MAX_REPORT_CHARS);
}

#[test]
fn nothing_is_added_when_no_room_is_left() {
    let value = "a".repeat(MAX_REPORT_CHARS - "Only: ".len());
    let report = read(&[
        ReportItem::text("Only", &value),
        ReportItem::text("Next", "x"),
    ]);
    assert!(report.cut_short);
    assert_eq!(report.lines.len(), 1);
    assert_eq!(characters(&report), MAX_REPORT_CHARS);
}

#[test]
fn a_report_just_inside_the_character_limit_is_whole() {
    let value = "a".repeat(MAX_REPORT_CHARS - "Only: ".len());
    let report = read(&[ReportItem::text("Only", &value)]);
    assert!(!report.cut_short);
    assert_eq!(characters(&report), MAX_REPORT_CHARS);
}

#[test]
fn a_damaged_report_is_an_error_not_a_panic() {
    let bytes = a_report(&DocumentOptions::default());
    for cut in [0, 1, 100, 132, 200, bytes.len() / 2, bytes.len() - 1] {
        let result = read_report(&bytes[..cut]);
        assert!(
            result.is_err(),
            "{cut} bytes of {} gave {result:?}",
            bytes.len()
        );
    }
}

#[test]
fn what_is_not_a_report_is_an_error() {
    assert!(matches!(
        read_report(b"just some text"),
        Err(VoxaError::NotDicom)
    ));
    let image = classic_slice(&SliceOptions::default());
    assert!(read_report(&image).is_err(), "an image is not a report");
    let other = DocumentOptions {
        sop_class: Some("1.2.840.10008.5.1.4.1.1.11.1".to_owned()),
        ..DocumentOptions::default()
    };
    assert!(
        read_report(&a_report(&other)).is_err(),
        "a presentation state is not a report"
    );
    let note = DocumentOptions {
        sop_class: Some(KEY_IMAGE_NOTE.to_owned()),
        ..DocumentOptions::default()
    };
    assert!(
        read_report(&a_report(&note)).is_err(),
        "a key-image note is not a report to read as text"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn whatever_the_bytes_reading_a_report_never_panics(
        cut in 0usize..700,
        at in 0usize..700,
        byte in any::<u8>(),
    ) {
        let mut bytes = a_report(&DocumentOptions::default());
        if let Some(slot) = bytes.get_mut(at) {
            *slot = byte;
        }
        bytes.truncate(cut.max(1));
        // Either answer will do: the test is that there is one.
        let _ = read_report(&bytes);
        let _ = pdf_bytes(&bytes);
    }
}
