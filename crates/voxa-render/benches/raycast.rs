//! How long one picture of a volume takes to draw and read back, in each look, at full and
//! at half quality, with and without stepping over empty space. Run with
//! `cargo bench -p voxa-render`.
//!
//! The volume is a made-up head: 256 x 256 voxels in 64 slices, a ball with a brighter core.
//! The picture is 1280 x 800, the size of a large viewport. A second set draws a 512 x 512 x
//! 64 head as the window does, moving and still, against the target of 30 pictures a second,
//! and still from above, along its slices.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use voxa_core::{Affine, Point, Volume, WindowLevel};
use voxa_labels::{LabelSet, Part, Side, Trust};
use voxa_render::{
    Camera, HeadlessScene, Look, MAX_VOLUME_BYTES, NamedView, Prepared, RenderError, SceneParams,
};

const PICTURE: [u32; 2] = [1280, 800];

/// A ball of value 300 and 90 mm radius with a core of value 800 and 40 mm radius, in the
/// middle of 256 x 256 voxels of 1 mm in 64 slices 3 mm apart.
fn made_up_head(side: u16, slices: u16) -> Volume {
    // The head is the same size whatever the number of voxels across it.
    let mm = 256.0 / f64::from(side);
    let middle = |count: u16| (f64::from(count) - 1.0) / 2.0;
    let to_patient = Affine::from_columns(
        [mm, 0.0, 0.0],
        [0.0, mm, 0.0],
        [0.0, 0.0, 3.0],
        [
            -mm * middle(side),
            -mm * middle(side),
            -3.0 * middle(slices),
        ],
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
    let volume = made_up_head(256, 64);
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

/// The target the plan sets: a 512 x 512 x 64 volume in a 1280 x 800 viewport, drawn as the
/// window draws it. While the camera moves that is at half quality and half the size each way;
/// once it is still, in full. Thirty pictures a second is 33 ms a picture.
fn as_the_window_draws(c: &mut Criterion) {
    let Ok(mut gpu) = HeadlessScene::new() else {
        eprintln!("skipped: no graphics adapter here, not even a software one");
        return;
    };
    let volume = made_up_head(512, 64);
    gpu.upload(&volume).expect("uploads");
    let viewport = PICTURE.map(|side| side as f32);
    let camera = Camera::fitted(NamedView::Anterior, &volume.corners(), viewport);
    for (name, look) in [
        ("Surface", Look::Surface),
        ("VR", Look::Vr),
        ("MIP", Look::Mip),
    ] {
        let still = SceneParams {
            camera: camera.orbited([60.0, 30.0]),
            window: WindowLevel::new(500.0, 1000.0),
            look,
            cuts: Vec::new(),
            slice: None,
            quality: 1.0,
            skip_empty: true,
        };
        c.bench_function(&format!("512 x 512 x 64, {name}, still"), |b| {
            b.iter(|| black_box(gpu.render(&still, PICTURE).expect("renders")));
        });
        // Half the size each way, the same scene: the camera sees twice as far to the pixel.
        let moving = SceneParams {
            camera: still.camera.zoomed_about(0.5, [0.0, 0.0]),
            quality: 0.5,
            ..still.clone()
        };
        let half = PICTURE.map(|side| side / 2);
        c.bench_function(&format!("512 x 512 x 64, {name}, moving"), |b| {
            b.iter(|| black_box(gpu.render(&moving, half).expect("renders")));
        });
        // Along the slices, which are six times as far apart as the voxels within them.
        let above = SceneParams {
            camera: Camera::fitted(NamedView::Superior, &volume.corners(), viewport),
            ..still
        };
        c.bench_function(&format!("512 x 512 x 64, {name}, still, from above"), |b| {
            b.iter(|| black_box(gpu.render(&above, PICTURE).expect("renders")));
        });
    }
}

/// What a person waits for when a large series is first shown in 3D, after its slices are
/// decoded: making the volume ready for the graphics device and putting it there. The
/// volume is 512 x 512 voxels in 300 slices, 300 MB of values.
fn put_a_large_volume_on_the_device(c: &mut Criterion) {
    let Ok(mut gpu) = HeadlessScene::new() else {
        eprintln!("skipped: no graphics adapter here, not even a software one");
        return;
    };
    let volume = made_up_head(512, 300);
    c.bench_function("512 x 512 x 300 on to the device", |b| {
        b.iter(|| black_box(gpu.upload(&volume).expect("uploads")));
    });
    // The same in its two parts: the first can be done on a worker, and only the second
    // holds the window up.
    c.bench_function("512 x 512 x 300 made ready", |b| {
        b.iter(|| black_box(Prepared::of(&volume, 2048, MAX_VOLUME_BYTES)));
    });
    let ready = Prepared::of(&volume, 2048, MAX_VOLUME_BYTES);
    c.bench_function("512 x 512 x 300 handed to the device", |b| {
        b.iter(|| {
            black_box(gpu.upload_prepared(&ready).expect("uploads"));
            gpu.settle();
        });
    });
    // And with the largest value in each block left to the device: less to make ready, and
    // the device's own work counted in with the handing over.
    eprintln!("on {}", gpu.device_name());
    c.bench_function("512 x 512 x 300 made ready without its blocks", |b| {
        b.iter(|| black_box(Prepared::without_blocks(&volume, 2048, MAX_VOLUME_BYTES)));
    });
    let light = Prepared::without_blocks(&volume, 2048, MAX_VOLUME_BYTES);
    c.bench_function(
        "512 x 512 x 300 handed to the device, blocks worked out there",
        |b| {
            b.iter(|| {
                black_box(gpu.upload_prepared(&light).expect("uploads"));
                gpu.settle();
            });
        },
    );
}

/// What turning Smooth on waits for, on a worker: the coefficients of the curve through a
/// volume of 512 x 512 voxels in 64 slices.
fn work_out_the_curve(c: &mut Criterion) {
    let volume = made_up_head(512, 64);
    c.bench_function("512 x 512 x 64 curve coefficients", |b| {
        b.iter(|| black_box(voxa_volume::coefficients(&volume)));
    });
    // The same curve worked out by the graphics device from the volume it holds, and for a
    // series of 300 slices, counting the device's work until it is done.
    let Ok(mut gpu) = HeadlessScene::new() else {
        return;
    };
    for slices in [64, 300] {
        let volume = made_up_head(512, slices);
        gpu.upload(&volume).expect("uploads");
        c.bench_function(&format!("512 x 512 x {slices} curve, on the device"), |b| {
            b.iter(|| {
                assert!(gpu.smooth_planes_on_device(), "the device can");
                gpu.settle();
            });
        });
        if slices == 300 {
            c.bench_function("512 x 512 x 300 curve, on the processor", |b| {
                b.iter(|| black_box(voxa_render::Smoothed::of(&volume, 2048, MAX_VOLUME_BYTES)));
            });
        }
    }
}

/// Labelled parts drawn as solids: a made-up label volume the size of a whole-head set at
/// 1 mm (200 x 230 x 150 voxels), a ball split into eight parts, in a 1280 x 800 picture.
fn draw_the_parts(c: &mut Criterion) {
    let Ok(mut gpu) = HeadlessScene::new() else {
        eprintln!("skipped: no graphics adapter here, not even a software one");
        return;
    };
    let dims = [200_u32, 230, 150];
    let mut numbers = Vec::new();
    for slice in 0..150_i32 {
        for row in 0..230_i32 {
            for col in 0..200_i32 {
                let (x, y, z) = (col - 100, row - 115, slice - 75);
                let inside = x * x + y * y + z * z < 70 * 70;
                let eighth = 1 + u16::from(x > 0) + 2 * u16::from(y > 0) + 4 * u16::from(z > 0);
                numbers.push(if inside { eighth } else { 0 });
            }
        }
    }
    let part = |number: u8| Part {
        name: format!("Eighth {number}"),
        group: "Ball".to_owned(),
        side: Side::Midline,
        colour: [40 + 20 * number, 200 - 20 * number, 90 + 15 * number],
        trust: Trust::NotSaid,
        voxels: 0,
        ml: 0.0,
    };
    let set = LabelSet {
        dims,
        numbers,
        placement: Affine::from_columns(
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-100.0, -115.0, -75.0],
        ),
        parts: (1..=8).map(part).collect(),
        title: String::new(),
        verdict: None,
    };
    assert!(gpu.label(Some(&set)), "there is room for the labels");
    let viewport = PICTURE.map(|side| side as f32);
    let camera =
        Camera::fitted(NamedView::Anterior, &set.corners(), viewport).orbited([60.0, 30.0]);
    let scene = |quality| SceneParams {
        camera,
        window: WindowLevel::new(0.5, 1.0),
        look: Look::Parts,
        cuts: Vec::new(),
        slice: None,
        quality,
        skip_empty: false,
    };
    c.bench_function("200 x 230 x 150 parts, still", |b| {
        b.iter(|| black_box(gpu.render_parts(&scene(1.0), PICTURE).expect("renders")));
    });
    let half = PICTURE.map(|side| side / 2);
    c.bench_function("200 x 230 x 150 parts, moving", |b| {
        b.iter(|| black_box(gpu.render_parts(&scene(0.5), half).expect("renders")));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).measurement_time(Duration::from_secs(4));
    targets = draw_a_head, as_the_window_draws, put_a_large_volume_on_the_device,
        work_out_the_curve, draw_the_parts
}
criterion_main!(benches);
