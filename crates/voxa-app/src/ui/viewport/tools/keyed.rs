//! The keyboard point of a viewport: a pixel of the slice that the arrows move and Enter places,
//! as the pointer is placed by the mouse. What Enter makes of the draft is what a press and a
//! release make, through the same `drafted` the mouse goes through.

use egui::{Color32, Rect, Stroke, Ui, vec2};
use voxa_core::{Patient, Point};

use super::super::by_keys::{Keyed, Step};
use super::{After, Drafts, OnSlice, measured_on};
use crate::action::Action;
use crate::annotations::{Shape, measured};
use crate::state::{AppState, Notice, NoticeKind};
use crate::ui::planes::{Draft, Pointer};
use crate::ui::theme;

/// Where the notice that says how a measurement made with the keys fared is kept, so that one
/// replaces the one before.
const NOTICE_KEY: &str = "keyboard measurement";
/// What is said of a ruler that was too short to keep.
const TOO_SHORT: &str = "That ruler is too short to keep: nothing was added.";
/// What is said of a region that was too small to keep.
const TOO_SMALL: &str = "That region is too small to keep: nothing was added.";
/// How far the arms of the cross reach from the point, in points.
const ARM: f32 = 9.0;

impl OnSlice<'_> {
    /// The pixel in the middle of the slice, as its column and row.
    fn middle(&self) -> [u32; 2] {
        self.size.map(|count| count / 2)
    }

    /// The pixel of the slice nearest a point in the patient, as its column and row: on the
    /// slice's edge if the point lies off it.
    fn nearest(&self, at: Point<Patient>) -> [u32; 2] {
        let pixel = self.from_mm.apply(at);
        [(pixel.x, self.size[0]), (pixel.y, self.size[1])].map(|(coord, count)| {
            let last = f64::from(count.saturating_sub(1));
            whole(coord.round().clamp(0.0, last))
        })
    }

    /// The middle of a pixel of the slice, in the patient.
    fn at_pixel(&self, [col, row]: [u32; 2]) -> Point<Patient> {
        let slice = self.slice as f64;
        let (col, row) = (f64::from(col), f64::from(row));
        self.at.placement.apply(Point::new(col, row, slice))
    }

    /// Where the middle of a pixel of the slice is in the viewport, in points from its top
    /// left corner.
    fn seen_pixel(&self, pixel: [u32; 2]) -> [f32; 2] {
        let at = self.at_pixel(pixel);
        self.seen([at.x, at.y, at.z])
    }

    /// The pixel `by` steps from `from` towards the way an arrow points on the screen, kept on
    /// the slice: whichever way the picture is turned or mirrored, the point goes the way the
    /// arrow points.
    fn moved(&self, from: [u32; 2], toward: [f32; 2], by: u32) -> [u32; 2] {
        let [dcol, drow] = self.direction(from, toward);
        let [cols, rows] = self.size;
        let [col, row] = from;
        [(col, dcol, cols), (row, drow, rows)].map(|(at, step, count)| {
            let moved = i64::from(at) + i64::from(by) * i64::from(step);
            let last = i64::from(count.saturating_sub(1));
            u32::try_from(moved.clamp(0, last)).unwrap_or(0)
        })
    }

    /// Which neighbouring pixel, as a step of columns and rows, lies the way an arrow points on
    /// the screen from the pixel `from`.
    fn direction(&self, from: [u32; 2], toward: [f32; 2]) -> [i32; 2] {
        let [col, row] = from;
        let [x, y] = self.seen_pixel(from);
        let [across, down] = toward;
        let along = |[dcol, drow]: [i32; 2]| {
            let neighbour = [
                col.saturating_add_signed(dcol),
                row.saturating_add_signed(drow),
            ];
            let [there_x, there_y] = self.seen_pixel(neighbour);
            (there_x - x) * across + (there_y - y) * down
        };
        let steps = [[1, 0], [-1, 0], [0, 1], [0, -1]];
        let best = steps.into_iter();
        best.max_by(|a, b| along(*a).total_cmp(&along(*b)))
            .unwrap_or([0, 0])
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "kept between 0 and the number of pixels first"
)]
fn whole(coord: f64) -> u32 {
    coord as u32
}

impl Drafts {
    /// Whether viewport `at` has an annotation begun and not finished.
    pub(in super::super) fn in_progress(&self, at: usize) -> bool {
        self.0
            .get(at)
            .is_some_and(|drafting| !matches!(drafting.draft, Draft::Idle))
    }

    /// What a screen reader is told of viewport `at`'s keyboard point, if it has anything to
    /// say yet.
    pub(in super::super) fn said(&self, at: usize) -> Option<&str> {
        self.0.get(at)?.said.as_deref()
    }

    /// Take in the keys pressed over viewport `at`'s slice this frame, in order: an arrow moves
    /// the point, Enter places it. `moved` says that the mouse moved over the viewport, and
    /// `last` is the number of the last annotation of the scene's series. Gives what adds the
    /// annotation a key finished, and the notice that says a ruler or region was too small.
    pub(in super::super) fn by_keys(
        &mut self,
        at: usize,
        on: &OnSlice<'_>,
        keyed: &Keyed,
        (moved, last): (bool, Option<u64>),
        read: &super::ReadSlice<'_>,
    ) -> Vec<Action> {
        let Some(drafting) = self.0.get_mut(at).filter(|_| keyed.in_use) else {
            return Vec::new();
        };
        if moved {
            drafting.point = None;
        }
        let mut made = Vec::new();
        for step in &keyed.steps {
            let now = drafting.point.unwrap_or_else(|| {
                let near = drafting.pointer.map(|pointer| on.nearest(pointer));
                near.unwrap_or_else(|| on.middle())
            });
            drafting.point = Some(match step {
                Step::Move { toward, by } => on.moved(now, *toward, *by),
                Step::Place => now,
            });
            if matches!(step, Step::Place) {
                made.extend(place(drafting, on, (read, last)));
            }
            if let Some(point) = drafting.point {
                drafting.pointer = Some(on.at_pixel(point));
                drafting.said = Some(spoken(drafting, on, point));
            }
        }
        // The point, not a resting mouse, is where the draft is drawn to.
        if let (Some(point), false) = (drafting.point, moved) {
            drafting.pointer = Some(on.at_pixel(point));
        }
        made
    }

    /// The notice that names the measurement the keys finished in the frame before, if it was
    /// added, with the words the Measure tab's list gives it; the viewport says them too.
    pub(in super::super) fn announced(
        &mut self,
        at: usize,
        on: &OnSlice<'_>,
        state: &AppState,
    ) -> Option<Action> {
        let drafting = self.0.get_mut(at)?;
        let After(before) = drafting.announce.take()?;
        let ours = state.scene.series.as_ref() == Some(on.series);
        let item = state.annotations.items.last().filter(|_| ours);
        let item = item.filter(|item| Some(item.id) != before)?;
        let shown = measured(&item.shape, Some(&on.at.placement), item.scale).shown();
        let words = format!("{}: {shown}", item.label);
        drafting.said = Some(words.clone());
        Some(told(words))
    }

    /// Draw the cross that marks the keyboard point of viewport `at` over its slice, in the
    /// accent colour on a dark edge so that it shows on any picture. `cell` is the viewport.
    pub(in super::super) fn paint_point(&self, ui: &Ui, cell: Rect, at: usize, on: &OnSlice<'_>) {
        let Some(drafting) = self.0.get(at) else {
            return;
        };
        let near = drafting.pointer.map(|pointer| on.nearest(pointer));
        let pixel = drafting.point.or(near).unwrap_or_else(|| on.middle());
        let [x, y] = on.seen_pixel(pixel);
        let middle = cell.min + vec2(x, y);
        let painter = ui.painter().with_clip_rect(cell);
        for stroke in [
            Stroke::new(4.0, Color32::BLACK),
            Stroke::new(2.0, theme::ACCENT),
        ] {
            painter.line_segment([middle - vec2(ARM, 0.0), middle + vec2(ARM, 0.0)], stroke);
            painter.line_segment([middle - vec2(0.0, ARM), middle + vec2(0.0, ARM)], stroke);
        }
    }
}

/// Enter at the keyboard point: a press, or a release when what is begun is a ruler or a
/// region, which a release finishes. Gives what adds the annotation it finished, or the notice
/// that it was too small to keep.
fn place(
    drafting: &mut super::Drafting,
    on: &OnSlice<'_>,
    (read, last): (&super::ReadSlice<'_>, Option<u64>),
) -> Vec<Action> {
    let Some(point) = drafting.point else {
        return Vec::new();
    };
    let at = on.at_pixel(point);
    let was = drafting.draft;
    let finishing = matches!(was, Draft::RulerFrom(_) | Draft::RegionFrom(_));
    let pointer = if finishing {
        Pointer::Released(at)
    } else {
        Pointer::Pressed(at)
    };
    match drafting.take(pointer, &on.plane) {
        Some(shape) => {
            drafting.announce = Some(After(last));
            vec![Action::AnnotationAdd(measured_on(shape, on, read))]
        }
        None if finishing => vec![told(too_small(was).to_owned())],
        None => Vec::new(),
    }
}

/// A notice that says how a measurement made with the keys fared.
fn told(text: String) -> Action {
    Action::Post(Notice {
        kind: NoticeKind::Info,
        text,
        details: Vec::new(),
        key: Some(NOTICE_KEY),
    })
}

/// What is said of the keyboard point once it has been moved or used: how much the draft
/// measures so far, if one is begun, and where the point is, counting columns and rows from 1.
fn spoken(drafting: &super::Drafting, on: &OnSlice<'_>, [col, row]: [u32; 2]) -> String {
    let so_far = super::preview(drafting.draft, on.at_pixel([col, row]), &on.plane);
    let (col, row) = (col + 1, row + 1);
    match so_far.and_then(|shape| draft_words(drafting.draft, &shape, on)) {
        Some(words) => format!("{words}; point at column {col}, row {row}"),
        None => format!("Point at column {col}, row {row}"),
    }
}

/// What a draft measures so far, in words.
fn draft_words(draft: Draft, shape: &Shape, on: &OnSlice<'_>) -> Option<String> {
    let said = measured(shape, Some(&on.at.placement), on.at.scale);
    match (draft, shape) {
        (Draft::RulerFrom(_), _) => Some(format!("{} from the start", said.value)),
        (Draft::AngleArm(_), _) => Some(format!("{} from the first point", said.value)),
        (Draft::AngleVertex(..), _) => Some(format!("angle {}", said.value)),
        (Draft::RegionFrom(_), Shape::Region { radii_mm, .. }) => Some(format!(
            "{:.1} by {:.1} mm",
            2.0 * radii_mm[0],
            2.0 * radii_mm[1]
        )),
        _ => None,
    }
}

/// What is said of the ruler or region that a draft would have been, too small to keep.
fn too_small(draft: Draft) -> &'static str {
    match draft {
        Draft::RulerFrom(_) => TOO_SHORT,
        _ => TOO_SMALL,
    }
}
