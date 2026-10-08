//! Pictures drawn on a graphics device with no window, checked pixel by pixel.

use super::*;

/// Fifteen screen pixels per voxel for a 4 × 4 image: an odd number, so each cell has a
/// pixel exactly at its centre, where no neighbour is blended in.
const SIZE: [u32; 2] = [60, 60];

fn ramp() -> Vec<f32> {
    (0..16u8).map(f32::from).collect()
}

/// The grey at the centre of the cell showing voxel (`col`, `row`).
fn centre(rgba: &[u8], col: usize, row: usize) -> u8 {
    let (x, y) = (col * 15 + 7, row * 15 + 7);
    rgba[(y * 60 + x) * 4]
}

/// A renderer, or `None` with a printed notice on a machine with no graphics adapter.
fn headless() -> Option<Headless> {
    match Headless::new() {
        Ok(gpu) => Some(gpu),
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

fn drawn(gpu: &mut Headless, wl: WindowLevel, view: &ViewTransform) -> Vec<u8> {
    gpu.render(wl, view, SIZE).expect("renders")
}

#[test]
fn a_ramp_is_drawn_with_each_cell_at_its_own_grey() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    let rgba = drawn(&mut gpu, WindowLevel::new(7.5, 16.0), &ViewTransform::fit());
    assert_eq!(rgba.len(), 60 * 60 * 4);
    for row in 0..4 {
        for col in 0..4 {
            // Black at 7.5 - 8 = -0.5 and white at 15.5.
            let value = (row * 4 + col) as f32;
            let expected = ((value + 0.5) / 16.0 * 255.0).round();
            let found = f32::from(centre(&rgba, col, row));
            assert!(
                (found - expected).abs() <= 2.0,
                "({col}, {row}): {found} for {expected}"
            );
        }
    }
    let grey = |px: &[u8; 4]| px[0] == px[1] && px[1] == px[2] && px[3] == 255;
    assert!(
        rgba.as_chunks::<4>().0.iter().all(grey),
        "every pixel is an opaque grey"
    );
}

#[test]
fn a_window_one_wide_makes_each_cell_black_or_white() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    let rgba = drawn(&mut gpu, WindowLevel::new(7.5, 1.0), &ViewTransform::fit());
    for row in 0..4 {
        for col in 0..4 {
            let expected = if row * 4 + col <= 7 { 0 } else { 255 };
            assert_eq!(centre(&rgba, col, row), expected, "({col}, {row})");
        }
    }
}

#[test]
fn changing_the_window_changes_the_picture_without_a_new_upload() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    let wide = drawn(&mut gpu, WindowLevel::new(7.5, 16.0), &ViewTransform::fit());
    let narrow = drawn(&mut gpu, WindowLevel::new(3.0, 4.0), &ViewTransform::fit());
    assert_ne!(wide, narrow);
    let again = drawn(&mut gpu, WindowLevel::new(7.5, 16.0), &ViewTransform::fit());
    assert_eq!(again, wide);
}

#[test]
fn the_view_decides_where_each_voxel_is_drawn() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    let wl = WindowLevel::new(8.0, 16.0); // grey = value x 16, near enough
    let flipped = ViewTransform {
        flip_h: true,
        ..ViewTransform::fit()
    };
    let rgba = drawn(&mut gpu, wl, &flipped);
    assert!(
        centre(&rgba, 0, 0).abs_diff(3 * 16) <= 2,
        "voxel 3 is now at the top left"
    );
    let turned = ViewTransform::fit().turned_clockwise();
    let rgba = drawn(&mut gpu, wl, &turned);
    assert!(
        centre(&rgba, 3, 0).abs_diff(0) <= 2,
        "voxel 0 has turned to the top right"
    );
    assert!(
        centre(&rgba, 0, 0).abs_diff(12 * 16) <= 2,
        "voxel 12 has turned to the top left"
    );
}

#[test]
fn outside_the_image_is_black() {
    let Some(mut gpu) = headless() else { return };
    // Four columns by two rows leaves a band above and below in a square viewport.
    gpu.upload(2, 4, &[100.0; 8]).expect("uploads");
    let rgba = drawn(
        &mut gpu,
        WindowLevel::new(50.0, 100.0),
        &ViewTransform::fit(),
    );
    let at = |x: usize, y: usize| rgba[(y * 60 + x) * 4];
    assert_eq!((at(30, 2), at(30, 57)), (0, 0), "the bands");
    assert_eq!(at(30, 30), 255, "the image");
}

#[test]
fn a_new_upload_replaces_the_picture_even_at_another_size() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    gpu.upload(2, 2, &[0.0, 0.0, 0.0, 16.0]).expect("uploads");
    let rgba = drawn(&mut gpu, WindowLevel::new(8.0, 16.0), &ViewTransform::fit());
    assert_eq!(
        (rgba[(15 * 60 + 15) * 4], rgba[(45 * 60 + 45) * 4]),
        (0, 255)
    );
}

#[test]
fn nothing_uploaded_draws_a_black_picture() {
    let Some(mut gpu) = headless() else { return };
    let rgba = drawn(&mut gpu, WindowLevel::new(0.0, 1.0), &ViewTransform::fit());
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .all(|px| *px == [0, 0, 0, 255])
    );
}

#[test]
fn a_frame_whose_values_do_not_match_its_size_is_refused() {
    let Some(mut gpu) = headless() else { return };
    let short = gpu.upload(4, 4, &[0.0; 15]);
    assert!(matches!(
        short,
        Err(RenderError::FrameSize {
            rows: 4,
            cols: 4,
            values: 15
        })
    ));
    assert!(matches!(
        gpu.upload(0, 0, &[]),
        Err(RenderError::FrameSize { .. })
    ));
    let huge = gpu.upload(1, 1_000_000, &vec![0.0; 1_000_000]);
    assert!(
        matches!(huge, Err(RenderError::TooLarge { .. })),
        "{huge:?}"
    );
}

#[test]
fn a_picture_of_no_size_is_refused() {
    let Some(mut gpu) = headless() else { return };
    let result = gpu.render(WindowLevel::new(0.0, 1.0), &ViewTransform::fit(), [0, 60]);
    assert!(matches!(result, Err(RenderError::EmptyTarget)));
}

#[test]
fn render_to_rgba_draws_in_one_call() {
    let Some(mut gpu) = headless() else { return };
    gpu.upload(4, 4, &ramp()).expect("uploads");
    let wl = WindowLevel::new(7.5, 16.0);
    let stepwise = drawn(&mut gpu, wl, &ViewTransform::fit());
    let at_once = render_to_rgba(4, 4, &ramp(), wl, &ViewTransform::fit(), SIZE).expect("renders");
    assert_eq!(at_once, stepwise);
}

#[test]
fn errors_read_as_plain_sentences() {
    assert_eq!(
        RenderError::NoAdapter.to_string(),
        "No graphics adapter is available, not even a software one."
    );
    assert_eq!(
        RenderError::FrameSize {
            rows: 4,
            cols: 4,
            values: 15
        }
        .to_string(),
        "The image has 15 values but its size is 4 × 4."
    );
}
