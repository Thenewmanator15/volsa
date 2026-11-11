//! Values sampled from the scan into a region of the plugin's grid: straight-line blending
//! along each of the scan's own axes, a place within half a voxel of the scan's ends held to
//! its end voxel, and any place further out given the value for outside.

use volsa_core::Volume;
use volsa_models::RunError;

use super::{Axis, Grid, Region, room};

/// One place along one axis: the two scan voxels it lies between, as offsets into the scan's
/// values, and how far it lies towards the second.
#[derive(Debug, Clone, Copy)]
struct Reading {
    first: usize,
    second: usize,
    towards: f64,
}

/// The readings along one axis of the grid for the region's voxels: `None` where the place lies
/// outside the scan.
fn readings(axis: &Axis, from: isize, size: usize, scan_stride: usize) -> Vec<Option<Reading>> {
    let last = f64::from(axis.scan) - 1.0;
    (0..size)
        .map(|inside| {
            let k = (from + isize::try_from(inside).unwrap_or(isize::MAX)) as f64;
            let place = axis.origin + k * axis.stride;
            if place < -0.5 || place > last + 0.5 {
                return None;
            }
            let place = place.clamp(0.0, last);
            let low = place.floor();
            let high = (low + 1.0).min(last);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "held within the scan's voxels"
            )]
            let (low_at, high_at) = (low as usize, high as usize);
            Some(Reading {
                first: low_at * scan_stride,
                second: high_at * scan_stride,
                towards: place - low,
            })
        })
        .collect()
}

/// The values of the scan in a region of the grid, in the tensor's order (the last axis
/// fastest), `outside` wherever the region lies outside the scan.
///
/// # Errors
///
/// [`RunError::NoRoom`] when there is not the memory for the region's values.
pub fn sample(
    volume: &Volume,
    grid: &Grid,
    region: Region,
    outside: f32,
) -> Result<Vec<f32>, RunError> {
    let [cols, rows, _] = volume.dims().map(|side| side as usize);
    let scan_strides = [1, cols, cols * rows];
    let along = |place: usize| {
        let axis = grid.axes.get(place)?;
        let from = region.from.get(place).copied()?;
        let size = region.size.get(place).copied()?;
        let stride = scan_strides.get(axis.from).copied()?;
        Some(readings(axis, from, size, stride))
    };
    let (Some(first), Some(second), Some(third)) = (along(0), along(1), along(2)) else {
        return Err(RunError::Failed(
            "a grid of other than three axes".to_owned(),
        ));
    };
    let mut values = room(first.len() * second.len() * third.len())?;
    let stored = volume.values();
    let at = |offset: usize| f64::from(stored.get(offset).copied().unwrap_or(0.0));
    for a in &first {
        for b in &second {
            for c in &third {
                let (Some(a), Some(b), Some(c)) = (a, b, c) else {
                    values.push(outside);
                    continue;
                };
                values.push(blend(a, b, c, &at));
            }
        }
    }
    Ok(values)
}

/// The blend of the eight scan voxels around a place.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a scan's values are singles"
)]
fn blend(a: &Reading, b: &Reading, c: &Reading, at: &impl Fn(usize) -> f64) -> f32 {
    let mut total = 0.0;
    for (a_at, a_weight) in [(a.first, 1.0 - a.towards), (a.second, a.towards)] {
        for (b_at, b_weight) in [(b.first, 1.0 - b.towards), (b.second, b.towards)] {
            for (c_at, c_weight) in [(c.first, 1.0 - c.towards), (c.second, c.towards)] {
                let weight = a_weight * b_weight * c_weight;
                if weight != 0.0 {
                    total += weight * at(a_at + b_at + c_at);
                }
            }
        }
    }
    total as f32
}
