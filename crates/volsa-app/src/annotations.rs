//! Measurements and markers made on a series: what they are, how they are kept with their
//! history so that a change can be undone, and what each one says.

use serde::{Deserialize, Serialize};
use volsa_core::{Affine, Patient, Point, Voxel};
use volsa_volume::{RegionStats, angle_degrees, distance_mm, pitch_along};

use crate::measuring::Scale;

pub use kept::{ANNOTATIONS_VERSION, load_annotations, save_annotations};
pub use left::Left;
#[cfg(test)]
pub use left::MOST as MOST_LEFT;

/// The most annotations one series may have.
pub const MAX_ANNOTATIONS: usize = 200;
/// How many changes can be undone.
pub const MAX_HISTORY: usize = 100;
/// The most characters a label may have.
pub const MAX_LABEL: usize = 60;

/// What the main button does in the plane views and the 3D scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Moves the position, or turns the scene.
    #[default]
    Navigate,
    /// Draws a straight line and gives its length.
    Ruler,
    /// Draws two lines from a point and gives the angle between them.
    Angle,
    /// Draws an ellipse and gives the scan values inside it.
    Region,
    /// Places a named point.
    Marker,
}

/// What an annotation is, in patient coordinates: DICOM's, in millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    /// A straight line from `a` to `b`.
    Ruler {
        /// One end.
        a: [f64; 3],
        /// The other end.
        b: [f64; 3],
    },
    /// The angle at `vertex` between the lines to `a` and to `b`.
    Angle {
        /// The end of one arm.
        a: [f64; 3],
        /// Where the arms meet.
        vertex: [f64; 3],
        /// The end of the other arm.
        b: [f64; 3],
    },
    /// An ellipse about `centre`, in the plane of `across` and `down`.
    Region {
        /// Its middle.
        centre: [f64; 3],
        /// The way its first half-width runs, of length 1.
        across: [f64; 3],
        /// The way its second half-width runs, of length 1.
        down: [f64; 3],
        /// Its two half-widths, in millimetres.
        radii_mm: [f64; 2],
        /// The scan values inside it, worked out when it was drawn; `None` if it lay
        /// outside the scan.
        stats: Option<RegionStats>,
    },
    /// A point.
    Marker {
        /// Where it is.
        at: [f64; 3],
    },
}

impl Shape {
    /// What kind of annotation it is, as a word.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Ruler { .. } => "Ruler",
            Self::Angle { .. } => "Angle",
            Self::Region { .. } => "Region",
            Self::Marker { .. } => "Marker",
        }
    }

    /// The points that place it, the first being where the position goes when it is chosen
    /// from the list: a ruler's ends, an angle's vertex and then its arms' ends, a region's
    /// middle, a marker.
    pub fn points(&self) -> Vec<[f64; 3]> {
        match self {
            Self::Ruler { a, b } => vec![*a, *b],
            Self::Angle { a, vertex, b } => vec![*vertex, *a, *b],
            Self::Region { centre, .. } => vec![*centre],
            Self::Marker { at } => vec![*at],
        }
    }
}

/// One measurement or marker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// Its number, which no other annotation of the series has had.
    pub id: u64,
    /// What the person calls it.
    pub label: String,
    /// What it is.
    pub shape: Shape,
    /// Whose millimetres its points are in. What 0.6.0 kept has none, and was measured in a 3D
    /// view, in the patient's.
    #[serde(default)]
    pub scale: Scale,
}

impl Annotation {
    /// The name an annotation has until it is given one: its kind and number.
    fn default_label(&self) -> String {
        format!("{} {}", self.shape.kind(), self.id)
    }
}

/// The annotations of the series in the scene, with what they were before each of the last
/// changes, so that a change can be undone and done again.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Annotations {
    /// The annotations, oldest first.
    pub items: Vec<Annotation>,
    /// What the list was before each change, oldest first.
    undo: Vec<Vec<Annotation>>,
    /// What the list was before each undoing, oldest first.
    redo: Vec<Vec<Annotation>>,
    /// The highest number given out so far.
    numbered: u64,
    /// Whether what is kept for the series has been asked for and has not come yet.
    awaited: bool,
}

impl Annotations {
    /// A list as it was read from where it is kept: with nothing to undo.
    pub fn loaded(items: Vec<Annotation>) -> Self {
        let numbered = items.iter().map(|item| item.id).max().unwrap_or(0);
        Self {
            items,
            numbered,
            ..Self::default()
        }
    }

    /// Whether there is a change to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is an undone change to do again.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Whether the list has as many annotations as it may.
    pub fn is_full(&self) -> bool {
        self.items.len() >= MAX_ANNOTATIONS
    }

    /// Add an annotation in the millimetres of `scale`, named by its kind and number. False,
    /// with nothing added, when the list is full.
    pub(crate) fn add(&mut self, shape: Shape, scale: Scale) -> bool {
        if self.is_full() {
            return false;
        }
        self.step();
        self.numbered += 1;
        let mut added = Annotation {
            id: self.numbered,
            label: String::new(),
            shape,
            scale,
        };
        added.label = added.default_label();
        self.items.push(added);
        true
    }

    /// Remove an annotation. False when there is none with that number.
    pub(crate) fn remove(&mut self, id: u64) -> bool {
        if !self.items.iter().any(|item| item.id == id) {
            return false;
        }
        self.step();
        self.items.retain(|item| item.id != id);
        true
    }

    /// Give an annotation a name: at most [`MAX_LABEL`] characters, with no space at either
    /// end. An empty name gives it back its own. False when there is none with that number.
    pub(crate) fn rename(&mut self, id: u64, label: &str) -> bool {
        let Some(at) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        self.step();
        if let Some(item) = self.items.get_mut(at) {
            let label: String = label.trim().chars().take(MAX_LABEL).collect();
            item.label = if label.is_empty() {
                item.default_label()
            } else {
                label
            };
        }
        true
    }

    /// Remove every annotation. False when there were none.
    pub(crate) fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.step();
        self.items.clear();
        true
    }

    /// Put the list back as it was before the last change. False when there is none.
    pub(crate) fn undo(&mut self) -> bool {
        let Some(before) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.items, before));
        true
    }

    /// Do again the change last undone. False when there is none.
    pub(crate) fn redo(&mut self) -> bool {
        let Some(after) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.items, after));
        true
    }

    /// Remember the list as it is, before a change: one more step that can be undone, the
    /// oldest step going when there are more than [`MAX_HISTORY`]. What had been undone can
    /// no longer be done again.
    fn step(&mut self) {
        self.undo.push(self.items.clone());
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

/// What an annotation says: its kind, its measurement, and what else there is to know of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    /// "Ruler", "Angle", "Region" or "Marker".
    pub kind: &'static str,
    /// The measurement in words, with its unit.
    pub value: String,
    /// What qualifies it, or nothing.
    pub detail: String,
    /// Whose millimetres it is measured in.
    pub scale: Scale,
}

impl Measured {
    /// The measurement as the list and the picture show it: its value, and after it, for one
    /// made from the file's pixel spacing alone, that it was.
    pub fn shown(&self) -> String {
        format!("{}{}", self.value, self.scale.said_after())
    }
}

/// What an annotation measures, in the millimetres of `scale`. `placement` is where the
/// series' voxels lie, which is what says how exactly a length can be known; without it
/// nothing is said of that.
pub fn measured(
    shape: &Shape,
    placement: Option<&Affine<Voxel, Patient>>,
    scale: Scale,
) -> Measured {
    let (value, detail) = match shape {
        Shape::Marker { at } if scale == Scale::PixelSpacing => in_the_picture(*at),
        Shape::Ruler { a, b } => {
            let along = [0, 1, 2].map(|part| part_of(*b, part) - part_of(*a, part));
            let pitch = placement.and_then(|placement| pitch_along(placement, along));
            let known_to = pitch.map(|pitch| {
                format!(
                    "Its ends can be placed to about one voxel: ± {pitch:.1} mm along this line."
                )
            });
            let length = distance_mm(point(*a), point(*b));
            (format!("{length:.1} mm"), known_to.unwrap_or_default())
        }
        Shape::Angle { a, vertex, b } => {
            let angle = angle_degrees(point(*a), point(*vertex), point(*b));
            let said = angle.map_or("no angle".to_owned(), |degrees| format!("{degrees:.1}°"));
            (said, String::new())
        }
        Shape::Region {
            radii_mm, stats, ..
        } => of_a_region(*radii_mm, stats.as_ref()),
        Shape::Marker { at } => (place_in_words(*at), String::new()),
    };
    Measured {
        kind: shape.kind(),
        value,
        detail,
        scale,
    }
}

/// Where a point of a picture placed by its pixel spacing alone is, as "12.0 mm across, 4.5 mm
/// down, slice 3", and from where that is measured. The picture is taken as its file holds it,
/// before it is turned or mirrored on the screen.
fn in_the_picture([x, y, z]: [f64; 3]) -> (String, String) {
    let place = format!("{x:.1} mm across, {y:.1} mm down, slice {:.0}", z + 1.0);
    let from = "From the middle of the picture's first pixel, as its file holds it.";
    (place, from.to_owned())
}

/// What a region says: the mean and spread of the scan values in it, then the rest; or that
/// it lies outside the scan.
fn of_a_region(radii_mm: [f64; 2], stats: Option<&RegionStats>) -> (String, String) {
    let [wide, high] = radii_mm;
    match stats {
        Some(stats) => (
            format!("mean {:.1}, SD {:.1}", stats.mean, stats.sd),
            format!(
                "min {:.0}, max {:.0}, area {}, {} samples",
                stats.min,
                stats.max,
                area_in_words(stats.area_mm2),
                stats.samples
            ),
        ),
        None => (
            "outside the scan".to_owned(),
            format!("area {}", area_in_words(std::f64::consts::PI * wide * high)),
        ),
    }
}

/// An area in square millimetres below 100 and square centimetres from there.
pub fn area_in_words(mm2: f64) -> String {
    if mm2 < 100.0 {
        format!("{mm2:.1} mm²")
    } else {
        format!("{:.1} cm²", mm2 / 100.0)
    }
}

/// Where a point is from the scanner's origin, as "L 12.0, P 4.5, S 30.2 mm": to the
/// patient's left or right, front (anterior) or back (posterior), head (superior) or feet.
fn place_in_words([x, y, z]: [f64; 3]) -> String {
    let side = |at: f64, towards: char, away: char| {
        let letter = if at < 0.0 { away } else { towards };
        format!("{letter} {:.1}", at.abs())
    };
    format!(
        "{}, {}, {} mm",
        side(x, 'L', 'R'),
        side(y, 'P', 'A'),
        side(z, 'S', 'I')
    )
}

fn point([x, y, z]: [f64; 3]) -> Point<Patient> {
    Point::new(x, y, z)
}

fn part_of([x, y, z]: [f64; 3], part: usize) -> f64 {
    match part {
        0 => x,
        1 => y,
        _ => z,
    }
}

mod awaited;
mod kept;
mod left;

#[cfg(test)]
mod tests;
