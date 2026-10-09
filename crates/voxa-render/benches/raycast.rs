//! How long one picture of a volume takes to draw and read back, in each look, at full and
//! at half quality, with and without stepping over empty space. Run with
//! `cargo bench -p voxa-render`.
//!
//! The volume is a made-up head: 256 x 256 voxels in 64 slices, a ball with a brighter core.
//! The picture is 1280 x 800, the size of a large viewport.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use voxa_core::{Affine, Point, Volume, WindowLevel};
use voxa_render::{Camera, HeadlessScene, Look, NamedView, RenderError, SceneParams};

const PICTURE: [u32; 2] = [1280, 800];

/// A ball of value 300 and 90 mm radius with a core of value 800 and 40 mm radius, in the
/// middle of 256 x 256 voxels of 1 mm in 64 slices 3 mm apart.
fn made_up_head() -> Volume {
    let (side, slices) = (256_u16, 64_u16);
    let middle = |count: u16| (f64::from(count) - 1.0) / 2.0;
    let to_patient = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 3.0],
        [-middle(side), -middle(side), -3.0 * middle(slices)],
    );
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..side {
            for col in 0..side {
                let voxel = Point::new(f64::from(col), f64::from(row), f64::from(slice));
                let at = to_patient.apply(voxel);
                let from_middle = (at.x * at.x + at.y * at.y + at.z * at.z).sqrt();
                values.push(match from_middle {
                    mm if mm <= 40.0 => 800.0,
                    mm if mm <= 90.0 => 300.0,
                    _ => 0.0,
                });
            }
        }
    }
    let dims = [u32::from(side), u32::from(side), u32::from(slices)];
    Volume::new(dims, values, to_patient).expect("the values fill the volume")
}

fn draw_a_head(c: &mut Criterion) {
    let mut gpu = match HeadlessScene::new() {
        Ok(gpu) => gpu,
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            return;
        }
        Err(other) => panic!("{other}"),
    };
    let volume = made_up_head();
    gpu.upload(&volume).expect("uploads");
    let viewport = PICTURE.map(|side| side as f32);
    // Turned a little, so that lines of sight cross the blocks of the volume at a slant.
    let camera = Camera::fitted(NamedView::Anterior, &volume.corners(), viewport);
    let scene = SceneParams {
        camera: camera.orbited([60.0, 30.0]),
        window: WindowLevel::new(500.0, 1000.0),
        look: Look::Surface,
        cuts: Vec::new(),
        slice: None,
        quality: 1.0,
        skip_empty: false,
    };
    let looks = [
        ("Surface", Look::Surface),
        ("VR", Look::Vr),
        ("MIP", Look::Mip),
    ];
    for (name, look) in looks {
        for quality in [1.0, 0.5] {
            for skip_empty in [false, true] {
                if skip_empty && look == Look::Mip {
                    continue; // MIP never skips, so the two would be the same measurement.
                }
                let scene = SceneParams {
                    look,
                    quality,
                    skip_empty,
                    ..scene.clone()
                };
                let skipping = if skip_empty {
                    "skipping empty space"
                } else {
                    "every step"
                };
                c.bench_function(&format!("{name}, quality {quality}, {skipping}"), |b| {
                    b.iter(|| black_box(gpu.render(&scene, PICTURE).expect("renders")));
                });
            }
        }
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).measurement_time(Duration::from_secs(4));
    targets = draw_a_head
}
criterion_main!(benches);
