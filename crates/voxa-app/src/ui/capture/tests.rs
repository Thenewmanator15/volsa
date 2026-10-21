//! What a picture of the window can ask for.

use super::*;

#[test]
fn a_picture_can_ask_for_the_scene_to_be_posed() {
    use voxa_render::Look;
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
fn a_picture_can_ask_for_one_of_each_kind_of_annotation_in_the_axial_plane() {
    use voxa_core::Volume;
    use voxa_volume::PlaneKind;

    use crate::annotations::Shape;
    use crate::planes::plane;
    use crate::testing::{THICK_SCAN, id, thick_scan_ready};
    let state = thick_scan_ready();
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let scan = Volume::new([4, 4, 30], vec![5.0; 480], placement).expect("adds up");
    let made = sample_annotations(&state, &scan);
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
        axial.of(voxa_core::Point::new(x, y, z)).1.abs() < 1.0e-9
    };
    for shape in &shapes {
        assert!(shape.points().iter().all(on_the_plane), "{shape:?}");
    }
    assert_eq!(
        sample_annotations(&AppState::default(), &scan),
        [],
        "nothing with no volume"
    );
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
