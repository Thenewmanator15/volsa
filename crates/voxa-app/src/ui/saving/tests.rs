//! What is offered and written when the 3D scene, a table of measurements or a report is saved:
//! never anything of the patient, whether their details are shown or not.

use std::path::PathBuf;

use super::*;
use crate::state::AppState;
use crate::testing::patient::said_of;
use crate::testing::{a_render_state, cube, thick_scan_ready, three_series};
use crate::ui::scene::scene_params;

/// Every source file of the application, with its text.
fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut folders = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("lists the sources") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|ending| ending == "rs") {
                let text = std::fs::read_to_string(&path).expect("reads a source");
                found.push((path, text));
            }
        }
    }
    found
}

/// What stands between the parenthesis that `text` follows and its closing one.
fn argument(text: &str) -> &str {
    let mut depth = 1;
    for (at, letter) in text.char_indices() {
        match letter {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return text.get(..at).unwrap_or_default().trim();
        }
    }
    text
}

/// The text a constant of this name is set to, in whichever source declares it.
fn value_of(name: &str, sources: &[(PathBuf, String)]) -> String {
    let declared = format!("const {name}: &str = \"");
    let found = sources.iter().find_map(|(_, text)| {
        let (_, after) = text.split_once(&declared)?;
        Some(after.split_once('"')?.0.to_owned())
    });
    found.unwrap_or_else(|| panic!("no constant {name}"))
}

#[test]
fn every_name_offered_for_a_saved_file_is_one_of_voxas_own() {
    // The needles are written in pieces, so that this file does not hold them.
    let offering = [".set_file", "_name("].concat();
    let saving = [".save", "_file()"].concat();
    let sources = sources();
    let mut offered = Vec::new();
    let mut dialogs = 0;
    for (_, text) in &sources {
        dialogs += text.matches(saving.as_str()).count();
        for (at, _) in text.match_indices(offering.as_str()) {
            let after = text.get(at + offering.len()..).unwrap_or_default();
            offered.push(argument(after).to_owned());
        }
    }
    // Each is a constant of Voxa's, alone or with an ending, and each dialog offers one: a new
    // way of saving something is added to this list only after what it offers has been read.
    let constants = [
        ("SUGGESTED_PDF", "SUGGESTED_PDF"),
        ("FILE_NAME", "FILE_NAME"),
        ("PARTS_NAME", "format!(\"{PARTS_NAME}.csv\")"),
        ("SUGGESTED_NAME", "format!(\"{SUGGESTED_NAME}.{ending}\")"),
        (
            "MEASUREMENTS_NAME",
            "format!(\"{MEASUREMENTS_NAME}.{ending}\")",
        ),
    ];
    let mut wanted: Vec<String> = constants
        .iter()
        .map(|(_, used)| (*used).to_owned())
        .collect();
    wanted.sort();
    offered.sort();
    assert_eq!(offered, wanted);
    assert_eq!(dialogs, offered.len(), "a dialog that offers no name");
    // And none of them holds anything the made-up export says of its patient or its scans.
    let said = said_of(&three_series());
    for (constant, _) in constants {
        let value = value_of(constant, &sources).to_lowercase();
        for text in &said {
            assert!(
                !value.contains(&text.to_lowercase()),
                "{constant} holds {text}"
            );
        }
        let plain = value
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || "-.".contains(letter));
        assert!(plain && !value.is_empty(), "{constant}: {value}");
    }
}

#[test]
fn a_saved_picture_of_the_scene_is_the_same_whether_patient_details_are_shown_or_not() {
    let hidden = thick_scan_ready();
    let mut shown = hidden.clone();
    shown.show_patient = true;
    assert!(!hidden.show_patient);
    let named = |state: &AppState| {
        let index = state.index.as_ref();
        index.is_some_and(|index| {
            index
                .studies
                .iter()
                .all(|study| study.patient_name.is_some())
        })
    };
    assert!(named(&hidden), "the made-up export names its patient");
    // What the picture is drawn from holds no letter of the patient's.
    let (without, with) = (scene_params(&hidden, 1.0), scene_params(&shown, 1.0));
    assert!(without.is_some());
    assert_eq!(without, with);
    // And so the picture drawn is the same, byte for byte.
    let Some(render) = a_render_state() else {
        return;
    };
    let (mut view, volume) = (scene::SceneView::new(Some(&render)), cube(8, 1.0));
    let _ = view.keep_on_card(Some(&render), Some(&volume));
    assert!(!view.needs(&volume), "the card holds the volume");
    let viewport = [64.0, 48.0];
    let pixels = [64, 48];
    let mut pictures = Vec::new();
    for params in [without, with].into_iter().flatten() {
        let drawn = view.snapshot(Some(&render), &params, viewport, pixels);
        pictures.push(drawn.expect("a device").expect("drawn"));
    }
    assert_eq!(pictures.len(), 2);
    assert_eq!(pictures[0].len(), 64 * 48 * 4);
    assert_eq!(pictures[0], pictures[1]);
}
