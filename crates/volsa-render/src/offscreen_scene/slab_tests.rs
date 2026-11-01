//! A volume held in several textures along its slices draws as the same volume held in one:
//! no seam where two textures meet, nothing moved, and the same blocks of largest values.

use std::sync::Arc;

use volsa_core::{HeldVolume, Patient, Point, Rescale, SliceIn, Stored, Volume};
use volsa_volume::{PlaneKind, plane_of};

use super::*;
use crate::camera::NamedView;
use crate::plane_view::PlaneView;
use crate::scene::Look;
use crate::testing::{MM, SIDE, ball_window, centred, grey, headless_scene, scene_of};
use crate::upload::CardRoom;

const DIMS: [u32; 3] = [32, 32, 40];
/// Where the bead lies: column, row and slice.
const BEAD: [f64; 3] = [20.0, 12.0, 20.0];

/// A made-up volume: a block whose values rise slice by slice, across the places where the
/// slabs below meet, and a bright bead off the middle in slices 18 to 22.
fn values() -> Vec<u16> {
    let [cols, rows, slices] = DIMS;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            for col in 0..cols {
                let [dc, dr, ds] = [col, row, slice].map(f64::from);
                let from_bead = (dc - BEAD[0]).hypot(dr - BEAD[1]).hypot(ds - BEAD[2]);
                let inside = (6..26).contains(&col) && (6..26).contains(&row);
                let value = if from_bead <= 2.5 {
                    390
                } else if inside {
                    150 + 5 * slice
                } else {
                    0
                };
                values.push(u16::try_from(value).expect("small"));
            }
        }
    }
    values
}

fn volume() -> Volume {
    let to_patient = centred(DIMS.map(|side| u16::try_from(side).expect("small")), MM);
    let values = values().into_iter().map(f32::from).collect();
    Volume::new(DIMS, values, to_patient).expect("adds up")
}

fn held() -> Arc<HeldVolume> {
    let volume = volume();
    let held = HeldVolume::new(
        DIMS,
        volume.voxel_to_patient(),
        Stored::Unsigned16,
        Rescale::default(),
        1 << 20,
    )
    .expect("a volume");
    let in_a_slice = (DIMS[0] * DIMS[1]) as usize;
    for (slice, numbers) in (0..).zip(values().chunks(in_a_slice)) {
        held.put_slice(slice, SliceIn::Unsigned(numbers))
            .expect("fits");
    }
    Arc::new(held)
}

/// A device's room, made so small that the volume goes into three textures, of 17, 17
/// and 8 slices, the slices 16 and 32 in two each.
fn three_slabs(gpu: &HeadlessScene) -> CardRoom {
    CardRoom {
        texture_bytes: 17 * u64::from(DIMS[0] * DIMS[1]) * 2,
        ..CardRoom::of(&gpu.gpu.device, None)
    }
}

/// The plane of `kind` through a point given in voxels.
fn plane(volume: &Volume, kind: PlaneKind, voxel: [f64; 3]) -> PlaneView {
    let [x, y, z] = voxel;
    let at = volume.voxel_to_patient().apply(Point::new(x, y, z));
    let at: Point<Patient> = at;
    let plane = plane_of(&volume.voxel_to_patient(), DIMS, kind, at, true).expect("a plane");
    PlaneView::showing(plane, &volume.corners(), [SIDE as f32; 2], 1.0, [0.0; 2])
}

/// Every picture compared: the scene seen from the front and the side in each look, and
/// planes across, along and down the slices through the bead and through a join.
fn pictures(gpu: &mut HeadlessScene, volume: &Volume) -> Vec<(String, Vec<u8>)> {
    let mut drawn = Vec::new();
    for look in [Look::Vr, Look::Mip, Look::Surface] {
        for side in [NamedView::Anterior, NamedView::Left] {
            let scene = scene_of(volume, side, look);
            let picture = gpu.render(&scene, [SIDE, SIDE]).expect("draws");
            drawn.push((format!("{look:?} from {side:?}"), picture));
        }
    }
    let planes = [
        (PlaneKind::Axial, [16.0, 16.0, 16.0]),
        (PlaneKind::Axial, [16.0, 16.0, 16.5]),
        (PlaneKind::Axial, BEAD),
        (PlaneKind::Sagittal, BEAD),
        (PlaneKind::Coronal, BEAD),
    ];
    for (kind, through) in planes {
        let view = plane(volume, kind, through);
        let picture = gpu
            .render_plane(&view, ball_window(), [SIDE, SIDE])
            .expect("draws");
        drawn.push((format!("{kind:?} through {through:?}"), picture));
    }
    drawn
}

/// The most two pictures differ at any pixel, in grey levels.
fn most_apart(one: &[u8], other: &[u8]) -> u8 {
    (0..SIDE * SIDE)
        .map(|pixel| {
            grey(one, pixel % SIDE, pixel / SIDE).abs_diff(grey(other, pixel % SIDE, pixel / SIDE))
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn the_room_given_makes_three_slabs() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let room = three_slabs(&gpu);
    assert_eq!(room.textures(DIMS), Some(3));
    gpu.upload_held_in(&held(), room).expect("uploads");
    let (placed, _) = gpu.pipeline.placed().expect("a volume");
    assert_eq!(placed.plan.depths, [17, 17, 8]);
}

#[test]
fn no_seam_where_slabs_meet_and_the_bead_is_where_it_was() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = volume();
    let held = held();
    gpu.upload_held(&held).expect("uploads");
    let whole = pictures(&mut gpu, &volume);
    let room = three_slabs(&gpu);
    gpu.upload_held_in(&held, room).expect("uploads");
    let slabs = pictures(&mut gpu, &volume);
    for ((name, one), (_, other)) in whole.iter().zip(&slabs) {
        assert!(
            one.iter().any(|&byte| byte > 0 && byte < 255),
            "{name} shows something"
        );
        let apart = most_apart(one, other);
        assert!(apart <= 1, "{name}: {apart} grey levels apart");
    }
}

#[test]
fn the_blocks_of_largest_values_are_the_same_in_slabs() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let held = held();
    gpu.upload_held(&held).expect("uploads");
    let whole = gpu.blocks_as_held().expect("blocks");
    let room = three_slabs(&gpu);
    gpu.upload_held_in(&held, room).expect("uploads");
    assert_eq!(gpu.blocks_as_held().expect("blocks"), whole);
}

#[test]
fn a_volume_in_slabs_has_no_curve_made_on_the_device() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let held = held();
    gpu.upload_held(&held).expect("uploads");
    assert!(!gpu.pipeline.held_in_slabs());
    let room = three_slabs(&gpu);
    gpu.upload_held_in(&held, room).expect("uploads");
    assert!(gpu.pipeline.held_in_slabs());
    let (device, queue) = (&gpu.gpu.device, &gpu.gpu.queue);
    assert!(gpu.pipeline.curve_on_device(device, queue).is_none());
}
