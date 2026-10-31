//! A structured report read as lines of text.

use proptest::prelude::*;

use super::tests::a_report;
use super::*;
use crate::testkit::{DocumentOptions, ReportItem, SliceOptions, classic_slice, structured_report};

fn line(depth: u8, heading: Option<&str>, text: &str) -> ReportLine {
    ReportLine {
        depth,
        heading: heading.map(str::to_owned),
        text: text.to_owned(),
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
            "Made on: 20200102",
            "At: 143015",
            "Stamped: 20200102143015",
            "Seen by: Nobody^Madeup",
            "Refers to: 1.2.3.4",
        ]
    );
    assert!(report.lines.iter().all(|line| line.heading.is_none()));
    assert!(!report.cut_short);
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
