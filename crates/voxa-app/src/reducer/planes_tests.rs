//! The position the three planes share, and their panels, in the state.

use std::path::PathBuf;

use voxa_core::{Affine, Patient, Point, Voxel};
use voxa_render::{MAX_PLANE_ZOOM, MIN_PLANE_ZOOM};
use voxa_volume::{PlaneKind, central_voxel, clamped_into, nearest_slice};

use super::scene_tests::{ready, select};
use super::tests::showing;
use super::*;
use crate::planes::{ColumnShows, PlanePanel, Planes, plane};
use crate::scene::{MainPanel, SceneStatus};
use crate::testing::{FINE_SCAN, THICK_SCAN, id};

/// Where the 30-slice series lies, and its columns, rows and slices.
fn thick_scan(state: &AppState) -> (Affine<Voxel, Patient>, [u32; 3]) {
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("it has geometry");
    (placement, [info.cols, info.rows, 30])
}

fn position(state: &AppState) -> Point<Patient> {
    state.planes.position.expect("there is a position")
}

fn apart(a: Point<Patient>, b: Point<Patient>) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

#[test]
fn the_position_starts_on_the_voxel_in_the_middle_once_the_volume_is_ready() {
    let building = showing(THICK_SCAN);
    assert_eq!(
        building.planes,
        Planes::default(),
        "nowhere while the volume is built"
    );
    assert_eq!(building.planes.position, None);
    assert!(building.planes.linked && !building.planes.smooth);
    let state = ready();
    let (placement, dims) = thick_scan(&state);
    assert_eq!(position(&state), central_voxel(&placement, dims));
    assert_eq!(state.planes.panels, [PlanePanel::default(); 3]);
    assert_eq!(
        PlanePanel::default(),
        PlanePanel {
            zoom: 1.0,
            offset_mm: [0.0; 2]
        }
    );
}

#[test]
fn a_position_asked_for_outside_the_volume_is_held_on_its_face() {
    let mut state = ready();
    let (placement, dims) = thick_scan(&state);
    let far = Point::new(500.0, 500.0, 500.0);
    reduce(&mut state, Action::SetPosition(far));
    assert_eq!(position(&state), clamped_into(&placement, dims, far));
    assert!(
        apart(position(&state), far) > 100.0,
        "it did not go where it was sent"
    );
    let inside = central_voxel(&placement, dims);
    reduce(&mut state, Action::SetPosition(inside));
    assert_eq!(position(&state), inside);
    let mut building = showing(THICK_SCAN);
    reduce(&mut building, Action::SetPosition(inside));
    assert_eq!(
        building.planes.position, None,
        "there is no volume to be in yet"
    );
}

#[test]
fn scrolling_a_plane_moves_the_position_one_layer_and_stops_at_the_last() {
    let mut state = ready();
    let (placement, _) = thick_scan(&state);
    let (kind, before) = (PlaneKind::Axial, position(&state));
    let axial = plane(&state, kind).expect("a plane");
    reduce(&mut state, Action::PlaneScroll { kind, steps: 1 });
    let after = position(&state);
    assert!(
        (apart(before, after) - 5.0).abs() < 1.0e-9,
        "slices are 5 mm apart"
    );
    let ([across, down], _) = axial.of(after);
    let ([was_across, was_down], _) = axial.of(before);
    assert!((across - was_across).abs() < 1.0e-9 && (down - was_down).abs() < 1.0e-9);
    let slice = |state: &AppState| nearest_slice(&placement, 30, position(state));
    assert_eq!(
        slice(&state),
        nearest_slice(&placement, 30, before) + 1,
        "towards the next slice"
    );
    reduce(&mut state, Action::PlaneScroll { kind, steps: 1000 });
    assert_eq!(slice(&state), 29);
    let at_the_end = position(&state);
    reduce(&mut state, Action::PlaneScroll { kind, steps: 1 });
    assert_eq!(position(&state), at_the_end);
    let mut building = showing(THICK_SCAN);
    reduce(&mut building, Action::PlaneScroll { kind, steps: 1 });
    assert_eq!(building.planes.position, None);
}

#[test]
fn zooming_a_panel_keeps_still_the_point_it_is_zoomed_about() {
    let mut state = ready();
    let kind = PlaneKind::Coronal;
    let zoom = |state: &mut AppState, factor: f64| {
        let about_mm = [10.0, 0.0];
        reduce(
            state,
            Action::PlaneZoom {
                kind,
                factor,
                about_mm,
            },
        );
        state.planes.panel(kind)
    };
    // Twice as large about a point 10 mm to the side: the middle is then half way to it.
    assert_eq!(
        zoom(&mut state, 2.0),
        PlanePanel {
            zoom: 2.0,
            offset_mm: [5.0, 0.0]
        }
    );
    assert_eq!(
        state.planes.panel(PlaneKind::Axial),
        PlanePanel::default(),
        "one panel only"
    );
    assert_eq!(zoom(&mut state, 1.0e9).zoom, MAX_PLANE_ZOOM);
    assert_eq!(zoom(&mut state, 1.0e-12).zoom, MIN_PLANE_ZOOM);
    let before = state.planes.panel(kind);
    for nothing in [f64::NAN, 0.0, -2.0, f64::INFINITY] {
        assert_eq!(zoom(&mut state, nothing), before, "{nothing}");
    }
}

#[test]
fn a_panel_is_moved_and_then_shown_whole_again() {
    let mut state = ready();
    let kind = PlaneKind::Sagittal;
    reduce(
        &mut state,
        Action::PlanePan {
            kind,
            by_mm: [3.0, -4.0],
        },
    );
    reduce(
        &mut state,
        Action::PlanePan {
            kind,
            by_mm: [1.0, 1.0],
        },
    );
    assert_eq!(state.planes.panel(kind).offset_mm, [4.0, -3.0]);
    reduce(
        &mut state,
        Action::PlanePan {
            kind,
            by_mm: [f64::NAN, 1.0],
        },
    );
    assert_eq!(state.planes.panel(kind).offset_mm, [4.0, -3.0]);
    reduce(&mut state, Action::PlaneFit(kind));
    assert_eq!(state.planes.panel(kind), PlanePanel::default());
}

#[test]
fn another_series_or_another_scan_starts_the_planes_afresh_but_keeps_what_was_chosen() {
    let mut state = ready();
    reduce(&mut state, Action::SetColumn(ColumnShows::Planes));
    reduce(
        &mut state,
        Action::PlanePan {
            kind: PlaneKind::Axial,
            by_mm: [3.0, 0.0],
        },
    );
    select(&mut state, 0, FINE_SCAN);
    assert_eq!(state.planes.position, None);
    assert_eq!(state.planes.panels, [PlanePanel::default(); 3]);
    assert_eq!(state.planes.column, ColumnShows::Planes);
    let mut state = ready();
    reduce(&mut state, Action::SetColumn(ColumnShows::Planes));
    reduce(&mut state, Action::OpenPath(PathBuf::from("another.zip")));
    assert_eq!(state.planes.position, None);
    assert_eq!(state.planes.column, ColumnShows::Planes);
}

#[test]
fn the_planes_can_have_the_large_part_unless_there_is_no_3d_view() {
    let mut state = ready();
    reduce(&mut state, Action::SetMain(MainPanel::Planes));
    assert_eq!(state.main.given(&state.scene.status), MainPanel::Planes);
    let no_view = SceneStatus::Unavailable(vec!["why".to_owned()]);
    assert_eq!(MainPanel::Planes.given(&no_view), MainPanel::Stack);
}

#[test]
fn a_plane_is_had_only_of_a_volume_that_is_ready_and_faces_as_radiological_says() {
    assert_eq!(plane(&showing(THICK_SCAN), PlaneKind::Axial), None);
    assert_eq!(plane(&AppState::default(), PlaneKind::Axial), None);
    let mut state = ready();
    let facing = |state: &AppState| plane(state, PlaneKind::Axial).expect("a plane").across;
    let on = facing(&state);
    reduce(&mut state, Action::ToggleRadiological);
    assert_eq!(facing(&state), on.map(|part| -part));
    let ([across, down], off) = plane(&state, PlaneKind::Coronal)
        .expect("one")
        .of(position(&state));
    assert!(
        off.abs() < 1.0e-9,
        "the plane holds the position: {across}, {down}, {off}"
    );
}

#[test]
fn the_planes_are_in_sight_in_the_column_or_with_the_large_part() {
    use crate::planes::in_sight;
    let mut state = ready();
    assert!(!in_sight(&state), "the column shows the slices");
    reduce(&mut state, Action::SetColumn(ColumnShows::Planes));
    assert!(in_sight(&state));
    reduce(&mut state, Action::SetMain(MainPanel::Stack));
    assert!(
        !in_sight(&state),
        "the slices are large and the column holds the 3D scene"
    );
    reduce(&mut state, Action::SetMain(MainPanel::Planes));
    reduce(&mut state, Action::SetColumn(ColumnShows::Slices));
    assert!(
        in_sight(&state),
        "they have the large part whatever the column held"
    );
    state.scene.status = SceneStatus::Unavailable(vec!["why".to_owned()]);
    assert!(
        !in_sight(&state),
        "with no 3D view the slices have the large part"
    );
}
