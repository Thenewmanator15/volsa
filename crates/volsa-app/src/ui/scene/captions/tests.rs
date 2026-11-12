//! What the 3D scene says of itself: its captions, its letters and how it settles.

use volsa_core::{Point, WindowLevel};
use volsa_render::{CutPlane, Look, NamedView};

use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::testing::{THICK_SCAN, id, three_series};

fn letters(view: NamedView) -> [String; 4] {
    let camera = Camera::looking(view, Point::new(0.0, 0.0, 0.0), 1.0);
    let EdgeLabels {
        left,
        right,
        top,
        bottom,
    } = edge_letters(&camera);
    [left, right, top, bottom]
}

/// A state showing the 30-slice series, whose slices are 5 mm apart going up the
/// patient, with its volume built.
fn ready() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(
        &mut state,
        Action::SelectSeries {
            viewport,
            series: series.clone(),
        },
    );
    let corners = [Point::new(0.0, 0.0, 0.0); 8];
    reduce(&mut state, Action::VolumeReady { series, corners });
    state
}

#[test]
fn the_solids_are_said_to_be_made_elsewhere_or_by_volsa_as_their_labels_say() {
    let (mut state, _) = crate::testing::labelled_head();
    assert_eq!(solids(&state), CAPTION);
    state.labels.made = Some(volsa_labels::Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "Made-up model".to_owned(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        ..volsa_labels::Made::default()
    });
    let made = "Solids are labels made by Volsa with Made-up model, on Made-up card A, \
                2099-01-02. Not for diagnostic use.";
    assert_eq!(solids(&state), made);
    // A check that failed is said first, as it is in the list; one that passed is not.
    let verdict = |passed| volsa_labels::Verdict {
        passed,
        fail_below: 0.65,
        low: Vec::new(),
    };
    state.labels.verdict = Some(verdict(true));
    assert_eq!(solids(&state), made);
    state.labels.verdict = Some(verdict(false));
    assert_eq!(
        solids(&state),
        format!("The model's own check of these labels failed. {made}")
    );
}

#[test]
fn the_scene_says_solids_a_plugin_made_were_added_by_you() {
    let (mut state, _) = crate::testing::labelled_head();
    let added = volsa_labels::Added {
        name: "Made-up strip".to_owned(),
        version: "1.2".to_owned(),
        ..volsa_labels::Added::default()
    };
    state.labels.made = Some(volsa_labels::Made {
        device: "Made-up card A".to_owned(),
        added: Some(added),
        ..volsa_labels::Made::default()
    });
    let said = "Solids are labels added by you, not part of Volsa: Made-up strip 1.2. Not \
                for diagnostic use.";
    assert_eq!(solids(&state), said);
}

#[test]
fn thick_slices_are_said_to_be_interpolated_between() {
    let said = "Slices are 7.5 mm apart, so detail between them is interpolated, not measured.";
    assert_eq!(captions([0.7, 0.7, 7.5], 0), [said]);
    assert_eq!(captions([1.0, 1.0, 1.0], 0), Vec::<String>::new());
    assert_eq!(
        captions([0.5, 0.5, 1.0], 0),
        Vec::<String>::new(),
        "twice is not thick"
    );
    let whole = "Slices are 3 mm apart, so detail between them is interpolated, not measured.";
    assert_eq!(captions([1.0, 0.9, 3.0], 0), [whole]);
}

#[test]
fn a_reduced_volume_says_by_how_much() {
    let said = |share: &str| format!("Shown at {share} resolution to fit this graphics device.");
    assert_eq!(captions([1.0, 1.0, 1.0], 1), [said("half")]);
    assert_eq!(captions([1.0, 1.0, 1.0], 2), [said("a quarter")]);
    assert_eq!(captions([1.0, 1.0, 1.0], 3), [said("an eighth")]);
    assert_eq!(captions([1.0, 1.0, 1.0], 5), [said("1/32")]);
    assert_eq!(
        captions([0.7, 0.7, 7.5], 1).len(),
        2,
        "both, when both are so"
    );
}

#[test]
fn the_letters_at_the_edges_follow_the_camera() {
    assert_eq!(letters(NamedView::Anterior), ["R", "L", "S", "I"]);
    assert_eq!(letters(NamedView::Left)[..2], ["A", "P"]);
    assert_eq!(letters(NamedView::Posterior)[..2], ["L", "R"]);
    assert_eq!(letters(NamedView::Inferior), ["R", "L", "A", "P"]);
    assert_eq!(letters(NamedView::Superior), ["L", "R", "A", "P"]);
}

#[test]
fn the_picture_is_coarse_until_the_camera_has_been_still_a_moment() {
    assert_eq!(quality(Duration::ZERO, None), 0.5);
    assert_eq!(quality(Duration::from_millis(149), None), 0.5);
    assert_eq!(quality(Duration::from_millis(150), None), 1.0);
    // A device timed at 3 ms for a full picture draws the moving one in full too, and
    // one timed at 12 ms at three quarters; once still, each draws in full.
    let (fast, slower) = (
        Some(Duration::from_millis(3)),
        Some(Duration::from_millis(12)),
    );
    assert_eq!(quality(Duration::ZERO, fast), 1.0);
    assert_eq!(quality(Duration::ZERO, slower), 0.75);
    assert_eq!(quality(Duration::from_millis(150), slower), 1.0);
    assert_eq!(
        until_full_quality(Duration::from_millis(100)),
        Some(Duration::from_millis(50))
    );
    assert_eq!(until_full_quality(Duration::from_millis(150)), None);
    assert_eq!(until_full_quality(Duration::from_secs(9)), None);
}

#[test]
fn a_scene_with_no_picture_says_where_it_has_got_to() {
    let mut state = AppState::default();
    assert_eq!(
        status_text(&state.scene).as_deref(),
        Some("No series is shown here")
    );
    state.scene.status = SceneStatus::Building { done: 16, of: 30 };
    let building = "Building the 3D view: 16 of 30 slices";
    assert_eq!(status_text(&state.scene).as_deref(), Some(building));
    let why = vec![
        "This series cannot be shown in 3D.".to_owned(),
        "Because.".to_owned(),
    ];
    state.scene.status = SceneStatus::Unavailable(why);
    let both = "This series cannot be shown in 3D.\nBecause.";
    assert_eq!(status_text(&state.scene).as_deref(), Some(both));
    assert_eq!(status_text(&ready().scene), None);
}

#[test]
fn the_picture_is_drawn_from_the_scene_and_the_active_viewports_window() {
    let mut state = ready();
    let wl = WindowLevel::new(321.0, 123.0);
    reduce(&mut state, Action::SetWindow { viewport: 0, wl });
    reduce(&mut state, Action::SceneLook(Look::Mip));
    reduce(&mut state, Action::CutAdd);
    let params = scene_params(&state, 0.5).expect("the volume is ready");
    assert_eq!(
        (params.window, params.look, params.quality),
        (wl, Look::Mip, 0.5)
    );
    assert_eq!(params.camera, state.scene.camera);
    assert_eq!(params.cuts, [CutPlane::facing(&state.scene.camera)]);
    assert!(params.skip_empty && params.slice.is_none());
}

#[test]
fn the_slice_the_viewport_shows_is_drawn_in_the_scene_when_asked() {
    let mut state = ready();
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 3,
        },
    );
    reduce(&mut state, Action::ShowSliceInScene(true));
    let plane = scene_params(&state, 1.0)
        .and_then(|params| params.slice)
        .expect("a plane");
    assert_eq!(plane.point.z, 15.0, "the fourth slice, 5 mm apart");
    assert_eq!(plane.normal.map(f64::abs), [0.0, 0.0, 1.0]);
    reduce(&mut state, Action::ShowSliceInScene(false));
    assert_eq!(
        scene_params(&state, 1.0).and_then(|params| params.slice),
        None
    );
}

#[test]
fn nothing_is_drawn_until_the_volume_is_ready() {
    let mut state = ready();
    state.scene.status = SceneStatus::Building { done: 3, of: 30 };
    assert_eq!(scene_params(&state, 1.0), None);
    assert_eq!(scene_params(&AppState::default(), 1.0), None);
}

#[test]
fn a_preview_is_drawn_while_the_volume_builds_and_says_how_far_it_has_got() {
    let mut state = ready();
    state.scene.status = SceneStatus::Building { done: 40, of: 300 };
    state.scene.preview = true;
    let said = "Preview: loading full detail (40 of 300 slices)";
    assert_eq!(preview_text(&state.scene).as_deref(), Some(said));
    assert!(scene_params(&state, 1.0).is_some());
    state.scene.status = SceneStatus::Ready;
    assert_eq!(preview_text(&state.scene), None, "the volume itself now");
}
