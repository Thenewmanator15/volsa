//! Pictures of a plane through a volume, drawn on a graphics device with no window.

use voxa_core::{Patient, Point, Volume, WindowLevel};
use voxa_volume::{PlaneKind, plane_of, reslice};

use super::tests::{CORE, MIDDLE, SHELL, at};
use super::*;
use crate::plane_view::PlaneView;
use crate::testing::{
    HALF_SHELL, MM, SIDE, ball, ball_on, ball_window, bead_middle, brightest, centred, grey,
    headless_scene, silhouette,
};

/// Where the bead of the test ball is.
const BEAD: [f64; 3] = [12.0, -14.0, 0.0];

/// What a square test panel shows of a volume: one of its planes through a point.
fn view(
    volume: &Volume,
    kind: PlaneKind,
    through: [f64; 3],
    radiological: bool,
    zoom: f64,
) -> PlaneView {
    let [x, y, z] = through;
    let placement = volume.voxel_to_patient();
    let point: Point<Patient> = Point::new(x, y, z);
    let plane = plane_of(&placement, volume.dims(), kind, point, radiological).expect("a plane");
    PlaneView::showing(plane, &volume.corners(), [SIDE as f32; 2], zoom, [0.0; 2])
}

fn drawn(gpu: &mut HeadlessScene, view: &PlaneView) -> Vec<u8> {
    gpu.render_plane(view, ball_window(), [SIDE, SIDE])
        .expect("renders")
}

/// The grey a scan value is shown as through a window, as the shader works it out.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a share of 255, held between 0 and 255"
)]
fn windowed(value: f32, window: WindowLevel) -> u8 {
    let share = ((value - window.low()) / window.width).clamp(0.0, 1.0);
    if value.is_nan() {
        0
    } else {
        (share * 255.0).round() as u8
    }
}

#[test]
fn the_axial_plane_through_the_ball_shows_its_core_and_shell_as_the_processor_reads_them() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let view = view(&volume, PlaneKind::Axial, [0.0; 3], true, 1.0);
    let rgba = drawn(&mut gpu, &view);
    // A pixel is 0.52 mm: 10 mm from the middle is 19 pixels, 20 mm is 38.
    assert_eq!(at(&rgba, MIDDLE), CORE);
    assert_eq!(at(&rgba, (67, 48)), SHELL);
    assert_eq!(at(&rgba, (86, 48)), 0);
    assert_eq!(rgba[3], 255, "every pixel is opaque");
    let read = reslice(&volume, &view.plane, [SIDE, SIDE], view.mm_per_px);
    let apart: u64 = (0..SIDE * SIDE)
        .map(|pixel| {
            let found = grey(&rgba, pixel % SIDE, pixel / SIDE);
            u64::from(found.abs_diff(windowed(read[pixel as usize], ball_window())))
        })
        .sum();
    let mean = apart as f64 / f64::from(SIDE * SIDE);
    assert!(
        mean < 1.0,
        "{mean} grey levels a pixel from the processor's own reading"
    );
}

#[test]
fn each_plane_has_the_patient_the_way_round_radiology_shows() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let middle = f64::from(SIDE) / 2.0;
    let mut bead = |kind, radiological| {
        bead_middle(&drawn(
            &mut gpu,
            &view(&volume, kind, BEAD, radiological, 1.0),
        ))
    };
    // The bead is on the patient's left, towards the front, level with the middle.
    let [across, down] = bead(PlaneKind::Axial, true);
    assert!(
        across > middle + 10.0 && down < middle - 10.0,
        "axial: {across}, {down}"
    );
    let [across, down] = bead(PlaneKind::Axial, false);
    assert!(
        across < middle - 10.0 && down < middle - 10.0,
        "not radiological: {across}, {down}"
    );
    let [across, down] = bead(PlaneKind::Coronal, true);
    assert!(
        across > middle + 10.0 && (down - middle).abs() < 3.0,
        "coronal: {across}, {down}"
    );
    let [across, _] = bead(PlaneKind::Coronal, false);
    assert!(
        across < middle - 10.0,
        "coronal, not radiological: {across}"
    );
    let [across, down] = bead(PlaneKind::Sagittal, true);
    assert!(
        across < middle - 10.0 && (down - middle).abs() < 3.0,
        "sagittal: {across}, {down}"
    );
    assert_eq!(
        bead(PlaneKind::Sagittal, false),
        [across, down],
        "the same either way"
    );
}

#[test]
fn thick_slices_are_drawn_in_their_true_proportions() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let dims = [48, 48, 12];
    let thick = ball_on(
        dims,
        centred(dims, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 4.0]]),
    );
    gpu.upload(&thick).expect("uploads");
    let rgba = drawn(
        &mut gpu,
        &view(&thick, PlaneKind::Coronal, [0.0; 3], true, 1.0),
    );
    let [wide, high] = silhouette(&rgba, HALF_SHELL);
    assert!(wide.abs_diff(high) <= 2, "{wide} wide, {high} high");
    assert!(
        (58..=66).contains(&wide),
        "32 mm at 0.52 mm a pixel: {wide}"
    );
}

#[test]
fn what_lies_outside_the_volume_is_black_not_a_smear_of_its_edge() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let even = Volume::new([8; 3], vec![300.0; 512], centred([8; 3], MM)).expect("adds up");
    gpu.upload(&even).expect("uploads");
    for kind in PlaneKind::ALL {
        let rgba = drawn(&mut gpu, &view(&even, kind, [0.0; 3], true, 0.5));
        assert_eq!(
            at(&rgba, MIDDLE),
            CORE,
            "{kind:?}: the volume is in the middle"
        );
        let corners = [(0, 0), (SIDE - 1, 0), (0, SIDE - 1), (SIDE - 1, SIDE - 1)];
        assert_eq!(corners.map(|corner| at(&rgba, corner)), [0; 4], "{kind:?}");
        // At half size the volume fills about half the panel each way.
        assert_eq!(
            (at(&rgba, (30, 48)), at(&rgba, (18, 48))),
            (CORE, 0),
            "{kind:?}"
        );
    }
}

#[test]
fn with_no_volume_a_plane_is_black_and_a_new_volume_takes_the_place_of_the_old() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    let through_the_middle = view(&volume, PlaneKind::Axial, [0.0; 3], true, 1.0);
    assert_eq!(brightest(&drawn(&mut gpu, &through_the_middle)), 0);
    gpu.upload(&volume).expect("uploads");
    assert_eq!(at(&drawn(&mut gpu, &through_the_middle), MIDDLE), CORE);
    let even = Volume::new([48; 3], vec![100.0; 48 * 48 * 48], centred([48; 3], MM));
    gpu.upload(&even.expect("adds up")).expect("uploads");
    assert_eq!(
        at(&drawn(&mut gpu, &through_the_middle), MIDDLE),
        SHELL,
        "the newer volume"
    );
    let no_size = gpu.render_plane(&through_the_middle, ball_window(), [0, 96]);
    assert!(
        matches!(no_size, Err(RenderError::EmptyTarget)),
        "{no_size:?}"
    );
}
