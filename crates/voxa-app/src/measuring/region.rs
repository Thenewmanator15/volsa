//! What a region drawn on a slice holds, worked out from the slice's own values where they lie,
//! once, as the region is finished.

use voxa_core::{Patient, Point};
use voxa_dicom::Frame;
use voxa_volume::RegionStats;

use super::{SlicePlacement, dot};

/// The most samples taken across a region either way, as on the planes. A larger region is
/// sampled more coarsely, so that no region takes long to measure.
const MOST_ACROSS: f64 = 1024.0;

/// One slice's values, row by row, borrowed from wherever they are kept: the decoded slice, or
/// the series' volume.
#[derive(Debug, Clone, Copy)]
pub struct SliceValues<'a> {
    /// The values, row by row.
    pub values: &'a [f32],
    /// How many columns and rows the slice has.
    pub size: [u32; 2],
}

impl<'a> From<&'a Frame> for SliceValues<'a> {
    fn from(frame: &'a Frame) -> Self {
        Self {
            values: &frame.values,
            size: [frame.cols, frame.rows],
        }
    }
}

impl SliceValues<'_> {
    /// The value of the pixel at `(column, row)`. `None` when the pixel is not in the slice.
    pub fn at(&self, (col, row): (usize, usize)) -> Option<f32> {
        let [cols, rows] = self.size.map(|count| usize::try_from(count).unwrap_or(0));
        if col >= cols || row >= rows {
            return None;
        }
        self.values
            .get(row.checked_mul(cols)?.checked_add(col)?)
            .copied()
    }

    /// The value of the pixel a place on the slice lies in, the place given as a column and a
    /// row that count from the middles of pixels. `None` off the slice, and for a value that
    /// is not a number.
    fn nearest(&self, column: f64, row: f64) -> Option<f32> {
        let [cols, rows] = self.size;
        let pixel = |at: f64, count: u32| {
            let nearest = (at + 0.5).floor();
            (0.0..f64::from(count))
                .contains(&nearest)
                .then(|| whole(nearest))
        };
        let value = self.at((pixel(column, cols)?, pixel(row, rows)?))?;
        value.is_finite().then_some(value)
    }
}

/// The ellipse a region is, in the millimetres of its slice: its middle, and the ways its two
/// half-widths run, each of length 1.
struct Ellipse {
    centre: [f64; 3],
    across: [f64; 3],
    down: [f64; 3],
}

impl Ellipse {
    /// The point `across` and `down` millimetres from the middle along the ellipse's sides.
    fn at(&self, across: f64, down: f64) -> Point<Patient> {
        let [x, y, z] = [0, 1, 2].map(|part| {
            let along = |v: [f64; 3]| v.get(part).copied().unwrap_or(0.0);
            along(self.centre) + across * along(self.across) + down * along(self.down)
        });
        Point::new(x, y, z)
    }

    /// Where a point falls along the ellipse's sides, in millimetres from its middle.
    fn of(&self, point: Point<Patient>) -> [f64; 2] {
        let [cx, cy, cz] = self.centre;
        let from = [point.x - cx, point.y - cy, point.z - cz];
        [dot(from, self.across), dot(from, self.down)]
    }
}

/// The values of slice `slice` inside an ellipse drawn on it, about `centre`, its half-widths
/// `radii_mm` along `across` and `down`, the ways the slice's rows and columns run.
///
/// The ellipse is gone over on a square grid half the slice's finer pixel spacing apart, as a
/// region on the planes is, and each sample takes the value of the pixel it lies in: the
/// slice's own values, never a blend of them. Only the part of the ellipse the slice reaches is
/// gone over. `None` when a half-width is not above zero or no sample falls on the slice.
pub fn frame_ellipse_stats(
    frame: SliceValues<'_>,
    at: &SlicePlacement,
    slice: usize,
    centre: [f64; 3],
    radii_mm: [f64; 2],
    across: [f64; 3],
    down: [f64; 3],
) -> Option<RegionStats> {
    let [wide, high] = radii_mm;
    if !(wide > 0.0 && high > 0.0 && wide.is_finite() && high.is_finite()) {
        return None;
    }
    let to_pixel = at.placement.inverse()?;
    let ellipse = Ellipse {
        centre,
        across,
        down,
    };
    let (reach_across, reach_down) = reach(frame.size, at, slice, &ellipse);
    let (left, right) = overlap((-wide, wide), reach_across)?;
    let (top, bottom) = overlap((-high, high), reach_down)?;
    let [along_rows, along_columns, _] = at.placement.columns();
    let finest = dot(along_rows, along_rows)
        .sqrt()
        .min(dot(along_columns, along_columns).sqrt());
    let apart = (finest / 2.0).max((right - left).max(bottom - top) / MOST_ACROSS);
    if !(apart > 0.0 && apart.is_finite()) {
        return None;
    }
    let mut seen = Seen::default();
    for down_mm in spread(top, bottom, apart) {
        for across_mm in spread(left, right, apart) {
            if (across_mm / wide).powi(2) + (down_mm / high).powi(2) > 1.0 {
                continue;
            }
            let pixel = to_pixel.apply(ellipse.at(across_mm, down_mm));
            if let Some(value) = frame.nearest(pixel.x, pixel.y) {
                seen.take(value);
            }
        }
    }
    seen.stats(std::f64::consts::PI * wide * high)
}

/// How far a slice of `size` (columns, rows) reaches along the ellipse's two sides, each as
/// (least, most), in millimetres from its middle: from the outer edges of its corner pixels.
fn reach(
    size: [u32; 2],
    at: &SlicePlacement,
    slice: usize,
    ellipse: &Ellipse,
) -> ((f64, f64), (f64, f64)) {
    let [cols, rows] = size.map(|count| f64::from(count) - 0.5);
    let corners = [(-0.5, -0.5), (cols, -0.5), (-0.5, rows), (cols, rows)];
    let none = (f64::INFINITY, f64::NEG_INFINITY);
    let widen = |(least, most): (f64, f64), mm: f64| (least.min(mm), most.max(mm));
    corners
        .iter()
        .fold((none, none), |(across, down), (col, row)| {
            let corner = at.placement.apply(Point::new(*col, *row, slice as f64));
            let [along, below] = ellipse.of(corner);
            (widen(across, along), widen(down, below))
        })
}

/// The part two stretches share, if they share any.
fn overlap(one: (f64, f64), other: (f64, f64)) -> Option<(f64, f64)> {
    let (from, to) = (one.0.max(other.0), one.1.min(other.1));
    (from < to).then_some((from, to))
}

/// Where samples fall along a stretch: every `apart` millimetres, the first half a step in.
fn spread(from: f64, to: f64, apart: f64) -> impl Iterator<Item = f64> + Clone {
    let count = steps(((to - from) / apart).ceil());
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

/// A count of steps that has been held to a small whole number already.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and just over MOST_ACROSS first"
)]
fn steps(count: f64) -> u32 {
    count.clamp(0.0, MOST_ACROSS + 1.0) as u32
}

/// A pixel's place, checked to lie between 0 and the number of pixels first.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "checked to lie between 0 and the number of pixels first"
)]
fn whole(at: f64) -> usize {
    at as usize
}
