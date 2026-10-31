//! Measuring on a volume: lengths, angles, and the scan values inside a region. Everything
//! is worked out in patient millimetres and scan values, never from a picture on the screen.

use serde::{Deserialize, Serialize};
use voxa_core::{Grid, Patient, Point};

use crate::plane::Plane;
use crate::sample::sample;

/// The most samples taken across a region either way. A larger region is sampled more
/// coarsely, so that no region takes long to measure.
const MOST_ACROSS: f64 = 1024.0;

/// The straight-line distance between two points of the patient, in millimetres.
pub fn distance_mm(a: Point<Patient>, b: Point<Patient>) -> f64 {
    let apart = between(a, b);
    dot(apart, apart).sqrt()
}

/// The angle at `vertex` between the lines to `a` and to `b`, from 0 to 180 degrees. `None`
/// when either line has no length, as there is then no angle to speak of.
pub fn angle_degrees(a: Point<Patient>, vertex: Point<Patient>, b: Point<Patient>) -> Option<f64> {
    let (one, other) = (between(vertex, a), between(vertex, b));
    let lengths = distance_mm(vertex, a) * distance_mm(vertex, b);
    if !(lengths.is_finite() && lengths > 0.0) {
        return None;
    }
    let cosine = dot(one, other) / lengths;
    Some(cosine.clamp(-1.0, 1.0).acos().to_degrees())
}

/// What the scan holds inside a region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionStats {
    /// The mean of the scan values sampled.
    pub mean: f64,
    /// How far they spread about the mean: their standard deviation.
    pub sd: f64,
    /// The smallest of them.
    pub min: f32,
    /// The largest of them.
    pub max: f32,
    /// The area of the region as drawn, in square millimetres, whatever of it lay outside
    /// the scan.
    pub area_mm2: f64,
    /// How many samples were inside the scan.
    pub samples: usize,
}

/// The scan values inside an ellipse drawn on a plane: its middle `centre_mm` across and down
/// from the middle of the plane, its half-widths `radii_mm` along the plane's sides.
///
/// The ellipse is sampled on a square grid half the volume's finest voxel spacing apart, each
/// sample blended from the eight voxels round it, and only samples inside the volume are
/// counted. `None` when a radius is not above zero or no sample falls inside the volume.
pub fn ellipse_stats(
    volume: &(impl Grid + ?Sized),
    plane: &Plane,
    centre_mm: [f64; 2],
    radii_mm: [f64; 2],
) -> Option<RegionStats> {
    let [wide, high] = radii_mm;
    if !(wide > 0.0 && high > 0.0 && wide.is_finite() && high.is_finite()) {
        return None;
    }
    let [across, down] = centre_mm;
    // Only the part of the ellipse's box that the volume reaches is gone over, so a region
    // far larger than the scan is no more work than the scan itself.
    let reached = reach(volume, plane);
    let (left, right) = overlap((across - wide, across + wide), reached.0)?;
    let (top, bottom) = overlap((down - high, down + high), reached.1)?;
    let finest = volume
        .spacing_mm()
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    let apart = (finest / 2.0).max((right - left).max(bottom - top) / MOST_ACROSS);
    if !(apart > 0.0 && apart.is_finite()) {
        return None;
    }
    let mut seen = Seen::default();
    for row in spread(top, bottom, apart) {
        for col in spread(left, right, apart) {
            let inside = ((col - across) / wide).powi(2) + ((row - down) / high).powi(2) <= 1.0;
            let value = inside.then(|| sample(volume, plane.at([col, row])));
            if let Some(value) = value.flatten() {
                seen.take(value);
            }
        }
    }
    seen.stats(std::f64::consts::PI * wide * high)
}

/// How far the volume's box reaches across and down a plane, each as (least, most), in
/// millimetres from the plane's middle.
fn reach(volume: &(impl Grid + ?Sized), plane: &Plane) -> ((f64, f64), (f64, f64)) {
    let none = (f64::INFINITY, f64::NEG_INFINITY);
    let widen = |(least, most): (f64, f64), at: f64| (least.min(at), most.max(at));
    let on_plane = volume.corners().map(|corner| plane.of(corner).0);
    on_plane
        .into_iter()
        .fold((none, none), |(across, down), [col, row]| {
            (widen(across, col), widen(down, row))
        })
}

/// The part two stretches share, if they share any.
fn overlap(one: (f64, f64), other: (f64, f64)) -> Option<(f64, f64)> {
    let (from, to) = (one.0.max(other.0), one.1.min(other.1));
    (from < to).then_some((from, to))
}

/// Where samples fall along a stretch: every `apart` millimetres, the first half a step in.
fn spread(from: f64, to: f64, apart: f64) -> impl Iterator<Item = f64> + Clone {
    let count = whole(((to - from) / apart).ceil());
    (0..count).map(move |at| from + (f64::from(at) + 0.5) * apart)
}

/// The samples taken so far, summed up.
#[derive(Default)]
struct Seen {
    count: usize,
    sum: f64,
    squares: f64,
    least: Option<f32>,
    most: Option<f32>,
}

impl Seen {
    fn take(&mut self, value: f32) {
        let wide = f64::from(value);
        self.count += 1;
        self.sum += wide;
        self.squares += wide * wide;
        self.least = Some(self.least.map_or(value, |least| least.min(value)));
        self.most = Some(self.most.map_or(value, |most| most.max(value)));
    }

    fn stats(&self, area_mm2: f64) -> Option<RegionStats> {
        let (min, max) = self.least.zip(self.most)?;
        let count = self.count as f64;
        let mean = self.sum / count;
        // Rounding can leave a hair below zero where every value is the same.
        let spread = (self.squares / count - mean * mean).max(0.0);
        Some(RegionStats {
            mean,
            sd: spread.sqrt(),
            min,
            max,
            area_mm2,
            samples: self.count,
        })
    }
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

/// From one point to another, in millimetres along the patient's axes.
fn between(from: Point<Patient>, to: Point<Patient>) -> [f64; 3] {
    [to.x - from.x, to.y - from.y, to.z - from.z]
}

/// A count of steps that has been held to a small whole number already.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and twice MOST_ACROSS first"
)]
fn whole(count: f64) -> u32 {
    count.clamp(0.0, MOST_ACROSS + 1.0) as u32
}

#[cfg(test)]
mod tests;
