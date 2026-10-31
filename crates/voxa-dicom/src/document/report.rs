//! Turning a structured report into lines of text.

use std::ops::ControlFlow;

use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use voxa_core::VoxaError;

use super::{REPORT_CLASSES, sop_class_of};
use crate::decode::{guarded, open};
use crate::header::{first_item, items, text};

/// How deep in the report an item may lie, counting the sections around it: items of the
/// outermost level are at depth 0. A report nested deeper than this is read no further.
pub const MAX_REPORT_DEPTH: usize = 16;
/// The most lines a report is read into. A report with more is read no further.
pub const MAX_REPORT_LINES: usize = 5_000;
/// The most characters a report is read into, all its lines together. A report with more is
/// read no further.
pub const MAX_REPORT_CHARS: usize = 1_000_000;

/// One line of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLine {
    /// How many sections this line is inside; 0 for the outermost level.
    pub depth: u8,
    /// The section's name, for a line that opens a section; `None` for any other line.
    pub heading: Option<String>,
    /// What the line says: `name: value` for an item, and empty for a section's opening line.
    pub text: String,
}

/// A report as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The lines, in the order of the report.
    pub lines: Vec<ReportLine>,
    /// True when the report went on past a limit and was read no further: the lines are the
    /// start of it, and the rest is only in the original.
    pub cut_short: bool,
}

/// Read a structured report as lines of text, in the order of the report.
///
/// A section opens a line of its own, headed by its name. An item that is text, a code, a
/// number with its unit, a date, a time, a name or an identifier becomes the line
/// `name: value`. An item of any other kind, such as an image, becomes a line that says what
/// it was and that it is not shown, so that nothing is left out unsaid. The report is read
/// no further than [`MAX_REPORT_DEPTH`] sections deep, [`MAX_REPORT_LINES`] lines or
/// [`MAX_REPORT_CHARS`] characters; it is then [`Report::cut_short`].
///
/// # Errors
///
/// [`VoxaError::NotDicom`] when the bytes are not a DICOM file, and [`VoxaError::Decode`] when
/// the file is damaged or cut short, or is not a structured report.
pub fn read_report(bytes: &[u8]) -> Result<Report, VoxaError> {
    guarded(|| {
        let file = open(bytes)?;
        if !sop_class_of(&file).is_some_and(|class| class.starts_with(REPORT_CLASSES)) {
            return Err(VoxaError::Decode(
                "this is not a structured report".to_owned(),
            ));
        }
        let mut read = Reading::default();
        let _ = read.items_of(&file, 0);
        Ok(Report {
            lines: read.lines,
            cut_short: read.cut_short,
        })
    })
}

/// A report being read.
#[derive(Default)]
struct Reading {
    lines: Vec<ReportLine>,
    /// The characters of all the lines so far.
    chars: usize,
    cut_short: bool,
}

impl Reading {
    /// Read the items under `parent`, whose depth is `depth`. Breaks when a limit is reached,
    /// and nothing more is to be read.
    fn items_of(&mut self, parent: &InMemDicomObject, depth: usize) -> ControlFlow<()> {
        let under = items(parent, tags::CONTENT_SEQUENCE);
        if under.is_empty() {
            return ControlFlow::Continue(());
        }
        if depth > MAX_REPORT_DEPTH {
            self.cut_short = true;
            return ControlFlow::Break(());
        }
        for item in under {
            self.push(line_of(item, depth))?;
            self.items_of(item, depth + 1)?;
        }
        ControlFlow::Continue(())
    }

    /// Add a line, or as much of it as the limits leave room for.
    fn push(&mut self, mut line: ReportLine) -> ControlFlow<()> {
        if self.lines.len() >= MAX_REPORT_LINES {
            self.cut_short = true;
            return ControlFlow::Break(());
        }
        let room = MAX_REPORT_CHARS.saturating_sub(self.chars);
        let mut size = characters(&line);
        let fits = size <= room;
        if !fits {
            self.cut_short = true;
            clip(&mut line, room);
            size = characters(&line);
        }
        if fits || size > 0 {
            self.chars += size;
            self.lines.push(line);
        }
        if fits {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

/// How many characters a line holds.
fn characters(line: &ReportLine) -> usize {
    let heading = line.heading.as_deref().map_or(0, |h| h.chars().count());
    heading + line.text.chars().count()
}

/// Cut a line down to its first `room` characters.
fn clip(line: &mut ReportLine, room: usize) {
    let first = |text: &str, count: usize| text.chars().take(count).collect::<String>();
    if let Some(heading) = &mut line.heading {
        *heading = first(heading, room);
    }
    let left = room.saturating_sub(line.heading.as_deref().map_or(0, |h| h.chars().count()));
    line.text = first(&line.text, left);
}

/// The line one item of the report is.
fn line_of(item: &InMemDicomObject, depth: usize) -> ReportLine {
    let depth = u8::try_from(depth).unwrap_or(u8::MAX);
    let concept = first_item(item, tags::CONCEPT_NAME_CODE_SEQUENCE).and_then(meaning);
    let value_type = text(item, tags::VALUE_TYPE).unwrap_or_default();
    if value_type == "CONTAINER" {
        return ReportLine {
            depth,
            heading: Some(concept.unwrap_or_default()),
            text: String::new(),
        };
    }
    let value = match value_of(item, &value_type) {
        Value::Said(value) => value,
        Value::Unshown(what) => format!("{what}, not shown"),
    };
    let text = match concept {
        Some(name) => format!("{name}: {value}").trim_end().to_owned(),
        None => value,
    };
    ReportLine {
        depth,
        heading: None,
        text,
    }
}

/// What an item holds.
enum Value {
    /// A value, as text.
    Said(String),
    /// A kind of value that is not shown, described.
    Unshown(&'static str),
}

fn value_of(item: &InMemDicomObject, value_type: &str) -> Value {
    let plain = |tag| Value::Said(text(item, tag).unwrap_or_default());
    match value_type {
        "TEXT" => plain(tags::TEXT_VALUE),
        "DATE" => plain(tags::DATE),
        "TIME" => plain(tags::TIME),
        "DATETIME" => plain(tags::DATE_TIME),
        "PNAME" => plain(tags::PERSON_NAME),
        "UIDREF" => plain(tags::UID),
        "CODE" => {
            let code = first_item(item, tags::CONCEPT_CODE_SEQUENCE);
            Value::Said(code.and_then(meaning).unwrap_or_default())
        }
        "NUM" => Value::Said(number(item)),
        "IMAGE" => Value::Unshown("an image"),
        "COMPOSITE" => Value::Unshown("a reference to another object"),
        "WAVEFORM" => Value::Unshown("a waveform"),
        "SCOORD" | "SCOORD3D" => Value::Unshown("a marked region"),
        "TCOORD" => Value::Unshown("a marked time range"),
        _ => Value::Unshown("an item of a kind Voxa does not read"),
    }
}

/// A number and its unit, as the report gives them: the number, then the meaning of the
/// unit's code. An item with no number says why not, where it does.
fn number(item: &InMemDicomObject) -> String {
    let Some(measured) = first_item(item, tags::MEASURED_VALUE_SEQUENCE) else {
        let why = first_item(item, tags::NUMERIC_VALUE_QUALIFIER_CODE_SEQUENCE);
        return why.and_then(meaning).unwrap_or_default();
    };
    let value = text(measured, tags::NUMERIC_VALUE).unwrap_or_default();
    let units = first_item(measured, tags::MEASUREMENT_UNITS_CODE_SEQUENCE).and_then(meaning);
    match units {
        Some(units) => format!("{value} {units}").trim().to_owned(),
        None => value,
    }
}

/// What a code means, in the words of its code meaning.
fn meaning(code: &InMemDicomObject) -> Option<String> {
    text(code, tags::CODE_MEANING)
}
