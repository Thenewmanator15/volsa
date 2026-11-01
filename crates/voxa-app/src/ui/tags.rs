//! The tag browser: every element of one file, the one behind the slice being shown, the
//! first of a series whose card asked for it, or a document's file.

use egui::RichText;
use voxa_dicom::{TagRow, is_patient_identifying};

use super::{access, floating, theme};
use crate::action::Action;
use crate::state::AppState;

mod panel;

pub use panel::Panel;

/// What stands in for the value of an element that could identify the patient.
pub const HIDDEN: &str = "hidden";
/// What the values hidden while the patient's details are hidden are.
const HIDDEN_KINDS: &str = "that could identify the patient: the patient's own, the people and \
                            the place of the scan, the numbers of its order, and makers' \
                            private values.";
/// Where the patient's details are shown again.
const SHOWS_THEM: &str = "\"Show patient details\" in the View tab shows them.";

/// The title of the tag browser window.
const TITLE: &str = "DICOM tags";

/// The rows of a file, or why they could not be listed.
pub type Listing = Result<Vec<TagRow>, String>;

/// One element as the tag browser shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShownTag<'a> {
    /// The element's number, as `(0010,0010)`.
    pub tag: &'a str,
    /// Its name in the DICOM standard.
    pub keyword: &'a str,
    /// Its value representation.
    pub vr: &'a str,
    /// Its value, or [`HIDDEN`] for an element that could identify the patient.
    pub value: &'a str,
}

/// The rows to show: those whose number, name or value contains `query`, whatever its case.
/// An element that could identify the patient (see [`is_patient_identifying`]) shows
/// [`HIDDEN`] in place of its value unless `show_patient` is true, and it is that stand-in the
/// search sees, so a name cannot be found by typing it.
pub fn filter_tags<'a>(rows: &'a [TagRow], query: &str, show_patient: bool) -> Vec<ShownTag<'a>> {
    let wanted = query.trim().to_lowercase();
    let shown = rows.iter().map(|row| {
        let private = !show_patient && is_patient_identifying(&row.tag);
        let value = if private { HIDDEN } else { row.value.as_str() };
        ShownTag {
            tag: &row.tag,
            keyword: &row.keyword,
            vr: &row.vr,
            value,
        }
    });
    let matches = |tag: &ShownTag<'_>| {
        let within = |text: &str| text.to_lowercase().contains(&wanted);
        wanted.is_empty() || within(tag.tag) || within(tag.keyword) || within(tag.value)
    };
    shown.filter(matches).collect()
}

/// What the window says while the patient's details are hidden: how many values of the file
/// in `rows` are hidden, and which kinds, or only that the details are hidden when there are
/// no rows to count.
pub fn hidden_note(rows: &[TagRow]) -> String {
    let hidden = rows
        .iter()
        .filter(|row| is_patient_identifying(&row.tag))
        .count();
    match hidden {
        0 => format!("The patient's details are hidden. {SHOWS_THEM}"),
        1 => format!("1 value is hidden {HIDDEN_KINDS} {SHOWS_THEM}"),
        many => format!("{many} values are hidden {HIDDEN_KINDS} {SHOWS_THEM}"),
    }
}

/// The interface's own name for the tag browser window, which it keeps its place under.
pub(super) fn id() -> egui::Id {
    egui::Id::new("voxa: tags")
}

/// The tag browser window: a search field, then the rows that match it.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    state: &AppState,
    listing: &Listing,
    actions: &mut Vec<Action>,
) {
    floating::over(egui::Window::new(TITLE), ctx)
        .id(id())
        .open(open)
        .default_size([620.0, 440.0])
        .show(ctx, |ui| {
            let mut filter = state.tag_filter.clone();
            ui.horizontal(|ui| {
                ui.label("Search");
                let field = ui.text_edit_singleline(&mut filter);
                access::call(&field, "Search the tags");
                if field.changed() {
                    actions.push(Action::SetTagFilter(filter.clone()));
                }
            });
            if !state.show_patient {
                let note = hidden_note(listing.as_deref().unwrap_or_default());
                ui.label(
                    RichText::new(note)
                        .small()
                        .color(theme::palette(ui.ctx()).quiet_text),
                );
            }
            match listing {
                Err(why) => {
                    ui.label(RichText::new(why).color(theme::palette(ui.ctx()).warning));
                }
                Ok(rows) if rows.is_empty() => {
                    ui.label("Show a series to see the tags of the file behind the slice.");
                }
                Ok(rows) => table(
                    ui,
                    &filter_tags(rows, &state.tag_filter, state.show_patient),
                ),
            }
        });
}

fn table(ui: &mut egui::Ui, shown: &[ShownTag<'_>]) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::Grid::new("tags")
                .num_columns(4)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for tag in shown {
                        ui.label(RichText::new(tag.tag).monospace());
                        ui.label(tag.keyword);
                        ui.label(
                            RichText::new(tag.vr)
                                .monospace()
                                .color(theme::palette(ui.ctx()).quiet_text),
                        );
                        ui.label(tag.value);
                        ui.end_row();
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(tag: &str, keyword: &str, vr: &str, value: &str) -> TagRow {
        TagRow {
            tag: tag.to_owned(),
            keyword: keyword.to_owned(),
            vr: vr.to_owned(),
            value: value.to_owned(),
        }
    }

    fn rows() -> Vec<TagRow> {
        vec![
            row("(0008,0060)", "Modality", "CS", "MR"),
            row("(0008,103E)", "SeriesDescription", "LO", "made-up axial"),
            row("(0010,0010)", "PatientName", "PN", "Phantom^Test"),
            row("(0010,0020)", "PatientID", "LO", "PHANTOM-001"),
            row("(0028,0010)", "Rows", "US", "512"),
        ]
    }

    fn keywords(shown: &[ShownTag<'_>]) -> Vec<String> {
        shown.iter().map(|tag| tag.keyword.to_owned()).collect()
    }

    #[test]
    fn a_search_matches_the_keyword_whatever_its_case() {
        let rows = rows();
        assert_eq!(keywords(&filter_tags(&rows, "modal", false)), ["Modality"]);
        assert_eq!(keywords(&filter_tags(&rows, "  ROWS ", false)), ["Rows"]);
    }

    #[test]
    fn a_search_matches_the_tag_number_and_the_value_too() {
        let rows = rows();
        assert_eq!(keywords(&filter_tags(&rows, "0028,", false)), ["Rows"]);
        assert_eq!(
            keywords(&filter_tags(&rows, "AXIAL", false)),
            ["SeriesDescription"]
        );
    }

    #[test]
    fn no_search_lists_every_element_in_order() {
        let rows = rows();
        let shown = filter_tags(&rows, "", false);
        assert_eq!(shown.len(), 5);
        assert_eq!(shown[0].tag, "(0008,0060)");
        assert_eq!((shown[4].vr, shown[4].value), ("US", "512"));
    }

    #[test]
    fn the_patients_details_are_hidden_and_cannot_be_searched_for_unless_shown() {
        let rows = rows();
        let shown = filter_tags(&rows, "", false);
        let name = shown
            .iter()
            .find(|tag| tag.keyword == "PatientName")
            .expect("listed");
        assert_eq!(name.value, HIDDEN);
        assert_eq!(HIDDEN, "hidden");
        assert_eq!(
            filter_tags(&rows, "phantom", false).len(),
            0,
            "the real name finds nothing"
        );
        assert_eq!(
            keywords(&filter_tags(&rows, "hidden", false)),
            ["PatientName", "PatientID"]
        );

        let open = filter_tags(&rows, "phantom", true);
        assert_eq!(keywords(&open), ["PatientName", "PatientID"]);
        assert_eq!(open[0].value, "Phantom^Test");
    }

    /// A file with the people and the place of the scan, and a maker's private elements.
    fn rows_with_who_and_where() -> Vec<TagRow> {
        let mut rows = rows();
        rows.extend([
            row("(0008,0050)", "AccessionNumber", "SH", "ACC-0042"),
            row("(0008,0080)", "InstitutionName", "LO", "Made-up Infirmary"),
            row("(0008,1070)", "OperatorsName", "PN", "Nobody^Operator"),
            row("(0018,1000)", "DeviceSerialNumber", "LO", "SN-31337"),
            row("(0029,0010)", "", "LO", "MADE UP CREATOR"),
            row("(0029,1010)", "", "OB", "<120 bytes>"),
        ]);
        rows
    }

    #[test]
    fn the_people_the_place_and_private_values_are_hidden_with_the_patients_details() {
        let rows = rows_with_who_and_where();
        let shown = filter_tags(&rows, "", false);
        let hidden: Vec<&str> = shown
            .iter()
            .filter(|tag| tag.value == HIDDEN)
            .map(|tag| tag.tag)
            .collect();
        let expected = [
            "(0010,0010)",
            "(0010,0020)",
            "(0008,0050)",
            "(0008,0080)",
            "(0008,1070)",
            "(0018,1000)",
            "(0029,0010)",
            "(0029,1010)",
        ];
        assert_eq!(hidden, expected);
        for word in ["infirmary", "ACC-0042", "nobody", "SN-31337", "creator"] {
            assert_eq!(filter_tags(&rows, word, false).len(), 0, "{word} is found");
        }
        let open = filter_tags(&rows, "infirmary", true);
        assert_eq!(keywords(&open), ["InstitutionName"]);
        assert_eq!(open[0].value, "Made-up Infirmary");
    }

    #[test]
    fn the_window_says_how_many_values_are_hidden() {
        let shows = "\"Show patient details\" in the View tab shows them.";
        let kinds = "that could identify the patient: the patient's own, the people and the place \
                     of the scan, the numbers of its order, and makers' private values.";
        assert_eq!(
            hidden_note(&rows_with_who_and_where()),
            format!("8 values are hidden {kinds} {shows}")
        );
        let one = [row("(0010,0010)", "PatientName", "PN", "Phantom^Test")];
        assert_eq!(
            hidden_note(&one),
            format!("1 value is hidden {kinds} {shows}")
        );
        assert_eq!(
            hidden_note(&[]),
            format!("The patient's details are hidden. {shows}")
        );
    }

    #[test]
    fn the_window_draws_with_rows_without_rows_and_with_an_error() {
        let ctx = egui::Context::default();
        let state = crate::state::AppState::default();
        for listing in [
            Ok(rows()),
            Ok(Vec::new()),
            Err("No scan is open.".to_owned()),
        ] {
            let (mut open, mut actions) = (true, Vec::new());
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                show(ui.ctx(), &mut open, &state, &listing, &mut actions);
            });
            // No renderer is behind this pass to take the font texture.
            output.textures_delta.clear();
            assert!(open && actions.is_empty());
        }
    }
}
