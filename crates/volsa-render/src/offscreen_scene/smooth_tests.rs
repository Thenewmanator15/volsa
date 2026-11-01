//! Pictures of a plane drawn from the curve through a volume's values, beside the plain ones.

use volsa_core::{Affine, Point, Volume, WindowLevel};
use volsa_volume::{PlaneKind, coefficients, plane_of, sample_cubic};

use crate::plane_view::PlaneView;
use crate::prepared::{MAX_VOLUME_BYTES, Prepared};
use crate::smoothed::Smoothed;
use crate::testing::{ball, ball_window, headless_scene};

/// The picture's width and height. Both are odd, so that the middle pixel shows the middle
/// of the plane, and at half a millimetre a pixel every other row shows a slice itself and
/// the rows between show the places half way between two slices.
const SIZE: [u32; 2] = [13, 33];

/// A volume of 6 by 6 voxels in 16 slices, 1 mm apart from the origin, whose every value is
/// the square of its slice's number: it bends between slices, which a blend cuts across.
fn squares() -> Volume {
    let values = (0..16u16).flat_map(|slice| vec![f32::from(slice * slice); 36]);
    Volume::new([6, 6, 16], values.collect(), Affine::identity()).expect("adds up")
}

/// The sagittal plane through the middle of [`squares`], half a millimetre to the pixel. Row
/// `row` of the picture shows the place 15.5 - row / 2 slices up: row 22 is half way between
/// slices 4 and 5, row 21 is slice 5, row 23 is slice 4.
fn view(volume: &Volume) -> PlaneView {
    let placement = volume.voxel_to_patient();
    let middle = Point::new(2.5, 2.5, 7.5);
    let plane = plane_of(&placement, volume.dims(), PlaneKind::Sagittal, middle, true);
    PlaneView {
        plane: plane.expect("a plane"),
        mm_per_px: 0.5,
        offset_mm: [0.0; 2],
    }
}

fn grey(rgba: &[u8], row: u32) -> u8 {
    let [wide, _] = SIZE;
    rgba[((row * wide + wide / 2) * 4) as usize]
}

/// The curve through a volume, as a device that holds the volume whole would hold it.
fn curve(volume: &Volume) -> Smoothed {
    Smoothed::of(volume, 2048, MAX_VOLUME_BYTES)
}

#[test]
fn between_slices_the_smooth_picture_is_on_the_bend_and_the_plain_one_is_off_it() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = squares();
    gpu.upload(&volume).expect("uploads");
    // Half way between slices 4 and 5 the true value is 20.25, shown as mid-grey.
    let narrow = WindowLevel::new(20.25, 10.0);
    let plain = gpu
        .render_plane(&view(&volume), narrow, SIZE)
        .expect("renders");
    assert!(
        gpu.smooth_planes(Some(&curve(&volume))),
        "there is room for it"
    );
    let smooth = gpu
        .render_plane(&view(&volume), narrow, SIZE)
        .expect("renders");
    let (plain_between, smooth_between) = (grey(&plain, 22), grey(&smooth, 22));
    assert!(
        smooth_between.abs_diff(128) <= 2,
        "the curve passes through 20.25: {smooth_between}"
    );
    assert!(
        plain_between.abs_diff(128) >= 5,
        "the blend of 16 and 25 is 20.5: {plain_between}"
    );
    // On the slices themselves the two say the same: slice 5 at row 21, slice 4 at row 23.
    for row in [21, 23] {
        let (plain, smooth) = (grey(&plain, row), grey(&smooth, row));
        assert!(
            plain.abs_diff(smooth) <= 2,
            "row {row}: {plain} and {smooth}"
        );
    }
}

#[test]
fn on_the_slices_themselves_the_two_pictures_agree() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = squares();
    gpu.upload(&volume).expect("uploads");
    let wide = WindowLevel::new(112.5, 225.0);
    let plain = gpu
        .render_plane(&view(&volume), wide, SIZE)
        .expect("renders");
    gpu.smooth_planes(Some(&curve(&volume)));
    let smooth = gpu
        .render_plane(&view(&volume), wide, SIZE)
        .expect("renders");
    let [across, down] = SIZE;
    for row in (1..down).step_by(2) {
        for column in 0..across {
            let at = ((row * across + column) * 4) as usize;
            let (plain, smooth) = (plain[at], smooth[at]);
            assert!(
                plain.abs_diff(smooth) <= 2,
                "row {row}, column {column}: {plain} and {smooth}"
            );
        }
    }
    assert!(
        smooth.chunks(4).all(|pixel| pixel[3] == 255),
        "every pixel is opaque"
    );
}

#[test]
fn without_the_curve_the_plain_picture_is_back_exactly() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = squares();
    gpu.upload(&volume).expect("uploads");
    let window = WindowLevel::new(20.25, 10.0);
    let plain = gpu
        .render_plane(&view(&volume), window, SIZE)
        .expect("renders");
    gpu.smooth_planes(Some(&curve(&volume)));
    let smooth = gpu
        .render_plane(&view(&volume), window, SIZE)
        .expect("renders");
    assert_ne!(smooth, plain);
    assert!(!gpu.smooth_planes(None), "nothing to draw from");
    let again = gpu
        .render_plane(&view(&volume), window, SIZE)
        .expect("renders");
    assert_eq!(again, plain);
}

#[test]
fn another_volume_is_not_drawn_from_the_curve_of_the_one_before() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = squares();
    let flat = Volume::new([6, 6, 16], vec![20.0; 576], Affine::identity()).expect("adds up");
    let window = WindowLevel::new(20.25, 10.0);
    gpu.upload(&flat).expect("uploads");
    let plain = gpu
        .render_plane(&view(&flat), window, SIZE)
        .expect("renders");
    gpu.upload(&volume).expect("uploads");
    gpu.smooth_planes(Some(&curve(&volume)));
    gpu.upload(&flat).expect("uploads");
    let after = gpu
        .render_plane(&view(&flat), window, SIZE)
        .expect("renders");
    assert_eq!(after, plain, "the curve went with its volume");
    // A curve that is not this volume's size is not used either.
    let small = Volume::new([2, 2, 2], vec![1.0; 8], Affine::identity()).expect("adds up");
    gpu.smooth_planes(Some(&curve(&small)));
    let still = gpu
        .render_plane(&view(&flat), window, SIZE)
        .expect("renders");
    assert_eq!(still, plain);
}

#[test]
fn the_curve_is_made_at_the_size_the_volume_is_held_at() {
    let volume = squares();
    for longest in [2048, 8, 4] {
        let held = Prepared::of(&volume, longest, MAX_VOLUME_BYTES);
        let smoothed = Smoothed::of(&volume, longest, MAX_VOLUME_BYTES);
        assert_eq!(smoothed.dims(), held.dims(), "a longest side of {longest}");
    }
    assert_eq!(curve(&volume).dims(), [6, 6, 16]);
}

#[test]
fn the_picture_is_the_curve_as_the_processor_works_it_out_sharp_edges_and_all() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    // The ball has hard edges, where the curve overshoots: the device must overshoot alike.
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    assert!(gpu.smooth_planes(Some(&curve(&volume))));
    let placement = volume.voxel_to_patient();
    let middle = Point::new(0.0, 0.0, 0.0);
    let plane = plane_of(&placement, volume.dims(), PlaneKind::Coronal, middle, true);
    let side = 64_u16;
    let extent = [f32::from(side); 2];
    let view = PlaneView::showing(
        plane.expect("a plane"),
        &volume.corners(),
        extent,
        1.0,
        [0.0; 2],
    );
    let window = ball_window();
    let picture = [u32::from(side); 2];
    let rgba = gpu.render_plane(&view, window, picture).expect("renders");
    let worked_out = coefficients(&volume);
    let to_voxel = volume.patient_to_voxel();
    let [cols, rows, slices] = volume.dims().map(f64::from);
    let (mut compared, mut apart, mut worst) = (0_u16, 0.0_f32, 0.0_f32);
    for row in 0..side {
        for column in 0..side {
            let pixel = [f32::from(column) + 0.5, f32::from(row) + 0.5];
            let at = to_voxel.apply(view.screen_to_patient(extent, pixel));
            // Well inside: at the very faces the two may differ on which side a pixel falls.
            let well_inside = (0.0..cols - 1.0).contains(&at.x)
                && (0.0..rows - 1.0).contains(&at.y)
                && (0.0..slices - 1.0).contains(&at.z);
            let value = sample_cubic(volume.dims(), &worked_out, [at.x, at.y, at.z]);
            let Some(value) = value.filter(|_| well_inside) else {
                continue;
            };
            let share = ((value - window.low()) / window.width).clamp(0.0, 1.0);
            let found = rgba[(usize::from(row) * usize::from(side) + usize::from(column)) * 4];
            let off = (f32::from(found) - share * 255.0).abs();
            compared += 1;
            apart += off;
            worst = worst.max(off);
        }
    }
    assert!(
        compared > 2000,
        "most of the picture is inside the volume: {compared}"
    );
    let mean = apart / f32::from(compared);
    assert!(mean < 1.0, "a grey level apart on average: {mean}");
    assert!(worst <= 6.0, "and never far apart: {worst}");
}
