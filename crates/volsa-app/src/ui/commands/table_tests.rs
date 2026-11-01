//! The entries agree with what they are built from: the brightness presets, the sizes of the
//! interface, the most cuts there can be, and what the bars draw.

use volsa_render::MAX_CUTS;
use volsa_volume::PRESETS;

use super::super::keys::Held;
use super::super::size::{SIZES, percent};
use super::table::no_3d_view as pictures_no_3d_view;
use super::*;
use crate::scene::SceneStatus;

/// The size an entry asks for, if it asks for one.
fn size_of(command: &Command) -> Option<f32> {
    match command.does {
        Does::Ask(Request::Size(size)) => Some(size),
        _ => None,
    }
}

#[test]
fn each_brightness_preset_is_an_entry_that_says_what_it_does() {
    let ids = ["window.auto", "window.soft", "window.hard"];
    let state = crate::testing::thick_scan_ready();
    for ((preset, named), id) in PRESETS.iter().enumerate().zip(ids) {
        let command = find(id).expect("listed");
        assert_eq!(command.label, named.name);
        let says = format!(
            "From the {} to the {} percentile of the slice shown",
            named.low_pct, named.high_pct
        );
        assert_eq!(command.explains, says);
        let Does::Act(act) = command.does else {
            panic!("{id} changes the state");
        };
        let applied = Action::ApplyPreset {
            viewport: 0,
            preset,
        };
        assert_eq!(act(&state), Some(applied));
    }
}

#[test]
fn each_size_the_menu_offers_is_an_entry_labelled_as_it_is_said() {
    let sizes: Vec<&Command> = all()
        .iter()
        .filter(|command| size_of(command).is_some())
        .collect();
    assert_eq!(sizes.len(), SIZES.len());
    for (command, size) in sizes.iter().zip(SIZES) {
        assert_eq!(size_of(command).map(percent), Some(percent(size)));
        assert_eq!(command.label, percent(size));
        assert_eq!(command.place, Place::Menu);
    }
}

#[test]
fn each_cut_there_can_be_has_an_entry_that_removes_it() {
    let state = AppState::default();
    for cut in 0..MAX_CUTS {
        let id = format!("scene.uncut.{}", cut + 1);
        let command = find(&id).expect("listed");
        assert!((command.why_not)(&state).is_some(), "there is no cut yet");
        let Does::Act(act) = command.does else {
            panic!("{id} changes the state");
        };
        assert_eq!(act(&state), Some(Action::CutRemove { cut }));
    }
    assert!(find(&format!("scene.uncut.{}", MAX_CUTS + 1)).is_none());
}

#[test]
fn an_action_that_cannot_be_used_says_why_in_a_sentence() {
    let nothing = AppState::default();
    let why = |id| (find(id).expect("listed").why_not)(&nothing);
    let no_scale = crate::measuring::NO_SCALE;
    assert_eq!(why("measure.ruler").as_deref(), Some(no_scale));
    assert_eq!(
        why("scene.save.picture").as_deref(),
        Some("There is no 3D view to save yet.")
    );
    // A scan shown that is not one of several says so; with nothing shown, there is no scan.
    let shown = crate::testing::thick_scan_ready();
    let volumes = (find("volume.before").expect("listed").why_not)(&shown);
    assert!(volumes.is_some_and(|said| said.contains("several volumes")));
    assert_eq!(
        why("volume.before").as_deref(),
        Some("Open a scan to see it here.")
    );
    assert_eq!(why("layout.two"), None);
}

/// States in which actions give their reasons: nothing open; a scan shown; its 3D view cut
/// as many times as it can be; the labelled parts drawn as solids; the first and the last
/// volume of a scan that holds several; and a picture in colour, which has no 3D view.
fn states_that_give_reasons() -> Vec<AppState> {
    use crate::reducer::reduce;
    use crate::testing::{
        COLOUR, FIRST_ECHO, SECOND_ECHO, colour_files, id, indexed, labelled_head,
        thick_scan_ready, three_series_files, two_echoes_files,
    };
    let mut cut = thick_scan_ready();
    for _ in 0..MAX_CUTS {
        reduce(&mut cut, Action::CutAdd);
    }
    let (mut solid, _) = labelled_head();
    reduce(&mut solid, Action::SceneLook(volsa_render::Look::Parts));
    let on = |files: Vec<(String, Vec<u8>)>, shown: &str| {
        let mut state = AppState::default();
        reduce(&mut state, Action::IndexProgress(indexed(files)));
        let (viewport, series) = (0, id(shown));
        reduce(&mut state, Action::SelectSeries { viewport, series });
        state
    };
    let echoes = || [three_series_files(), two_echoes_files()].concat();
    let mut covered = thick_scan_ready();
    covered.document = Some(crate::state::OpenDocument {
        entry: "DOCS/D0001".to_owned(),
        report: None,
    });
    let mut running = thick_scan_ready();
    running.labelling.running = Some(volsa_segment::Stage::Preparing);
    let mut kept = thick_scan_ready();
    kept.labelling.kept = Some("2099-01-02".to_owned());
    vec![
        covered,
        running,
        kept,
        AppState::default(),
        thick_scan_ready(),
        cut,
        solid,
        on(echoes(), FIRST_ECHO),
        on(echoes(), SECOND_ECHO),
        on(colour_files(), COLOUR),
    ]
}

#[test]
fn every_reason_an_action_cannot_be_used_ends_with_a_full_stop() {
    // Search posts a reason as a notice, which is a sentence.
    let mut said = std::collections::HashSet::new();
    for state in states_that_give_reasons() {
        for command in all() {
            if let Some(why) = (command.why_not)(&state) {
                assert!(why.ends_with('.'), "{}: {why}", command.id);
                said.insert(why);
            }
        }
    }
    // The states reach the reasons that were without a stop.
    for reached in [
        "Open a scan to see it here.",
        "A document is open: close it to see the pictures.",
        "No labelling is running.",
        "No labels are kept for this series.",
        "No labels are open on this series.",
        "There are no notices.",
        "There is nothing to take back.",
        "There is nothing to do again.",
        "Three cuts at most.",
        "There is no cut 1.",
        "There is no cut 3.",
        "This is the first volume.",
        "This is the last volume.",
        "The scan shown is not one of several volumes.",
        "There is no 3D view to save yet.",
        "The slice is a picture of the scan: it is not drawn among the labelled parts.",
        "Open labels in the Labels tab to see them as solids here.",
        "There is no tool in hand to put down.",
    ] {
        assert!(said.contains(reached), "no state gives: {reached}");
    }
    // And a reason that says what the scan itself says.
    let no_view = said
        .iter()
        .find(|why| why.starts_with("This scan has no 3D view: "));
    assert!(no_view.is_some(), "{said:?}");
}

#[test]
fn each_ct_window_is_an_entry_that_says_its_numbers_in_hu() {
    use crate::profile::CT_WINDOWS;
    let ids = [
        "window.brain",
        "window.soft_tissue",
        "window.lung",
        "window.bone",
    ];
    let state = crate::testing::showing_a_scan_of("CT", None);
    for (window, id) in CT_WINDOWS.iter().zip(ids) {
        let command = find(id).expect("listed");
        assert_eq!(command.label, window.name);
        let (width, level) = (window.wl.width, window.wl.center);
        assert_eq!(command.explains, format!("Width {width}, level {level} HU"));
        assert_eq!(command.profiles, [Profile::Ct], "{id}");
        assert!(offered(command, &state));
        assert_eq!((command.why_not)(&state), None, "{id}");
        let Does::Act(act) = command.does else {
            panic!("{id} changes the state");
        };
        let applied = Action::SetWindow {
            viewport: 0,
            wl: window.wl,
        };
        assert_eq!(act(&state), Some(applied));
    }
}

#[test]
fn the_ct_windows_are_for_ct_and_the_percentile_presets_for_mr() {
    let ct_only = [
        "window.brain",
        "window.soft_tissue",
        "window.lung",
        "window.bone",
    ];
    let ct = crate::testing::showing_a_scan_of("CT", None);
    let mr = crate::testing::showing_a_scan_of("MR", None);
    let basic = crate::testing::showing_a_scan_of("US", None);
    for id in ct_only {
        let command = find(id).expect("listed");
        assert!(offered(command, &ct) && !offered(command, &mr), "{id}");
        assert!(!offered(command, &basic) && !offered(command, &AppState::default()));
    }
    for id in ["window.soft", "window.hard"] {
        let command = find(id).expect("listed");
        assert!(offered(command, &mr), "{id}");
        assert!(!offered(command, &ct) && !offered(command, &basic), "{id}");
    }
    let auto = find("window.auto").expect("listed");
    for state in [&ct, &mr, &basic, &AppState::default()] {
        assert!(offered(auto, state));
    }
}

#[test]
fn a_ct_window_is_greyed_with_a_reason_for_a_ct_whose_values_are_not_in_hu() {
    let in_hu = crate::testing::showing_a_scan_of("CT", Some("HU"));
    let unspecified = crate::testing::showing_a_scan_of("CT", Some("US"));
    let named = crate::testing::showing_a_scan_of("CT", Some("MGML"));
    let brain = find("window.brain").expect("listed");
    assert_eq!((brain.why_not)(&in_hu), None);
    for state in [&unspecified, &named] {
        let why = (brain.why_not)(state).expect("greyed");
        assert!(why.contains("not in Hounsfield units"), "{why}");
    }
}

/// The ids of the controls and keys of the pictures: they act on the picture of the viewport
/// the keys act on, which is not there to be acted on without a scan or under a document.
const PICTURE_CONTROLS: [&str; 13] = [
    "view.flip_across",
    "view.flip_down",
    "view.turn",
    "view.fit",
    "slice.play",
    "slice.before",
    "slice.after",
    "slice.ten_back",
    "slice.ten_on",
    "slice.first",
    "slice.last",
    "window.auto",
    "volume.before",
];

#[test]
fn the_controls_of_the_pictures_say_why_they_do_nothing_with_no_scan_or_under_a_document() {
    let why = |id: &str, state: &AppState| (find(id).expect("listed").why_not)(state);
    let nothing = AppState::default();
    for id in PICTURE_CONTROLS {
        let said = why(id, &nothing);
        // A scan of several volumes says it is not one when asked about the volumes.
        let first = said.as_deref().unwrap_or_default();
        assert_eq!(first, "Open a scan to see it here.", "{id}");
    }
    // Under a document the pictures are out of sight, and so are the layouts they are in.
    let mut covered = crate::testing::thick_scan_ready();
    covered.document = Some(crate::state::OpenDocument {
        entry: "DOCS/D0001".to_owned(),
        report: None,
    });
    let closed = "A document is open: close it to see the pictures.";
    for id in PICTURE_CONTROLS
        .into_iter()
        .chain(["layout.one", "layout.two", "layout.four"])
    {
        assert_eq!(why(id, &covered).as_deref(), Some(closed), "{id}");
    }
    // With a scan shown and nothing over it they work, as they always have.
    let shown = crate::testing::thick_scan_ready();
    for id in PICTURE_CONTROLS
        .into_iter()
        .filter(|id| !id.starts_with("volume"))
    {
        assert_eq!(why(id, &shown), None, "{id}");
    }
    // And so do their keys: they do nothing while the control is greyed.
    let keyed = |state: &AppState| crate::ui::keys::pressed(egui::Key::H, Held::default(), state);
    assert!(keyed(&nothing).is_none() && keyed(&covered).is_none());
    assert!(keyed(&shown).is_some());
}

#[test]
fn the_looks_and_sides_of_the_scene_say_why_when_the_scan_has_no_3d_view() {
    let colour = states_that_give_reasons()
        .pop()
        .expect("the picture in colour is last");
    let ids = [
        "scene.look.surface",
        "scene.look.vr",
        "scene.look.mip",
        "scene.look.parts",
        "scene.side.anterior",
        "scene.side.posterior",
        "scene.side.left",
        "scene.side.right",
        "scene.side.superior",
        "scene.side.inferior",
        "scene.cut",
        "scene.slice",
    ];
    for id in ids {
        let said = (find(id).expect("listed").why_not)(&colour);
        let said = said.expect(id);
        assert!(said.starts_with("This scan has no 3D view"), "{id}: {said}");
    }
    // Where there is one, they can be used.
    let ready = crate::testing::thick_scan_ready();
    for id in ["scene.look.surface", "scene.side.left", "scene.slice"] {
        assert_eq!((find(id).expect("listed").why_not)(&ready), None, "{id}");
    }
}

#[test]
fn a_scan_with_no_3d_view_whose_first_reason_is_blank_says_the_first_reason_that_is_not() {
    let mut state = AppState::default();
    state.scene.status = SceneStatus::Unavailable(vec![
        String::new(),
        "  ".to_owned(),
        "It is a picture".to_owned(),
    ]);
    let said = pictures_no_3d_view(&state).expect("a reason");
    assert_eq!(said, "This scan has no 3D view: it is a picture.");
    state.scene.status = SceneStatus::Unavailable(vec![String::new()]);
    let said = pictures_no_3d_view(&state).expect("a reason");
    assert_eq!(said, "This scan has no 3D view.");
}

#[test]
fn the_labelling_entries_say_what_they_do_and_their_texts_match_what_they_are_made_from() {
    // The hover text of the automatic model says how thick is thick, as the model's own rule does.
    let automatic = find("tools.model.automatic").expect("listed");
    let thick = format!("{}", volsa_segment::THICK_MM);
    assert!(
        automatic.explains.contains(&format!("{thick} mm")),
        "{}",
        automatic.explains
    );
    for (id, wanted) in [
        ("tools.label.stop", Some("No labelling is running.")),
        (
            "tools.label.show_kept",
            Some("No labels are kept for this series."),
        ),
        (
            "tools.label.delete_kept",
            Some("No labels are kept for this series."),
        ),
        ("tools.label.cortex", None),
        ("tools.label.keep", None),
        ("tools.model.robust", None),
    ] {
        let said = (find(id).expect("listed").why_not)(&AppState::default());
        assert_eq!(said.as_deref(), wanted, "{id}");
    }
}
