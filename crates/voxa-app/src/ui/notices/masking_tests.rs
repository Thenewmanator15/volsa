//! The notices never name the patient while their details are hidden: not on the line, not in
//! the list, not in the hover text nor the name a screen reader is given.

use super::tests::show_state;
use super::*;
use crate::ui::seen::{self, Seen};

/// A state with an export open whose patient is the made-up one, and notices that name them:
/// the way a folder named for the patient makes a skipped file's path do.
fn naming_the_patient(show_patient: bool) -> AppState {
    let text = "Could not read Phantom^Test.png in PHANTOM-001";
    let details = ["Phantom^Test.txt", "PHANTOM-001 notes.txt", "plain.bin"];
    let notice = Notice {
        kind: NoticeKind::Info,
        text: text.to_owned(),
        details: details.map(str::to_owned).to_vec(),
        key: None,
    };
    AppState {
        index: Some(crate::testing::documents::scans_and_a_named_report()),
        notices: vec![notice],
        show_patient,
        ..AppState::default()
    }
}

/// Everything a pass of the line and its list drew or told a screen reader, in lower case.
fn everything_of(state: &AppState) -> String {
    let ctx = seen::context(1.0);
    let mut last = Seen::default();
    let mut open = true;
    for _ in 0..3 {
        let ((), seen) = seen::pass(&ctx, [900.0, 600.0], Vec::new(), |ui| {
            egui::Panel::bottom("line").show(ui, |ui| {
                show_state(ui, state, &mut open, &mut Vec::new());
            });
        });
        last = seen;
    }
    let words = last.words.iter().map(|(word, _)| word.to_lowercase());
    let names = last.names.iter().map(|(name, _)| name.to_lowercase());
    words.chain(names).collect::<Vec<_>>().join("\n")
}

#[test]
fn while_the_details_are_hidden_no_notice_names_the_patient_on_the_line_or_in_the_list() {
    let hidden = naming_the_patient(false);
    let said = everything_of(&hidden);
    assert!(!said.contains("phantom"), "{said}");
    assert!(
        said.contains("could not read [hidden].png in [hidden]"),
        "{said}"
    );
    assert!(
        said.contains("plain.bin"),
        "what names nobody is as it was: {said}"
    );
    // The hover text and the name a screen reader is given are made from the same words.
    let shown = Said::of(&hidden).notice(&hidden.notices[0]);
    assert_eq!(shown.text, "Could not read [hidden].png in [hidden]");
    assert_eq!(
        shown.details,
        ["[hidden].txt", "[hidden] notes.txt", "plain.bin"]
    );
    // With the details on, as written.
    let said = everything_of(&naming_the_patient(true));
    assert!(said.contains("phantom^test.png"), "{said}");
}
