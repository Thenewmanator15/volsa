//! What a picture of the window can ask for.

use super::*;
use crate::testing::held_of;

#[test]
fn a_picture_can_ask_for_the_scene_to_be_posed() {
    use volsa_render::Look;
    assert_eq!(pose("tags,keys"), []);
    assert_eq!(pose("cut"), [Action::CutAdd]);
    let all = pose("about, mip, turned, cut, slice");
    let expected = [
        Action::SceneLook(Look::Mip),
        Action::SceneOrbit { by: [70.0, 35.0] },
        Action::CutAdd,
        Action::ShowSliceInScene(true),
    ];
    assert_eq!(
        all, expected,
        "in an order that does not depend on how they were asked"
    );
    assert_eq!(pose("vr"), [Action::SceneLook(Look::Vr)]);
    assert_eq!(pose("smooth"), [Action::SetSmooth(true)]);
    let next = Action::StepVolume { viewport: 0, by: 1 };
    assert_eq!(pose("stepped"), [next]);
}

#[test]
fn a_picture_can_ask_for_a_series_by_words_from_its_description() {
    let mut index = crate::testing::with_odd_series();
    let named = ["ball, fine", "ball, two echoes", "a picture in colour"];
    for (listed, name) in index.series.iter_mut().zip(named) {
        listed.info.description = name.to_owned();
    }
    let second = index.series.get(1).map(|listed| listed.info.id.clone());
    let series = second.expect("listed");
    let state = AppState {
        index: Some(index),
        ..AppState::default()
    };
    let shown = listed_as(&state, "two echoes", |_| true);
    assert_eq!(
        shown,
        Some(Action::SelectSeries {
            viewport: 0,
            series
        })
    );
    let early = listed_as(&state, "two echoes", |_| false);
    assert_eq!(early, None, "not before its thumbnail is there");
    let missing = listed_as(&state, "sideways", |_| true);
    assert_eq!(missing, None, "nothing is called that");
}

#[test]
fn no_words_for_a_series_is_not_asking_for_one() {
    assert_eq!(asked_for(None), None);
    assert_eq!(asked_for(Some(" ".to_owned())), None);
    let words = Some("two echoes".to_owned());
    assert_eq!(asked_for(words.clone()), words);
}

#[test]
fn a_picture_can_ask_for_the_first_series_listed_once_the_index_is_finished() {
    let mut state = AppState::default();
    assert_eq!(
        first_listed(&state, |_| true),
        None,
        "nothing is listed yet"
    );
    state.index = Some(crate::testing::with_odd_series());
    let listed = state.index.as_ref().and_then(|index| index.series.first());
    let series = listed.map(|listed| listed.info.id.clone()).expect("listed");
    let unfinished = first_listed(&state, |_| false);
    assert_eq!(
        unfinished, None,
        "its thumbnail comes once the index is finished"
    );
    let (viewport, shown) = (0, first_listed(&state, |_| true));
    assert_eq!(shown, Some(Action::SelectSeries { viewport, series }));
}

#[test]
fn a_picture_can_ask_for_the_labelled_parts_to_be_posed_once_they_are_read() {
    use volsa_render::Look;
    assert_eq!(
        label_pose("parts, filled, mip"),
        [],
        "those wait for no labels"
    );
    assert_eq!(label_pose("solids"), [Action::SceneLook(Look::Parts)]);
    let faded = Action::PartOpacity {
        part: 0,
        opacity: 0.3,
    };
    let chosen = Action::PartSelect(Some(0));
    let all = label_pose("chosen, faded, solids");
    assert_eq!(all, [Action::SceneLook(Look::Parts), faded, chosen]);
    assert_eq!(pose("solids, faded, chosen"), [], "not before the labels");
}

#[test]
fn a_window_size_is_read_as_width_by_height() {
    assert_eq!(size_from("900x600"), Some([900.0, 600.0]));
    assert_eq!(size_from(" 1440 x 900 "), Some([1440.0, 900.0]));
    assert_eq!(size_from("900"), None);
    assert_eq!(size_from("0x600"), None, "a window has to have a size");
    assert_eq!(size_from("wide x tall"), None);
}

#[test]
fn a_picture_can_draw_the_interface_smaller_to_show_a_larger_screen() {
    assert_eq!(zoom_from("0.75"), Some(0.75));
    assert_eq!(zoom_from(" 0.375 "), Some(0.375));
    for nonsense in ["0", "-1", "large", "", "NaN", "inf"] {
        assert_eq!(zoom_from(nonsense), None, "{nonsense}");
    }
    // The size asked for is in points of the interface as it is drawn: at 0.75, 1920 points
    // across are a window 1440 points across at the usual size.
    assert_eq!(window_for([1920.0, 1010.0], 0.75), [1440.0, 757.5]);
    assert_eq!(window_for([3840.0, 2090.0], 0.375), [1440.0, 783.75]);
    assert_eq!(window_for([900.0, 600.0], 1.0), [900.0, 600.0]);
}

#[test]
fn what_a_picture_asks_for_is_read_word_by_word() {
    assert!(asks_for("keys, nolist", "nolist"));
    assert!(asks_for("keys, nolist", "keys"));
    assert!(
        !asks_for("nolist", "list"),
        "a word is read whole: `nolist`, which folds the rail of scans, is not `list`"
    );
    assert!(!asks_for("", "keys"));
}

#[test]
fn a_picture_can_ask_for_one_of_each_kind_of_annotation_in_the_axial_plane() {
    use volsa_core::Volume;
    use volsa_volume::PlaneKind;

    use crate::annotations::Shape;
    use crate::planes::plane;
    use crate::testing::{THICK_SCAN, id, thick_scan_ready};
    let state = thick_scan_ready();
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let scan = Volume::new([4, 4, 30], vec![5.0; 480], placement).expect("adds up");
    let made = sample_annotations(&state, &held_of(&scan));
    let shapes: Vec<&Shape> = made
        .iter()
        .filter_map(|action| match action {
            Action::AnnotationAdd(shape) => Some(shape),
            _ => None,
        })
        .collect();
    let kinds: Vec<&str> = shapes.iter().map(|shape| shape.kind()).collect();
    assert_eq!(kinds, ["Ruler", "Angle", "Region", "Marker"]);
    assert_eq!(made.len(), 4, "and nothing else");
    let axial = plane(&state, PlaneKind::Axial).expect("a plane");
    let on_the_plane = |at: &[f64; 3]| {
        let [x, y, z] = *at;
        axial.of(volsa_core::Point::new(x, y, z)).1.abs() < 1.0e-9
    };
    for shape in &shapes {
        assert!(shape.points().iter().all(on_the_plane), "{shape:?}");
    }
    assert_eq!(
        sample_annotations(&AppState::default(), &held_of(&scan)),
        [],
        "nothing with no volume"
    );
}

#[test]
fn preferences_are_kept_in_the_settings_folder_and_what_comes_of_scans_beside_the_log() {
    assert_eq!(folders(true), (None, None), "a picture keeps nothing");
    let (settings, local) = folders(false);
    assert_eq!(settings, crate::settings::folder());
    assert_eq!(local, crate::log::folder());
    if cfg!(windows) {
        assert_ne!(
            settings, local,
            "the roaming profile is not this computer's own"
        );
    }
}

#[test]
fn a_picture_of_the_window_neither_shows_nor_changes_what_is_kept_between_runs() {
    let folder = || Some(PathBuf::from("where settings are kept"));
    assert_eq!(kept_between_runs(false, folder()), folder());
    assert_eq!(kept_between_runs(true, folder()), None);
    assert_eq!(kept_between_runs(false, None), None);
}

#[test]
fn a_picture_of_the_scene_moving_turns_it_to_and_fro_once_it_has_come_to_rest() {
    assert_eq!(turn_at(Motion::Still, 100), None, "not asked for");
    assert_eq!(
        turn_at(Motion::Moving, 0),
        None,
        "left to come to rest first"
    );
    assert_eq!(turn_at(Motion::Moving, 44), None);
    let turned = |settled| match turn_at(Motion::Moving, settled) {
        Some(Action::SceneOrbit { by: [across, down] }) => (across, down),
        other => panic!("{other:?}"),
    };
    assert_eq!((turned(45), turned(46)), ((-0.5, 0.0), (0.5, 0.0)));
    assert_eq!(turned(45).0 + turned(46).0, 0.0, "and back: the same view");
    let frames = (frames_before(Motion::Still), frames_before(Motion::Moving));
    assert_eq!(frames, (30, 60));
}

#[test]
fn a_picture_can_ask_for_the_strip_as_it_is_on_a_computer_that_can_label() {
    use super::labelling::looked;
    use crate::labelling::{LabellingAction, Ready};
    use crate::reducer::reduce;
    use volsa_models::CardKind;
    assert_eq!(looked("tags,keys"), None);
    assert_eq!(looked("parts, solids"), None);
    let asked = looked("parts, labelling").expect("a look is made up");
    let LabellingAction::LabellingLooked { look, ready, cards } = asked.clone() else {
        panic!("not a look: {asked:?}");
    };
    assert_eq!(
        (look, ready),
        (0, Ready::Yes),
        "the look nobody has asked for yet"
    );
    let kinds: Vec<CardKind> = cards.iter().map(|card| card.kind).collect();
    assert!(kinds.contains(&CardKind::Discrete) && kinds.contains(&CardKind::Integrated));
    assert_eq!(
        looked("labelled"),
        None,
        "that asks for a run, not for a made-up look"
    );
    // The state takes it in as the answer to the first look.
    let mut state = AppState::default();
    reduce(&mut state, Action::Labelling(asked));
    assert_eq!(state.labelling.ready, Ready::Yes);
    assert_eq!(state.labelling.cards, cards);
}

#[test]
fn a_picture_of_a_result_made_by_volsa_starts_a_run_once_it_can_and_waits_for_it() {
    use super::labelling::Run;
    use crate::labelling::{LabellingAction, Ready};
    use crate::reducer::reduce;
    assert_eq!(Run::asked("parts, solids"), Run::Not);
    let mut run = Run::asked("solids, labelled");
    assert_eq!(run, Run::ToStart);
    let mut state = crate::testing::thick_scan_ready();
    assert_eq!(
        run.start(&state),
        None,
        "it is not known yet that labelling can run"
    );
    assert!(!run.is_over(&state));
    state.labelling.ready = Ready::Yes;
    let start = Action::Labelling(LabellingAction::LabelScan { held: 0 });
    assert_eq!(run.start(&state), Some(start.clone()));
    reduce(&mut state, start);
    assert_eq!(run, Run::On);
    assert!(!run.is_over(&state), "the run is on");
    assert_eq!(run.start(&state), None, "a run is started once");
    reduce(
        &mut state,
        Action::Labelling(LabellingAction::StopLabelling),
    );
    assert!(run.is_over(&state), "the run has ended");
}

#[test]
fn a_picture_does_not_wait_for_a_run_that_this_computer_cannot_make() {
    use super::labelling::Run;
    use crate::labelling::Ready;
    let mut run = Run::asked("labelled");
    let mut state = crate::testing::thick_scan_ready();
    state.labelling.ready = Ready::No("Made-up: the runtime files are not there.".to_owned());
    assert_eq!(run.start(&state), None);
    assert!(run.is_over(&state), "the strip is pictured as it is");
    let usual = Duration::from_secs(20);
    assert!(
        Run::asked("labelled").patience(usual) > usual,
        "a run takes longer"
    );
    assert_eq!(Run::Not.patience(usual), usual);
}

#[test]
fn a_picture_can_ask_for_a_tab_of_the_panel_to_be_open() {
    use crate::ui::panel::Tab;
    assert_eq!(tab_asked("tags, vr"), None);
    assert_eq!(tab_asked("measure"), Some(Tab::Measure));
    assert_eq!(
        tab_asked("labelling"),
        Some(Tab::Tools),
        "the strip that labels a scan is in the Tools tab"
    );
    assert_eq!(
        tab_asked("labelled, parts"),
        Some(Tab::Labels),
        "the parts are in the Labels tab"
    );
    assert_eq!(
        tab_asked("labelling, parts"),
        Some(Tab::Labels),
        "the labels, when both are asked for"
    );
}

#[test]
fn the_parts_are_no_longer_a_pose_of_the_column() {
    assert_eq!(pose("parts"), [], "they are a tab of the panel now");
}

#[test]
fn a_picture_can_ask_for_an_arrangement_of_the_pictures() {
    use crate::pictures::Pictures;
    assert_eq!(arrangement_asked(""), None, "the slices, as Volsa opens");
    assert_eq!(arrangement_asked("tags, vr"), None);
    assert_eq!(arrangement_asked("planes"), Some(Pictures::Planes));
    assert_eq!(arrangement_asked("quad"), Some(Pictures::Planes));
    assert_eq!(arrangement_asked("3d"), Some(Pictures::Scene));
    assert_eq!(
        arrangement_asked("vr, slices3d"),
        Some(Pictures::SlicesAndScene)
    );
    assert_eq!(
        arrangement_asked("3d, planes"),
        Some(Pictures::Planes),
        "the last of those asked for"
    );
    for word in ["planes", "quad", "3d", "slices3d"] {
        assert_eq!(pose(word), [], "{word} is set at once, not as a pose");
    }
}

#[test]
fn a_picture_is_dark_unless_it_asks_for_the_light_theme() {
    use crate::ui::theme::ThemeChoice::{Dark, Light, System};
    // A picture is the same wherever it is made, so the system's choice is not followed.
    assert_eq!(theme_for("", true, System), Dark);
    assert_eq!(theme_for("keys,about", true, System), Dark);
    assert_eq!(theme_for("light", true, System), Light);
    assert_eq!(theme_for("about, light", true, Dark), Light);
    assert_eq!(theme_for("lighter", true, System), Dark, "whole words");
    // A window that is not a picture is drawn as the person left it, whatever the words say.
    assert_eq!(theme_for("light", false, Dark), Dark);
    assert_eq!(theme_for("", false, System), System);
    assert_eq!(theme_for("", false, Light), Light);
    assert_eq!(theme_for("", false, Dark), Dark);
}
