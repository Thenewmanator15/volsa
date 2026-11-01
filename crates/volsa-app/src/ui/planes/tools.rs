//! Drawing annotations with the mouse in a plane view: what each press, move and release
//! makes with the tool in hand.

use volsa_core::{Patient, Point, SeriesId};
use volsa_volume::{Plane, PlaneKind, distance_mm};

use super::mouse::PlaneInput;
use crate::annotations::{Shape, Tool};

/// The shortest a ruler can be, in millimetres: let go nearer than this to where it was
/// pressed, and it was a click, not a ruler.
const LEAST_RULER_MM: f64 = 1.0;
/// The least either half-width of a region can be, in millimetres.
const LEAST_RADIUS_MM: f64 = 0.5;

/// An annotation that has been begun and not finished.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Draft {
    /// Nothing has been begun.
    #[default]
    Idle,
    /// A ruler, from where the button went down.
    RulerFrom(Point<Patient>),
    /// An angle, with the end of its first arm placed.
    AngleArm(Point<Patient>),
    /// An angle, with the end of its first arm and its vertex placed.
    AngleVertex(Point<Patient>, Point<Patient>),
    /// A region, from its middle.
    RegionFrom(Point<Patient>),
}

/// What the pointer did, at a point of the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pointer {
    /// The main button went down here.
    Pressed(Point<Patient>),
    /// The pointer is here now.
    Moved(Point<Patient>),
    /// The main button came up here.
    Released(Point<Patient>),
}

/// What a press, move or release makes of a draft with the tool in hand: the draft as it
/// then is, and an annotation if one has just been finished. A ruler is pressed at one end
/// and let go at the other; an angle is three presses, an arm, the vertex and the other arm;
/// a region is pressed at its middle and let go at a corner of the box round it, its sides
/// along the plane's; a marker is one press. A region's statistics are left for whoever
/// holds the volume to fill in.
pub fn drafted(
    tool: Tool,
    draft: Draft,
    pointer: Pointer,
    plane: &Plane,
) -> (Draft, Option<Shape>) {
    match (tool, draft, pointer) {
        (Tool::Navigate, _, _) => (Draft::Idle, None),
        // Moving finishes nothing, and an angle's points are placed by presses alone.
        (_, draft, Pointer::Moved(_)) | (Tool::Angle, draft, Pointer::Released(_)) => (draft, None),
        (Tool::Ruler, _, Pointer::Pressed(at)) => (Draft::RulerFrom(at), None),
        (Tool::Ruler, Draft::RulerFrom(from), Pointer::Released(at)) => {
            let long_enough = distance_mm(from, at) >= LEAST_RULER_MM;
            (Draft::Idle, long_enough.then(|| ruler(from, at)))
        }
        (Tool::Angle, Draft::AngleArm(arm), Pointer::Pressed(at)) => {
            (Draft::AngleVertex(arm, at), None)
        }
        (Tool::Angle, Draft::AngleVertex(arm, vertex), Pointer::Pressed(at)) => {
            (Draft::Idle, Some(angle(arm, vertex, at)))
        }
        (Tool::Angle, _, Pointer::Pressed(at)) => (Draft::AngleArm(at), None),
        (Tool::Region, _, Pointer::Pressed(at)) => (Draft::RegionFrom(at), None),
        (Tool::Region, Draft::RegionFrom(middle), Pointer::Released(at)) => {
            (Draft::Idle, region(middle, at, plane))
        }
        (Tool::Marker, _, Pointer::Pressed(at)) => {
            (Draft::Idle, Some(Shape::Marker { at: xyz(at) }))
        }
        (_, _, Pointer::Released(_)) => (Draft::Idle, None),
    }
}

/// A draft as it would be if it were finished with the pointer where it is now, to be drawn
/// while it is made. `None` when nothing has been begun.
pub fn preview(draft: Draft, now: Point<Patient>, plane: &Plane) -> Option<Shape> {
    match draft {
        Draft::Idle => None,
        Draft::RulerFrom(from) | Draft::AngleArm(from) => Some(ruler(from, now)),
        Draft::AngleVertex(arm, vertex) => Some(angle(arm, vertex, now)),
        Draft::RegionFrom(middle) => region(middle, now, plane),
    }
}

fn ruler(a: Point<Patient>, b: Point<Patient>) -> Shape {
    Shape::Ruler {
        a: xyz(a),
        b: xyz(b),
    }
}

fn angle(a: Point<Patient>, vertex: Point<Patient>, b: Point<Patient>) -> Shape {
    Shape::Angle {
        a: xyz(a),
        vertex: xyz(vertex),
        b: xyz(b),
    }
}

/// The ellipse about `middle` that fits the box whose corner is at `corner`, its sides along
/// the plane's. `None` if either half-width is too small to be a region.
fn region(middle: Point<Patient>, corner: Point<Patient>, plane: &Plane) -> Option<Shape> {
    let ([from_across, from_down], _) = plane.of(middle);
    let ([to_across, to_down], _) = plane.of(corner);
    let radii_mm = [(to_across - from_across).abs(), (to_down - from_down).abs()];
    let large_enough = radii_mm.iter().all(|radius| *radius >= LEAST_RADIUS_MM);
    large_enough.then(|| Shape::Region {
        centre: xyz(middle),
        across: plane.across,
        down: plane.down,
        radii_mm,
        stats: None,
    })
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// What the plane panels remember of an annotation being drawn: the draft, the panel it was
/// begun in, where the pointer last was there, and the tool and series it belongs to.
#[derive(Debug, Default)]
pub struct Drawing {
    draft: Draft,
    on: Option<PlaneKind>,
    pointer: Option<Point<Patient>>,
    with: Tool,
    of: Option<SeriesId>,
}

impl Drawing {
    /// Give up what was begun if the tool or the series has changed, or if `cancel` is true:
    /// the person pressed Escape.
    pub fn keep_up(&mut self, tool: Tool, series: Option<&SeriesId>, cancel: bool) {
        if cancel || self.with != tool || self.of.as_ref() != series {
            *self = Self {
                with: tool,
                of: series.cloned(),
                ..Self::default()
            };
        }
    }

    /// Take in what the mouse did over one plane's panel this frame, and give the annotation
    /// it finished, if it finished one. What is begun in one panel is finished in that panel:
    /// its points lie in that plane.
    pub fn step(&mut self, kind: PlaneKind, input: &PlaneInput, plane: &Plane) -> Option<Shape> {
        let mut made = None;
        if let Some(at) = input.pressed_at {
            if self.on != Some(kind) {
                self.draft = Draft::Idle;
            }
            self.on = Some(kind);
            made = self.take(Pointer::Pressed(at), plane);
        }
        if self.on != Some(kind) {
            return made;
        }
        if let Some(at) = input.hover_at {
            self.pointer = Some(at);
            made = made.or(self.take(Pointer::Moved(at), plane));
        }
        if let Some(at) = input.released_at {
            made = made.or(self.take(Pointer::Released(at), plane));
        }
        made
    }

    fn take(&mut self, pointer: Pointer, plane: &Plane) -> Option<Shape> {
        let (draft, shape) = drafted(self.with, self.draft, pointer, plane);
        self.draft = draft;
        shape
    }

    /// What is being drawn in one plane's panel, as it would be if finished where the pointer
    /// is now.
    pub fn so_far(&self, kind: PlaneKind, plane: &Plane) -> Option<Shape> {
        let now = self.pointer.filter(|_| self.on == Some(kind))?;
        preview(self.draft, now, plane)
    }
}
