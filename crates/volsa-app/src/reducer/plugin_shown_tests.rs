//! Labels a plugin made: drawn through the other solids in 3D when its description asks, and
//! shown, hidden and faded all at once from its entry in the Tools tab.

use std::path::PathBuf;

use super::scene_tests::{corners, ready};
use super::*;
use crate::labels::NamesFrom;
use crate::plugins::{PluginEntry, ResultShown};

/// The made-up plugin's identity.
const ID: &str = "example.vessels";

fn part(name: &str) -> volsa_labels::Part {
    volsa_labels::Part {
        name: name.to_owned(),
        group: String::new(),
        side: volsa_labels::Side::Midline,
        colour: [1, 2, 3],
        trust: volsa_labels::Trust::Model,
        voxels: 1,
        ml: 1.0,
    }
}

/// The scene ready, with the made-up plugin added (`see_through` as its description says) and
/// two parts it made open on the scene's study.
fn made_by_plugin(see_through: bool) -> AppState {
    let mut state = ready();
    let mut entry = PluginEntry::broken(ID, String::new());
    entry.id = ID.to_owned();
    entry.checked = Ok(());
    entry.see_through = see_through;
    state.plugins.entries = vec![entry];
    reduce(
        &mut state,
        Action::OpenLabels {
            labels: PathBuf::from("labels.nii.gz"),
            table: None,
        },
    );
    let made = volsa_labels::Made {
        added: Some(volsa_labels::Added {
            id: ID.to_owned(),
            ..volsa_labels::Added::default()
        }),
        ..volsa_labels::Made::default()
    };
    reduce(
        &mut state,
        Action::LabelsRead {
            title: String::new(),
            parts: vec![part("Artery"), part("Vein")],
            verdict: None,
            corners: corners(),
            names_from: NamesFrom::Table,
            made: Some(made),
            run: None,
        },
    );
    state
}

#[test]
fn a_plugins_labels_are_drawn_through_the_rest_only_when_its_description_asks() {
    let through = |state: &AppState| -> Vec<bool> {
        state
            .labels
            .looks()
            .iter()
            .map(|look| look.through)
            .collect()
    };
    assert_eq!(through(&made_by_plugin(true)), [true, true]);
    assert_eq!(through(&made_by_plugin(false)), [false, false]);
}

#[test]
fn the_plugins_switch_and_slider_work_on_every_part_it_made() {
    let mut state = made_by_plugin(false);
    assert_eq!(
        ResultShown::of(&state, "another.plugin"),
        None,
        "not another's"
    );
    let result = ResultShown::of(&state, ID).expect("its labels are open");
    assert!(result.shown && result.parts == 2);
    for action in result.show(false) {
        reduce(&mut state, action);
    }
    let hidden = ResultShown::of(&state, ID).expect("still open");
    assert!(!hidden.shown, "every part hidden");
    for action in result.show(true).into_iter().chain(result.strength(0.3)) {
        reduce(&mut state, action);
    }
    let opacity: Vec<f32> = state.labels.shown.iter().map(|how| how.opacity).collect();
    assert_eq!(opacity, [0.3, 0.3]);
}
