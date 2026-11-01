//! A slice drawn on a graphics device that has stopped answering.

use super::*;

const SIZE: [u32; 2] = [60, 60];
const WINDOW: (f32, f32) = (7.5, 16.0);

fn ramp() -> Vec<f32> {
    (0..16u8).map(f32::from).collect()
}

/// A renderer holding the ramp, or `None` with a printed notice on a machine with no adapter.
fn holding_the_ramp() -> Option<Headless> {
    match Headless::new() {
        Ok(mut gpu) => {
            gpu.upload(4, 4, &ramp()).expect("uploads");
            Some(gpu)
        }
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

fn drawn(gpu: &mut Headless) -> Result<Vec<u8>, RenderError> {
    gpu.render(
        WindowLevel::new(WINDOW.0, WINDOW.1),
        &ViewTransform::fit(),
        SIZE,
    )
}

#[test]
fn a_slice_on_a_lost_device_is_not_drawn_and_that_is_said() {
    let Some(mut gpu) = holding_the_ramp() else {
        return;
    };
    assert_eq!(drawn(&mut gpu).expect("draws").len(), 60 * 60 * 4);
    gpu.gpu.lose();
    let lost = drawn(&mut gpu);
    assert!(matches!(lost, Err(RenderError::DeviceLost)), "{lost:?}");
    assert!(gpu.gpu.is_lost());
    // Asked to take another image, and asked again, it carries on without a panic.
    let upload = gpu.upload(4, 4, &ramp());
    assert!(matches!(upload, Err(RenderError::DeviceLost)), "{upload:?}");
    let again = drawn(&mut gpu);
    assert!(matches!(again, Err(RenderError::DeviceLost)), "{again:?}");
}
