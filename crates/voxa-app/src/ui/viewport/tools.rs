//! Measuring on a slice: where the pointer is in millimetres, what each press, move and release
//! makes with the tool in hand, and the annotations of the series drawn where they lie on the
//! slice shown. An annotation is begun and finished in the active viewport, on one slice of one
//! series: a press on another viewport only makes it the active one, and what was begun is
//! given up as soon as its viewport shows another slice or another series.

use egui::{PointerButton, Pos2, Rect, Response, Ui};
use voxa_core::{Affine, Patient, Point, Screen, SeriesId, Voxel};
use voxa_volume::Plane;

use super::super::pen::Pen;
use super::super::planes::{
    Draft, Pointer, beside, drafted, drawn_through, paint, preview, shown_on,
};
use super::super::theme;
use super::value::screen_to_image;
use super::{Pane, ReadSlice, narrowed, pixel_aspect};
use crate::action::Action;
use crate::annotations::{Shape, Tool, measured};
use crate::measuring::{
    SlicePlacement, frame_ellipse_stats, half_slice, placement_of, slice_plane,
};
use crate::state::{AppState, VIEWPORTS};

/// How far below the top of a viewport the words beside an annotation begin at the least, in
/// points: clear of the letter in the middle of the top edge.
const TOP: f32 = 26.0;

/// How the slice a viewport shows lies this frame: on the screen, and in millimetres.
pub(super) struct OnSlice<'a> {
    /// The series shown, and which of its slices.
    series: &'a SeriesId,
    slice: usize,
    /// Where its pixels lie, and in whose millimetres.
    at: SlicePlacement,
    /// From points of the viewport, counted from its top left corner, to the slice's columns
    /// and rows; and back.
    from_screen: Affine<Screen, Voxel>,
    to_screen: Affine<Voxel, Screen>,
    /// From millimetres to the series' columns, rows and slices.
    from_mm: Affine<Patient, Voxel>,
    /// How many columns and rows the slice has.
    size: [u32; 2],
    /// The plane the slice lies in.
    plane: Plane,
}

impl<'a> OnSlice<'a> {
    /// The slice a viewport shows, if its files say where its pixels lie in millimetres.
    pub(super) fn of(pane: &Pane<'a>) -> Option<Self> {
        let viewport = pane.state.viewports.get(pane.at)?;
        let series = viewport.series.as_ref()?;
        let info = &pane.state.series(series)?.info;
        let at = placement_of(info)?;
        let size = [info.cols, info.rows];
        let cell = [pane.cell.width(), pane.cell.height()];
        let aspect = pixel_aspect(pane.state, pane.at);
        Some(Self {
            series,
            slice: viewport.slice,
            at,
            from_screen: screen_to_image(&pane.shown, cell, aspect, size)?,
            to_screen: pane.shown.image_to_screen(size, cell, aspect),
            from_mm: at.placement.inverse()?,
            size,
            plane: slice_plane(&at, viewport.slice, size),
        })
    }

    /// The point of the slice under a place of the viewport, in points from its top left
    /// corner, and whether that place is on the picture.
    fn under(&self, [x, y]: [f32; 2]) -> (Point<Patient>, bool) {
        let pixel = self
            .from_screen
            .apply(Point::new(f64::from(x), f64::from(y), 0.0));
        let [cols, rows] = self.size.map(f64::from);
        let inside = |at: f64, count: f64| (-0.5..=count - 0.5).contains(&at);
        let on = inside(pixel.x, cols) && inside(pixel.y, rows);
        let slice = self.slice as f64;
        (
            self.at.placement.apply(Point::new(pixel.x, pixel.y, slice)),
            on,
        )
    }

    /// Where a point falls in the viewport, in points from its top left corner.
    fn seen(&self, [x, y, z]: [f64; 3]) -> [f32; 2] {
        let pixel = self.from_mm.apply(Point::new(x, y, z));
        let shown = self.to_screen.apply(Point::new(pixel.x, pixel.y, 0.0));
        [narrowed(shown.x), narrowed(shown.y)]
    }
}

/// What each viewport remembers of an annotation being drawn on its slice.
#[derive(Debug, Default)]
pub struct Drafts([Drafting; VIEWPORTS]);

/// What one viewport remembers of an annotation being drawn: the draft, where the pointer
/// last was over the slice, and the tool, series and slice it is drawn with.
#[derive(Debug, Default)]
struct Drafting {
    draft: Draft,
    pointer: Option<Point<Patient>>,
    with: Tool,
    on: Option<(SeriesId, usize)>,
}

impl Drafting {
    /// What a press, move or release makes of the draft.
    fn take(&mut self, pointer: Pointer, plane: &Plane) -> Option<Shape> {
        let (draft, shape) = drafted(self.with, self.draft, pointer, plane);
        self.draft = draft;
        shape
    }
}

impl Drafts {
    /// Give up what a viewport has begun when the tool in hand, the series it shows or its
    /// slice has changed, when it is no longer the active viewport, or when `cancel` is true:
    /// the person pressed Escape. Nothing begun on one slice is finished on another.
    pub fn keep_up(&mut self, state: &AppState, cancel: bool) {
        for (at, drafting) in self.0.iter_mut().enumerate() {
            let viewport = state.viewports.get(at).filter(|_| at == state.active);
            let shown = viewport.and_then(|shown| Some((shown.series.as_ref()?, shown.slice)));
            let same = drafting.on.as_ref().map(|(series, slice)| (series, *slice)) == shown;
            if cancel || drafting.with != state.tool || !same {
                *drafting = Drafting {
                    with: state.tool,
                    on: shown.map(|(series, slice)| (series.clone(), slice)),
                    ..Drafting::default()
                };
            }
        }
    }

    /// Take in what the main button and the pointer did over viewport `at`'s slice this
    /// frame, and give what adds the annotation it finished, if it finished one: a region with
    /// what the slice holds inside it, read by `read` once, as it is finished. A press off the
    /// picture begins nothing.
    pub(super) fn step(
        &mut self,
        at: usize,
        on: &OnSlice<'_>,
        input: &SliceInput,
        read: &ReadSlice<'_>,
    ) -> Option<Action> {
        let drafting = self.0.get_mut(at)?;
        let mut made = None;
        if let Some((point, true)) = input.pressed.map(|place| on.under(place)) {
            made = drafting.take(Pointer::Pressed(point), &on.plane);
        }
        if let Some((point, _)) = input.hovered.map(|place| on.under(place)) {
            drafting.pointer = Some(point);
            made = made.or(drafting.take(Pointer::Moved(point), &on.plane));
        }
        if let Some((point, _)) = input.released.map(|place| on.under(place)) {
            made = made.or(drafting.take(Pointer::Released(point), &on.plane));
        }
        made.map(|shape| Action::AnnotationAdd(measured_on(shape, on, read)))
    }

    /// What viewport `at` is drawing, as it would be if finished where the pointer is now.
    pub(super) fn so_far(&self, at: usize, on: &OnSlice<'_>) -> Option<Shape> {
        let drafting = self.0.get(at)?;
        preview(drafting.draft, drafting.pointer?, &on.plane)
    }
}

/// A region with what the slice holds inside it worked out, from the slice's own values read
/// where they lie; any other annotation as it is.
fn measured_on(shape: Shape, on: &OnSlice<'_>, read: &ReadSlice<'_>) -> Shape {
    let Shape::Region {
        centre,
        across,
        down,
        radii_mm,
        stats: None,
    } = shape
    else {
        return shape;
    };
    let mut stats = None;
    read(on.series, on.slice, &mut |values| {
        stats = frame_ellipse_stats(values, &on.at, on.slice, centre, radii_mm, across, down);
    });
    Shape::Region {
        centre,
        across,
        down,
        radii_mm,
        stats,
    }
}

/// What the main button and the pointer did over a viewport this frame, in points from its
/// top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct SliceInput {
    /// Where the main button went down on the viewport.
    pub pressed: Option<[f32; 2]>,
    /// Where the pointer is, while it is over the viewport or dragging from it.
    pub hovered: Option<[f32; 2]>,
    /// Where the main button came up, if it had gone down on the viewport or the pointer is
    /// over it.
    pub released: Option<[f32; 2]>,
}

/// What the main button and the pointer did over the viewport at `cell` this frame.
fn gather(ui: &Ui, response: &Response, cell: Rect) -> SliceInput {
    let in_cell = |at: Pos2| [at.x - cell.left(), at.y - cell.top()];
    let (went_down, came_up, latest) = ui.input(|i| {
        let main = PointerButton::Primary;
        let pointer = &i.pointer;
        let (down, up) = (pointer.button_pressed(main), pointer.button_released(main));
        (down, up, pointer.latest_pos())
    });
    let over = response.contains_pointer();
    let down_here = went_down && response.is_pointer_button_down_on();
    let up_here = came_up && (over || response.drag_stopped());
    SliceInput {
        pressed: down_here
            .then(|| response.interact_pointer_pos())
            .flatten()
            .map(in_cell),
        hovered: latest.filter(|_| over || response.dragged()).map(in_cell),
        released: latest.filter(|_| up_here).map(in_cell),
    }
}

/// Measure on a viewport's slice: with a tool in hand, in the active viewport, turn what the
/// mouse did into what adds the annotation it finished, if it finished one; and draw over the
/// slice the annotations of the scene's series that lie on it, and what is being drawn.
pub(super) fn measure(
    ui: &Ui,
    pane: &Pane<'_>,
    response: &Response,
    (drafts, read): (&mut Drafts, &ReadSlice<'_>),
) -> Option<Action> {
    let on = OnSlice::of(pane)?;
    let state = pane.state;
    let drawing = pane.at == state.active && state.tool != Tool::Navigate;
    let made = drawing
        .then(|| {
            let input = gather(ui, response, pane.cell);
            drafts.step(pane.at, &on, &input, read)
        })
        .flatten();
    draw(ui, pane, &on, drafts.so_far(pane.at, &on));
    made
}

/// Draw over the slice the annotations of the scene's series that lie on it, each with its
/// name and what it measures, and then the one being drawn, with what it would measure if it
/// were finished now.
fn draw(ui: &Ui, pane: &Pane<'_>, on: &OnSlice<'_>, so_far: Option<Shape>) {
    let state = pane.state;
    let pen = Pen::over(ui, pane.cell);
    let area = (pane.cell, TOP);
    let placement = Some(&on.at.placement);
    // The list is of the scene's series, in its millimetres: another series has its own.
    let ours = state.scene.series.as_ref() == Some(on.series);
    let half = half_slice(&on.at, &on.plane);
    let items = state.annotations.items.iter().filter(|_| ours);
    let here = |shape: &Shape| shown_on(shape, &on.plane, half);
    for item in items.filter(|item| item.scale == on.at.scale && here(&item.shape)) {
        let said = measured(&item.shape, placement, item.scale).shown();
        let drawn = drawn_through(&item.shape, |at| on.seen(at));
        paint(&pen, area, &drawn, theme::ACCENT, &beside(item, &said));
    }
    if let Some(shape) = so_far {
        let said = measured(&shape, placement, on.at.scale).shown();
        let drawn = drawn_through(&shape, |at| on.seen(at));
        paint(&pen, area, &drawn, theme::WARNING, &said);
    }
}
