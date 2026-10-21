//! Everything that can happen to the application, and the work it can ask for.

use std::path::PathBuf;

use voxa_core::{Patient, Point, SeriesId, WindowLevel};
use voxa_dicom::Index;
use voxa_labels::{Made, Part, Verdict};
use voxa_render::{Look, NamedView, Overlay};
use voxa_volume::PlaneKind;

use crate::annotations::{Annotation, Shape, Tool};
pub use crate::effect::Effect;
use crate::labelling::LabellingAction;
use crate::labels::NamesFrom;
use crate::planes::ColumnShows;
use crate::scene::MainPanel;
use crate::state::{Layout, Notice, PresetWindows};
use crate::views::ViewsAction;

/// One thing that happened: something the person did, or something a worker finished.
/// [`reduce`](crate::reduce) is the only code that acts on these.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "labels are reported once for a study and every other action is small"
)]
pub enum Action {
    /// Open a folder, zip or file in place of whatever is open.
    OpenPath(PathBuf),
    /// The index of the source so far.
    IndexProgress(Index),
    /// The source could not be opened or holds no images; the message says which.
    IndexFailed(String),
    /// Indexing has finished.
    IndexDone,
    /// Show a series in a viewport.
    SelectSeries {
        /// Which viewport.
        viewport: usize,
        /// Which series.
        series: SeriesId,
    },
    /// Move through the slices by `delta`, stopping at either end.
    Scroll {
        /// Which viewport.
        viewport: usize,
        /// How many slices, forwards or backwards.
        delta: i32,
    },
    /// Show another volume of the series a viewport shows, when that series holds several.
    StepVolume {
        /// Which viewport.
        viewport: usize,
        /// How many volumes on; less than none goes back.
        by: i32,
    },
    /// Go to a slice, stopping at the last.
    SetSlice {
        /// Which viewport.
        viewport: usize,
        /// Which slice, counting from 0.
        slice: usize,
    },
    /// Magnify by `factor`, keeping still what lies under `about`.
    Zoom {
        /// Which viewport.
        viewport: usize,
        /// How many times larger; below 1 shrinks.
        factor: f32,
        /// The point to keep still, in screen pixels from the middle of the viewport.
        about: [f32; 2],
    },
    /// Shift the picture.
    Pan {
        /// Which viewport.
        viewport: usize,
        /// How far, in screen pixels.
        by: [f32; 2],
    },
    /// Mirror the picture as it is seen.
    Flip {
        /// Which viewport.
        viewport: usize,
        /// Left to right when true, otherwise top to bottom.
        horizontal: bool,
    },
    /// Turn the picture a quarter turn clockwise.
    Rotate {
        /// Which viewport.
        viewport: usize,
    },
    /// Show the whole picture again, undoing zoom and pan.
    Fit {
        /// Which viewport.
        viewport: usize,
    },
    /// Change the window by dragging.
    AdjustWindow {
        /// Which viewport.
        viewport: usize,
        /// Added to the centre.
        d_center: f32,
        /// Added to the width.
        d_width: f32,
    },
    /// Set the window outright.
    SetWindow {
        /// Which viewport.
        viewport: usize,
        /// The window wanted.
        wl: WindowLevel,
    },
    /// Use one of [`voxa_volume::PRESETS`] on the slice shown.
    ApplyPreset {
        /// Which viewport.
        viewport: usize,
        /// Which preset, by its place in the list.
        preset: usize,
    },
    /// Arrange the viewports.
    SetLayout(Layout),
    /// Make a viewport the one keyboard commands act on.
    SetActive(usize),
    /// Switch between radiological and neurological display.
    ToggleRadiological,
    /// Show or hide the patient's name and identifier.
    TogglePatient,
    /// Filter the tag browser.
    SetTagFilter(String),
    /// Start or pause film-like playback.
    CineToggle {
        /// Which viewport.
        viewport: usize,
    },
    /// Set the speed of playback and whether it loops.
    CineSet {
        /// Which viewport.
        viewport: usize,
        /// Slices each second.
        fps: f32,
        /// Whether to start again after the last slice.
        looping: bool,
    },
    /// Playback is due to show the next slice.
    CineTick {
        /// Which viewport.
        viewport: usize,
    },
    /// A slice has been decoded and is in the frame cache.
    FrameReady {
        /// The series it belongs to.
        series: SeriesId,
        /// Which slice.
        slice: usize,
        /// The window each preset gives for it.
        windows: PresetWindows,
    },
    /// A slice could not be decoded.
    FrameFailed {
        /// The series it belongs to.
        series: SeriesId,
        /// Which slice.
        slice: usize,
        /// Why, in plain words.
        reason: String,
    },
    /// Remove a notice, by its place in the list.
    DismissNotice(usize),
    /// Tell the person something the window itself has to say, such as how saving a file
    /// went. A notice with a key takes the place of the last with that key.
    Post(Notice),
    /// Give the 3D scene or the 2D viewports the large part of the window.
    SetMain(MainPanel),
    /// The scene's picture has a new size.
    SceneResized {
        /// Its width and height, in pixels.
        viewport: [f32; 2],
    },
    /// Turn the scene with the pointer.
    SceneOrbit {
        /// How far the pointer was dragged, in pixels across and down.
        by: [f32; 2],
    },
    /// Shift the scene with the pointer.
    ScenePan {
        /// How far the pointer was dragged, in pixels across and down.
        by: [f32; 2],
    },
    /// Magnify the scene by `factor`, keeping still what lies under `about`.
    SceneZoom {
        /// How many times larger; below 1 shrinks.
        factor: f32,
        /// The point to keep still, in pixels from the middle of the scene.
        about: [f32; 2],
    },
    /// See the whole volume from one side of the patient.
    SceneView(NamedView),
    /// Put this point of the patient in the middle of the scene.
    SceneCentre(Point<Patient>),
    /// Change how the volume is turned into a picture.
    SceneLook(Look),
    /// The volume is on the graphics device.
    SceneUploaded {
        /// How many times it had to be halved, each way, to fit.
        halved: u32,
    },
    /// Cut the volume open straight across the view, through the middle of the scene.
    CutAdd,
    /// Move a cut along its own direction.
    CutMove {
        /// Which cut, by its place in the list.
        cut: usize,
        /// How far, in millimetres; a positive distance removes more.
        by_mm: f64,
    },
    /// Turn a cut with the pointer.
    CutTurn {
        /// Which cut, by its place in the list.
        cut: usize,
        /// How far the pointer was dragged, in pixels across and down.
        by: [f32; 2],
    },
    /// Take a cut away.
    CutRemove {
        /// Which cut, by its place in the list.
        cut: usize,
    },
    /// Draw the slice shown in the 2D viewport inside the scene, or stop drawing it.
    ShowSliceInScene(bool),
    /// More slices of a volume have been decoded.
    VolumeProgress {
        /// The series the volume is built from.
        series: SeriesId,
        /// Slices decoded so far.
        done: usize,
        /// Slices in the series.
        of: usize,
    },
    /// A volume has been built and is in the volume store.
    VolumeReady {
        /// The series it was built from.
        series: SeriesId,
        /// Its eight outer corners in the patient.
        corners: [Point<Patient>; 8],
    },
    /// Something about the saved views of the scene's series.
    Views(ViewsAction),
    /// A volume could not be built.
    VolumeFailed {
        /// The series it was to be built from.
        series: SeriesId,
        /// Why, in plain words.
        reason: String,
    },
    /// Move the point the three planes pass through. It is held inside the volume.
    SetPosition(Point<Patient>),
    /// Move a plane through the volume by whole layers, the way the voxel numbers rise.
    PlaneScroll {
        /// Which plane.
        kind: PlaneKind,
        /// How many layers; less than none goes back.
        steps: i32,
    },
    /// Magnify a plane's panel, keeping still what lies under `about_mm`.
    PlaneZoom {
        /// Which plane.
        kind: PlaneKind,
        /// How many times larger; less than 1 makes it smaller.
        factor: f64,
        /// The point of the plane that stays put, in millimetres across and down from the
        /// plane's centre.
        about_mm: [f64; 2],
    },
    /// Move what is in the middle of a plane's panel.
    PlanePan {
        /// Which plane.
        kind: PlaneKind,
        /// How far the middle of the panel moves over the plane, in millimetres across and
        /// down.
        by_mm: [f64; 2],
    },
    /// Show the whole of a plane again, undoing zoom and pan.
    PlaneFit(PlaneKind),
    /// Choose what the column beside the 3D scene shows.
    SetColumn(ColumnShows),
    /// Link the stack viewports to the planes' position, or let them go.
    SetLinked(bool),
    /// Draw the plane views from a curve through the measured values, or plainly.
    SetSmooth(bool),
    /// Open a file of labels on the study in the scene.
    OpenLabels {
        /// The label volume.
        labels: PathBuf,
        /// The table that names its parts; `None` to look for one beside it.
        table: Option<PathBuf>,
    },
    /// A worker has read the labels asked for.
    LabelsRead {
        /// What they are of, as their table says.
        title: String,
        /// Their parts.
        parts: Vec<Part>,
        /// The labelling model's own check, if the table carries one.
        verdict: Option<Verdict>,
        /// The corners of their box in the patient.
        corners: [Point<Patient>; 8],
        /// Where the parts' names came from.
        names_from: NamesFrom,
        /// Who made them, when their table says; `None` for labels made elsewhere.
        made: Option<Made>,
        /// The run of labelling that made them; `None` for labels read from a file.
        run: Option<u64>,
    },
    /// The labels asked for could not be read; the sentence says why.
    LabelsFailed(String),
    /// Close the labels.
    CloseLabels,
    /// Show or hide one part.
    PartShow {
        /// Its place among the parts.
        part: usize,
        /// Whether it is to be shown.
        shown: bool,
    },
    /// Show or hide every part of a group.
    GroupShow {
        /// The group's name.
        group: String,
        /// Whether its parts are to be shown.
        shown: bool,
    },
    /// Show every part.
    PartsShowAll,
    /// Show one part and hide the rest.
    PartIsolate(usize),
    /// Make a part more or less solid.
    PartOpacity {
        /// Its place among the parts.
        part: usize,
        /// How solid, up to 1.
        opacity: f32,
    },
    /// Select a part, or none.
    PartSelect(Option<usize>),
    /// Choose how labels are drawn over the plane views.
    SetOverlay(Overlay),
    /// The curve through a series' volume has been worked out and is kept with the volume.
    /// Nothing in the state changes: the window finds it there at its next frame.
    SmoothReady {
        /// The series whose volume it runs through.
        series: SeriesId,
    },
    /// Choose what the main button does in the plane views and the 3D scene.
    SetTool(Tool),
    /// Add a measurement or a marker to the scene's series.
    AnnotationAdd(Shape),
    /// Remove the annotation with this number.
    AnnotationRemove(u64),
    /// Give an annotation a name; an empty one gives it back its own.
    AnnotationRename {
        /// Its number.
        id: u64,
        /// The name.
        label: String,
    },
    /// Remove every annotation of the scene's series.
    AnnotationsClear,
    /// Put the annotations back as they were before the last change to them.
    Undo,
    /// Do again the change last undone.
    Redo,
    /// The annotations kept for a series have been read.
    AnnotationsLoaded {
        /// The series they are of.
        series: SeriesId,
        /// The annotations, oldest first.
        items: Vec<Annotation>,
    },
    /// The annotations could not be written to where they are kept.
    AnnotationsNotStored {
        /// Why, as the system gave it.
        reason: String,
    },
    /// Something about labelling the scan with Voxa's own networks.
    Labelling(LabellingAction),
}
