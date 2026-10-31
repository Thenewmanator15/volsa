//! The lines of a report, drawn as far as they are in sight.
//!
//! A report can run to 5,000 lines, so the window lays out and prepares only the lines in sight
//! and gives the room of the rest as empty space. What a line is drawn with (its words with the
//! patient's name left out while their details are hidden) is made when it first comes in
//! sight and kept. The height of each line, which depends on how its words wrap at the width
//! it is drawn at, is measured when it is first drawn and kept; until then it is a guess from
//! how long it is. Nothing here grows with the length of the report from one frame to the
//! next except adding up the heights when one has been measured.

use egui::{Label, RichText, ScrollArea, Ui};
use voxa_dicom::{IndexedDocument, Report, ReportLine};

use super::INDENT;
use crate::documents::Mask;
use crate::state::AppState;

/// About how wide a character of the body type is, in points: what a line that has not been
/// drawn yet is guessed to wrap by.
const CHARACTER: f32 = 7.0;

/// What a report is drawn from.
pub struct Shown<'a> {
    /// The report, whose lines are drawn.
    pub report: &'a Report,
    /// The document it is in.
    pub document: &'a IndexedDocument,
    /// What the window knows: which export is open and whether the patient's details are shown.
    pub state: &'a AppState,
}

/// What a line is drawn with.
#[derive(Debug)]
struct Prepared {
    heading: Option<String>,
    text: String,
}

/// What what is kept was made for. Another report, another export or a change of whether the
/// patient's details are shown makes it all again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Content {
    entry: String,
    /// How many times the export's index had changed.
    index: u64,
    lines: usize,
    hiding: bool,
}

/// What is kept of the lines of the report shown from one frame to the next.
#[derive(Debug, Default)]
pub struct Kept {
    /// What the lines and their masks were made for.
    content: Option<Content>,
    /// What is left out of the words of the lines.
    mask: Mask,
    /// What each line is drawn with, once it has come in sight.
    prepared: Vec<Option<Prepared>>,
    /// The width the heights were measured or guessed for, in whole points.
    width: f32,
    /// How much room each line takes, its gap to the next included: measured or guessed.
    heights: Vec<f32>,
    /// Where each line begins from the top of the lines, and last, where the last ends.
    tops: Vec<f32>,
    /// How many lines were prepared, in all.
    #[cfg(test)]
    pub(super) prepared_count: usize,
    /// How many lines were drawn in the last frame.
    #[cfg(test)]
    pub(super) drawn_count: usize,
}

impl Kept {
    /// Make again whatever what is shown no longer matches: the words of the lines when the
    /// report or whether details are hidden has changed, and the heights when the width has.
    fn fit(&mut self, shown: &Shown<'_>, ui: &Ui) {
        let (state, lines) = (shown.state, &shown.report.lines);
        let content = Content {
            entry: shown.document.entry.clone(),
            index: state.index_changes,
            lines: lines.len(),
            hiding: !state.show_patient,
        };
        if self.content.as_ref() != Some(&content) {
            self.content = Some(content);
            self.mask = Mask::for_document(state, shown.document);
            self.prepared = lines.iter().map(|_| None).collect();
            self.width = 0.0;
        }
        let width = ui.available_width().floor().max(1.0);
        if (width - self.width).abs() >= 1.0 {
            self.width = width;
            let room = (ui.spacing().interact_size.y, ui.spacing().item_spacing.y);
            let guessed = lines.iter().map(|line| guess(line, width, room));
            self.heights = guessed.collect();
            self.settle();
        }
    }

    /// Add up the heights again.
    fn settle(&mut self) {
        self.tops.clear();
        let mut top = 0.0;
        self.tops.push(top);
        for height in &self.heights {
            top += height;
            self.tops.push(top);
        }
    }

    /// Where line `at` begins, from the top of the lines.
    fn top(&self, at: usize) -> f32 {
        self.tops.get(at).copied().unwrap_or(0.0)
    }

    /// How tall the lines are together.
    fn height(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    /// The place of the line the height `y` from the top of the lines is in.
    fn line_at(&self, y: f32) -> usize {
        self.tops.partition_point(|top| *top <= y).saturating_sub(1)
    }

    /// What line `at` of the report is drawn with, made now if it has not been.
    fn prepared(&mut self, at: usize, report: &Report) -> Option<&Prepared> {
        let line = report.lines.get(at)?;
        let mask = &self.mask;
        #[cfg(test)]
        let counted = &mut self.prepared_count;
        let kept = self.prepared.get_mut(at)?;
        Some(kept.get_or_insert_with(|| {
            #[cfg(test)]
            {
                *counted += 1;
            }
            Prepared {
                heading: line
                    .heading
                    .as_deref()
                    .map(|heading| mask.words(heading, false).into_owned()),
                text: mask.words(&line.text, line.is_name).into_owned(),
            }
        }))
    }

    /// Note how much room line `at` took when it was drawn; whether that was not what was
    /// thought, which means the heights are to be added up again.
    fn measured(&mut self, at: usize, height: f32) -> bool {
        match self.heights.get_mut(at) {
            Some(held) if (*held - height).abs() > 0.5 => {
                *held = height;
                true
            }
            _ => false,
        }
    }
}

/// A guess at how much room a line takes at `width` points across: its rows of characters, one
/// row at least, and the gap beneath.
fn guess(line: &ReportLine, width: f32, (row, gap): (f32, f32)) -> f32 {
    let room = (width - INDENT * f32::from(line.depth)).max(CHARACTER);
    let length = line.heading.as_deref().map_or(0, str::len) + line.text.len();
    let rows = (as_points(length) * CHARACTER / room).ceil().max(1.0);
    rows * row + gap
}

/// A count of bytes as a number of points, as near as an `f32` holds it.
#[expect(
    clippy::cast_precision_loss,
    reason = "a report holds at most a million characters, which an f32 holds exactly enough for a guess"
)]
fn as_points(count: usize) -> f32 {
    count as f32
}

/// Draw the lines of a report in a scroll area that fills what is left of `ui`, drawing only
/// those in sight.
pub fn show(ui: &mut Ui, shown: &Shown<'_>, kept: &mut Kept) {
    ScrollArea::vertical()
        .id_salt("voxa: the lines of a report")
        .auto_shrink(false)
        .show_viewport(ui, |ui, view| {
            kept.fit(shown, ui);
            #[cfg(test)]
            {
                kept.drawn_count = 0;
            }
            let (first, gap) = (kept.line_at(view.min.y), ui.spacing().item_spacing.y);
            ui.add_space(kept.top(first));
            let (mut at, mut y, mut changed) = (first, kept.top(first), false);
            while y < view.max.y {
                let Some(depth) = shown.report.lines.get(at).map(|line| line.depth) else {
                    break;
                };
                let Some(words) = kept.prepared(at, shown.report) else {
                    break;
                };
                let height = draw(ui, depth, words) + gap;
                changed |= kept.measured(at, height);
                #[cfg(test)]
                {
                    kept.drawn_count += 1;
                }
                (at, y) = (at + 1, y + height);
            }
            if changed {
                kept.settle();
            }
            ui.add_space((kept.height() - kept.top(at)).max(0.0));
        });
}

/// Draw one line, moved to the right by how deep it lies; gives how tall it is. The words wrap
/// at the edge of the room, and can be selected.
fn draw(ui: &mut Ui, depth: u8, words: &Prepared) -> f32 {
    let row = ui.horizontal(|ui| {
        ui.add_space(INDENT * f32::from(depth));
        if let Some(heading) = &words.heading {
            ui.add(Label::new(RichText::new(heading).strong()).wrap());
        }
        if !words.text.is_empty() {
            ui.add(Label::new(&words.text).wrap());
        }
    });
    row.response.rect.height()
}

#[cfg(test)]
mod tests;
