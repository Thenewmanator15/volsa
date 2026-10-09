# Voxa Milestone 3 (Plane views and measurement) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Beside the 3D view the person sees the same volume in axial, coronal and sagittal planes that all pass through one position they can move from any view, and can measure on it: distances, angles, region statistics and named markers, listed, undoable, kept between sessions and exportable.

**Architecture:** One point in the patient, the *position*, lives in `AppState`. Each plane view is the volume's own layer through that point, drawn by a small shader that samples the 3D texture the scene already holds, so scrolling costs nothing to decode. Plane and measurement arithmetic is in `voxa-volume`, screen mapping and the shader in `voxa-render`, and `voxa-app` only turns mouse input into `Action`s and draws what the state says. Annotations are points in patient millimetres belonging to a series; they change only through `reduce`, which is what gives undo and redo.

**Tech Stack:** As milestone 2. No new crates: CSV is written by hand (RFC 4180), JSON with `serde_json`, files chosen with `rfd`.

**Spec:** `docs/superpowers/specs/2026-10-05-voxa-design.md`: the milestone 3 row of A4, the "Slice planes in the scene" and "Measure and annotate" parts of A8, the reslicing lines of A5, "notes pinned to points" in A9. Sections A2, A3 and A6 bind every task.

**Branch:** `m3-plane-views`, cut from `perf-pass` (milestone 2 and the performance pass are not merged into `main`; merging is the owner's call).

## Scope

In: the position; three plane views with crosshairs, letters, scale bar and their own zoom and pan; the planes and the position drawn in the 3D scene, a click on a surface moving the position and a drag on a plane's handle scrolling it; the column beside the 3D view showing either the stack viewports or the planes, and a four-panel view when the planes are enlarged; ruler, angle and elliptical region statistics in the plane views; distance and markers in 3D; a measurements window with the list, renaming, deleting, undo and redo; annotations kept between sessions; CSV and JSON export; stack viewports of the same study following the position, with the position drawn on them; smooth (cubic B-spline) reslicing as a choice.

Out, with where it goes: turning a plane view by hand to any angle, thick-slab and curved planes (a later milestone 3 release; this plan's planes are oblique only when the acquisition is); moving an annotation's ends after it is made (same); measuring in a stack viewport, and so on a series with no 3D view (same); reference lines on locating scans, which need the geometry of each slice kept in the index (same); linking views across studies (6, needs registration); a cut-away to the three planes; anything needing labels (4).

## Conventions

The owner's instruction for milestone 2, to keep it standardised, is carried over as this plan's reading of what they want: where medical imaging has a convention Voxa follows it, elsewhere it does what milestones 1 and 2 do.

- **Orientation on screen.** Axial is seen from the feet and coronal from the front, so in both the patient's left is on the right of the screen while Radiological is on, and on the left when it is off. Sagittal has the front of the head on the left and is not changed by Radiological. Up is anterior in axial and superior in the other two. Edges are lettered by `overlay::letter`, the function milestone 1 uses.
- **Colours** are those of 3D Slicer, which most people who have seen plane views will know: axial red, sagittal yellow, coronal green. A plane's colour is used for its caption, its outline in 3D and its line in the other two planes.
- **Planes follow the volume's own axes**, each named by the anatomical plane it is nearest. The scan's own slices are then shown exactly as taken, and a tilted acquisition gives tilted planes with slanted letters, which is true.
- **Mouse in a plane view** is the 2D viewport's: wheel moves through layers, Ctrl with the wheel zooms about the pointer, middle drag pans, right drag changes the window, double-click fits. Left click or drag moves the position, or draws with the tool chosen.
- **Keys.** `Ctrl+Z` undo, `Ctrl+Y` and `Ctrl+Shift+Z` redo, `M` the measurements window, `Esc` puts the tool down before it does anything else.
- **Units and wording.** Millimetres to one decimal place, degrees to one, areas in mm² below 100 and cm² from there. Every measurement says how exactly it can be known.

Decisions this plan makes where the spec is silent:

- Reslicing is done on the graphics device with the blend of the eight nearest voxels, which is exact on the scan's own planes. The cubic B-spline of A5 is the "Smooth" choice (Task 13): it needs a second texture as large as the volume, so it is made only when asked for.
- Measuring happens where there is a volume: in the plane views and in 3D.
- A region is an ellipse. Its numbers are worked out once, when it is drawn, from the scan values and not from what the window shows.
- A ruler's uncertainty is one voxel along its own direction (`pitch_along`), said beside the number: across slices 7.5 mm apart a length is known to about 7.5 mm however fine the picture looks.
- Annotations belong to the series they were made on and are shown only on views of it. They are kept in `annotations.json` in the settings folder under the series key milestone 2 uses for saved views. The file and every export hold coordinates, numbers and the labels the person typed, and nothing else.
- Undo and redo cover annotations. The camera, the position and the window are not undone.
- Stack viewports are linked only to series of the same study, on the assumption, said in the interface, that the patient did not move between them.

## Global Constraints

Every constraint of the milestone 1 and 2 plans still holds: the disclaimer verbatim; dual MIT / Apache-2.0; no network, no telemetry; patient data never in the repository; nothing from a real export in a test, a document or a commit message; the repository stays local until the owner says so; no `unsafe` outside `voxa-render`; library crates never panic; `cargo xtask check` passes at every commit (format, clippy pedantic as errors, 400-line files, 60-line functions, tests, `cargo deny`, docs); every public item documented; coverage floor 80% of lines on `voxa-core`, `voxa-dicom`, `voxa-volume`; conventional commits, one per task; crates depend only downward; `Patient`, `Voxel` and `Screen` stay distinct types; `voxa-app` contains no image mathematics; tests that need a graphics device return early with a notice when there is none; a background task that panics must not stop the program; every sentence shown is plain words and says what the picture cannot show. In addition:

- A measurement is computed from patient millimetres and scan values, never from screen pixels or windowed greys.
- An exported file's suggested name is `voxa-measurements`, and neither it nor `annotations.json` contains a series identifier, description, date or patient detail.
- Colours, type sizes and spacing come from `theme.rs`; the three plane colours are added there once.

## Review Focus

Inputs the spec implies but does not spell out, most likely first. Each has its test in the named task.

1. **Which way round.** A plane view that is mirrored looks right and is wrong. Each view must put the patient's left, front and top where its letters say, with Radiological on and off, for a volume scanned in any of the three orientations and for a tilted one. Tasks 1, 3 and 5.
2. **Thick and tilted slices.** Planes are drawn in true proportions, one step of the wheel is one layer, and a length measured across thick slices is right in millimetres and says how coarse it is. Tasks 1, 2, 3 and 9.
3. **The position at the edge.** Clicking outside the volume, scrolling past the last layer, or a saved marker that lies outside the volume: the position stays inside, and what lies outside the volume is black, never a smear of the nearest voxels. Tasks 1, 3 and 4.
4. **No volume to work on.** A series with no 3D view, a volume still being built, another export opened half way through a measurement: the panels say why they are empty, a tool does nothing, and annotations of one series never show on another. Tasks 4, 5, 7 and 8.
5. **Files that are wrong.** An `annotations.json` that is damaged, half written or of a later version loads as no annotations and is not overwritten until the person next changes something; an export that cannot be written says so. Tasks 8 and 11.

---

### Task 1: Planes through a volume

**Files:** `crates/voxa-volume/src/plane.rs`, `crates/voxa-volume/src/lib.rs`

**Interfaces:**
- Consumes: `Volume`, `Affine`, `Point`, `Patient`, `Voxel`, `sample` (milestones 1 and 2).
- Produces:

```rust
pub enum PlaneKind { Axial, Coronal, Sagittal }           // Copy, Eq, Hash; PlaneKind::ALL in that order; name() -> "Axial" ...
pub struct Plane { pub centre: Point<Patient>, pub across: [f64; 3], pub down: [f64; 3] }   // across and down of length 1, square to each other
impl Plane {
    pub fn normal(&self) -> [f64; 3];                              // across x down
    pub fn at(&self, mm: [f64; 2]) -> Point<Patient>;              // centre + mm[0] across + mm[1] down
    pub fn of(&self, point: Point<Patient>) -> ([f64; 2], f64);    // where on the plane, and how far along the normal
}
pub fn plane_of(placement: &Affine<Voxel, Patient>, dims: [u32; 3], kind: PlaneKind,
                through: Point<Patient>, radiological: bool) -> Option<Plane>;
pub fn pitch_along(placement: &Affine<Voxel, Patient>, direction: [f64; 3]) -> Option<f64>;  // mm between voxel layers met along a direction
pub fn clamped_into(placement: &Affine<Voxel, Patient>, dims: [u32; 3], point: Point<Patient>) -> Point<Patient>;
pub fn nearest_slice(placement: &Affine<Voxel, Patient>, slices: usize, point: Point<Patient>) -> usize;
pub fn reslice(volume: &Volume, plane: &Plane, size: [u32; 2], mm_per_px: f64) -> Vec<f32>;  // rows from the top; NaN outside the volume
```
`plane_of` assigns the volume's three axes to left-right, front-back and head-foot by whichever assignment is most nearly aligned in total, so the three kinds always use three different axes. The plane holds two of the axes (the second made square to the first if the grid is sheared), signed as Conventions says; its centre is the middle of the volume moved along the normal until the plane holds `through`. `None` when the placement cannot be turned round. `clamped_into` holds a point within the box of voxel centres widened by half a voxel.

- [ ] **Step 1: Write failing tests** on a grid of `[20, 30, 40]` voxels of 1 mm along the patient's x, y and z, first voxel centre at the origin. `plane_of(.., Axial, (5, 6, 7), true)` has across `[1, 0, 0]`, down `[0, 1, 0]`, centre `(9.5, 14.5, 7.0)`; Coronal has across `[1, 0, 0]`, down `[0, 0, -1]`, centre `(9.5, 6.0, 19.5)`; Sagittal has across `[0, 1, 0]`, down `[0, 0, -1]`, centre `(5.0, 14.5, 19.5)`. With `radiological` false the axial and coronal across are `[-1, 0, 0]` and the sagittal plane is unchanged. A grid scanned sagittally (columns along +y, rows along -z, slices along +x) gives the same three across and down vectors. A grid tilted 20° about x has an axial normal within 1e-12 of its slice axis and an across of `[1, 0, 0]`. A placement of all zeros gives `None`. `at` then `of` returns `([3.0, -2.0], 0.0)`; a point 4 mm along the normal gives 4. On voxels of `[0.5, 0.5, 3.0]`: `pitch_along` z is 3.0, x is 0.5, `[0.6, 0.0, 0.8]` is 0.5 / 0.6, a direction of no length is `None`. `clamped_into` takes `(-50, 6, 7)` to `(-0.5, 6, 7)` and leaves `(5, 6, 7)` alone. `nearest_slice` with 40 slices: `(5, 6, 7.4)` is 7, `(5, 6, 99)` is 39, `(5, 6, -3)` is 0. `reslice` of milestone 2's 21³ ball (100 within 3 voxels of `(10, 10, 10)`) on the axial plane through its middle, 21 x 21 at 1 mm: the middle value is 100, the one 7 across from it is 0; at 41 x 41 the first value is NaN. A `proptest`: for random points and kinds, `plane_of(..).of(through).1` is 0 within 1e-9.
- [ ] **Step 2:** Run `cargo test -p voxa-volume plane`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(volume): the three planes of a volume through a point`.

---

### Task 2: Measuring

**Files:** `crates/voxa-volume/src/measure.rs`, `crates/voxa-volume/src/lib.rs`

**Interfaces:**
- Consumes: `Plane`, `sample` (Task 1, milestone 2).
- Produces:

```rust
pub fn distance_mm(a: Point<Patient>, b: Point<Patient>) -> f64;
pub fn angle_degrees(a: Point<Patient>, vertex: Point<Patient>, b: Point<Patient>) -> Option<f64>;   // 0 to 180; None when an arm has no length
pub struct RegionStats { pub mean: f64, pub sd: f64, pub min: f32, pub max: f32, pub area_mm2: f64, pub samples: usize }  // Clone, PartialEq, serde
pub fn ellipse_stats(volume: &Volume, plane: &Plane, centre_mm: [f64; 2], radii_mm: [f64; 2]) -> Option<RegionStats>;
```
`ellipse_stats` samples the ellipse on a square grid half the volume's finest spacing apart, counting only samples that fall inside the volume. The area is π times the two radii, whatever fell outside. `None` when a radius is not above zero or no sample is inside the volume. `voxa-volume` gains `serde` (derive) as a dependency, already in the workspace.

- [ ] **Step 1: Write failing tests.** `distance_mm` from the origin to `(3, 4, 0)` is 5 and to `(2, 3, 6)` is 7. `angle_degrees` with arms along +x and +y is 90, along +x and -x is 180, both along +x is 0, within 1e-9; with an arm of no length it is `None`. On a 21³ volume all of value 7, an ellipse of radii `[4, 2]` at the middle of the axial plane has mean 7, sd 0, min and max 7 and area 8π within 1e-9. On the ball, radii `[2, 2]` at its middle gives mean 100 and sd 0; radii `[8, 8]` gives a mean between 10 and 20 and an sd above 30. Radii `[0, 2]` and an ellipse wholly outside the volume are `None`. An ellipse half outside has `samples` between 40% and 60% of the same ellipse wholly inside, and the same area.
- [ ] **Step 2:** Run `cargo test -p voxa-volume measure`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(volume): distances, angles and region statistics`.

---

### Task 3: Drawing a plane on the graphics device

**Files:** `crates/voxa-render/src/{plane_view,plane_pipeline}.rs`, `plane.wgsl`, `offscreen_scene.rs`, `offscreen_scene/plane_tests.rs`, `volume_pipeline.rs`, `lib.rs`

**Interfaces:**
- Consumes: `Plane`, `reslice` (Task 1); `VolumePipeline` and its volume on the device, `HeadlessScene`, the ball helpers of `testing.rs` (milestone 2).
- Produces:

```rust
pub struct PlaneView { pub plane: Plane, pub mm_per_px: f64, pub offset_mm: [f64; 2] }   // offset: where on the plane the middle of the panel is
impl PlaneView {
    pub fn showing(plane: Plane, corners: &[Point<Patient>; 8], viewport: [f32; 2], zoom: f64, offset_mm: [f64; 2]) -> Self;
        // zoom 1 shows the whole of the volume's box as it falls on the plane, with a margin of 4%
    pub fn screen_to_patient(&self, viewport: [f32; 2], at: [f32; 2]) -> Point<Patient>;
    pub fn patient_to_screen(&self, viewport: [f32; 2], point: Point<Patient>) -> ([f32; 2], f64);   // and mm along the normal
}
pub const MIN_PLANE_ZOOM: f64 = 0.5;  pub const MAX_PLANE_ZOOM: f64 = 40.0;
pub struct PlanePipeline { .. }
impl PlanePipeline {
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self;
    pub fn show(&mut self, device: &wgpu::Device, volume: &VolumePipeline);   // draw from the volume that pipeline holds, or nothing if it holds none
    pub fn set_params(&self, queue: &wgpu::Queue, view: &PlaneView, viewport: [f32; 2], window: WindowLevel);
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>);
}
impl HeadlessScene { pub fn render_plane(&mut self, view: &PlaneView, window: WindowLevel, size: [u32; 2]) -> Result<Vec<u8>, RenderError>; }
```
The shader draws one triangle over the panel; for each pixel it finds the patient point, then the texture coordinate as `volume.wgsl` does, and writes the windowed grey, or black where the point is outside the volume. One `PlanePipeline` per panel, as each viewport has its own `SlicePipeline`.

- [ ] **Step 1: Write failing tests.** `PlaneView::showing` on the 48 mm ball volume's axial plane in a 200 x 100 panel has `mm_per_px` 48 x 1.04 / 100; the middle of the panel is the plane's centre; `patient_to_screen` of `plane.at([10, 0])` is 10 / `mm_per_px` to the right of the middle with 0 along the normal; `screen_to_patient` undoes it within 1e-6; zoom 2 halves `mm_per_px`; an offset of `[5, 0]` puts `plane.at([5, 0])` in the middle. Pictures, 96 x 96, through the ball's window: the axial plane through the middle is grey 191 at the middle, 64 at 10 mm from it and 0 at 20 mm; it differs from `reslice` of the same plane, windowed the same way, by under 1 grey level a pixel. The axial plane through the bead's middle `(12, -14, 0)` has the bead (grey above 250) in the upper right quarter with Radiological on and the upper left with it off; the coronal plane through it has the bead right of the middle; the sagittal plane through it has the bead left of the middle. On `[48, 48, 12]` voxels of 1 x 1 x 4 mm the ball's outline in the coronal picture is as tall as it is wide within 2 pixels. A panel zoomed out to 0.5 is black outside the volume's box (grey 0 at its corners). With nothing uploaded the picture is black.
- [ ] **Step 2:** Run `cargo test -p voxa-render plane`. Expected: FAIL.
- [ ] **Step 3:** Implement. Read how `volume_pipeline.rs` builds its bind group before writing the new one; share the volume's texture view and sampler, do not upload the volume again.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): a plane through the volume, drawn from the scene's own texture`.

---

### Task 4: The position and the planes in the state

**Files:** `crates/voxa-app/src/{planes,action,state,scene}.rs`, `reducer.rs`, `reducer/{planes,planes_tests}.rs`

**Interfaces:**
- Consumes: `plane_of`, `pitch_along`, `clamped_into` (Task 1); `Scene`, `SceneStatus`, `MainPanel` (milestone 2).
- Produces:

```rust
pub struct PlanePanel { pub zoom: f64, pub offset_mm: [f64; 2] }                 // Default: zoom 1, no offset
pub enum ColumnShows { Slices, Planes }                                          // Default: Slices
pub struct Planes { pub position: Option<Point<Patient>>, pub panels: [PlanePanel; 3], pub column: ColumnShows,
                    pub linked: bool, pub smooth: bool }                         // AppState.planes; panels in the order of PlaneKind::ALL
pub fn plane(state: &AppState, kind: PlaneKind) -> Option<Plane>;      // of the scene's series, through the position, as Radiological has it
// MainPanel gains Planes; MainPanel::given turns Planes into Stack too when there is no 3D view.
// Action gains: SetPosition(Point<Patient>), PlaneScroll { kind: PlaneKind, steps: i32 }, PlaneZoom { kind, factor: f64, about_mm: [f64; 2] },
//               PlanePan { kind, by_mm: [f64; 2] }, PlaneFit(PlaneKind), SetColumn(ColumnShows)
```
The position is `None` until the scene's volume is ready and is then the middle of the volume. `SetPosition` holds the point inside the volume. `PlaneScroll` moves it along that plane's normal by whole layers (`pitch_along`). `PlaneZoom` keeps the point of the plane at `about_mm` still and holds zoom between `MIN_PLANE_ZOOM` and `MAX_PLANE_ZOOM`. Selecting another series or opening another path sets the position to `None` and the panels back to their defaults; `column`, `linked` and `smooth` are kept, as `main` is.

- [ ] **Step 1: Write failing tests.** With the testkit's whole-head series shown and `VolumeReady` taken in, the position is the mean of the eight corners. `SetPosition` 500 mm outside comes back on the face of the volume. `PlaneScroll { Axial, 1 }` moves the position along the slice axis by the series' slice spacing and leaves its place in the plane alone; scrolling 1000 layers stops at the last one; with no position it changes nothing. `PlaneZoom` by 2 about `[10, 0]` doubles zoom and leaves `plane.at([10, 0])` at the same place in the panel; zoom never leaves its bounds; a factor that is not a number changes nothing. `PlanePan` adds to the offset; `PlaneFit` restores zoom 1 and no offset. Selecting another series, and `OpenPath`, empty the position and reset the panels but keep `column`. `MainPanel::Planes.given(&Unavailable)` is `Stack`. `plane` is `None` while the scene is building.
- [ ] **Step 2:** Run `cargo test -p voxa-app planes`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): one position shared by three planes, in the state`.

---

### Task 5: The plane panels in the window

**Files:** `crates/voxa-app/src/ui/{planes,layout,theme,mod}.rs`, `ui/planes/{overlay,mouse,gpu,tests}.rs`, `ui/capture.rs`

**Interfaces:**
- Consumes: `PlaneView`, `PlanePipeline` (Task 3); `Planes` and its actions (Task 4); `overlay::letter`, `overlay::scale_bar`, `pen` (milestone 1 and 2).
- Produces:

```rust
// theme.rs: pub const AXIAL, SAGITTAL, CORONAL: Color32   (red, yellow, green)
pub fn plane_colour(kind: PlaneKind) -> Color32;
pub fn plane_caption(kind: PlaneKind, plane: &Plane, position: Point<Patient>) -> String;   // "Axial · S 12.5 mm": which side of the scanner's origin, and how far
pub fn plane_letters(plane: &Plane) -> EdgeLabels;
pub fn crosshair(view: &PlaneView, viewport: [f32; 2], position: Point<Patient>) -> [[[f32; 2]; 2]; 4];   // four arms, leaving 6 points clear round the position
pub fn interpolated_note(spacing_mm: [f64; 3], pitch_across: f64, pitch_down: f64) -> Option<String>;
pub fn plane_input_to_actions(kind: PlaneKind, active: usize, input: &PlaneInput) -> Vec<Action>;
pub fn quad(area: Rect) -> [Rect; 4];          // axial, sagittal over coronal, 3D: two by two with the viewports' gap
```
When the 3D scene has the large part, the strip at the top of the column has two tabs, "Slices" and "Planes", and its Enlarge button gives whichever is shown the large part. With the planes large, the large part is `quad` (the 3D scene in the fourth panel) and the column shows the slices. A panel with no volume says the scene's own `status_text`. A panel whose picture runs across layers more than 1.5 times as far apart as the finest voxels says "Interpolated between layers {n} mm apart: detail between them is not measured." `VOXA_CAPTURE_SHOW` gains `planes` and `quad`.

- [ ] **Step 1: Write failing tests.** `plane_caption` for the Task 1 grid's axial plane through `(5, 6, 7)` is "Axial · S 7.0 mm", through `(5, 6, -3.3)` "Axial · I 3.3 mm"; coronal through `(5, -6, 7)` is "Coronal · A 6.0 mm"; sagittal through `(5, 6, 7)` is "Sagittal · L 5.0 mm". `plane_letters` of the axial plane with Radiological on has R at the left, L at the right, A at the top, P at the bottom; off, L at the left; sagittal has A left, P right, S top, I bottom. `crosshair` has two arms on the row of the position and two on its column, none within 6 points of it, and none when the position is off the panel by more than its size. `interpolated_note` for spacing `[0.5, 0.5, 3.0]` with pitches `(0.5, 3.0)` names 3.0 mm, and with `(0.5, 0.5)` is `None`. `plane_input_to_actions`: a left press or drag gives `SetPosition`; one notch of the wheel `PlaneScroll` by 1, away from the person being towards the next layer as in the 2D viewport; Ctrl with the wheel `PlaneZoom`; a middle drag `PlanePan` in millimetres; a right drag `AdjustWindow` on the active viewport; a double click `PlaneFit`. `quad` of a 1000 x 800 area gives four panels of equal size with the gap between them. Clicking along the column's strip finds both tabs and Enlarge (the sweep helper of `layout.rs`), and Enlarge with Planes shown asks for `SetMain(MainPanel::Planes)`.
- [ ] **Step 2:** Run `cargo test -p voxa-app planes`. Expected: FAIL.
- [ ] **Step 3:** Implement. Then capture the window on the made-up study with `planes` and with `quad` at 1280 x 800 and at 900 x 600, and read the pictures: nothing overlaps, the letters agree with the browser's thumbnail of the same series, the three panels cross at the same point.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): axial, coronal and sagittal panels beside the 3D view`.

---

### Task 6: The planes and the position in the 3D scene

**Files:** `crates/voxa-render/src/handle.rs`, `crates/voxa-app/src/ui/scene/{handles,input}.rs`, `ui/scene.rs`

**Interfaces:**
- Consumes: `Camera`, `on_screen`, `drag_to_mm`, `pick`, `surface_threshold` (milestone 2); `Planes`, `SetPosition`, `PlaneScroll` (Task 4).
- Produces:

```rust
// voxa-render
pub fn box_section(corners: &[Point<Patient>; 8], point: Point<Patient>, normal: [f64; 3]) -> Vec<Point<Patient>>;
    // where a plane cuts the volume's box: three to six points in order round the cut, or none
pub fn position_cross(camera: &Camera, viewport: [f32; 2], position: Point<Patient>, axes: [[f64; 3]; 3], arm_mm: f64) -> [[[f32; 2]; 2]; 3];
```
With the planes shown (the column's Planes tab or the four-panel view), each plane is outlined in the scene in its colour with a handle at the middle of one edge, and the position is a small cross whose arms run along the three plane normals. A click on a surface that is not a drag (the pointer moves under 4 points) moves the position there; over empty space it does nothing. Dragging a plane's handle scrolls that plane by whole layers. `slice_outline` is rewritten on `box_section`.

- [ ] **Step 1: Write failing tests.** `box_section` of the box from `(0, 0, 0)` to `(2, 2, 2)` by the plane through `(1, 1, 1)` with normal z is four points, all with z 1, going round a square of area 4; by the normal `(1, 1, 1)` it is six points, each 1.414 from the next; by a plane through `(9, 9, 9)` it is empty; a normal of no length is empty. `position_cross` seen from the front draws the arm along x across the screen, the arm along z up it, and the arm along y as a point. The scene's input: a press and release 2 points apart over the ball gives `SetPosition` on the ball's surface (within half a voxel of radius 16); 6 points apart gives the orbit it gave before and no `SetPosition`; a click over empty space gives nothing. Dragging the axial plane's handle 3 layers' worth along its normal gives `PlaneScroll { Axial, 3 }`, and half a layer gives none until the rest is dragged.
- [ ] **Step 2:** Run `cargo test -p voxa-render handle` and `cargo test -p voxa-app scene`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): the planes and the position shown and moved in the 3D scene`.

---

### Task 7: Annotations, with undo and redo

**Files:** `crates/voxa-app/src/{annotations,action,state}.rs`, `reducer/{annotations,annotations_tests}.rs`

**Interfaces:**
- Consumes: `RegionStats`, `distance_mm`, `angle_degrees`, `pitch_along` (Tasks 1 and 2).
- Produces:

```rust
pub enum Tool { Navigate, Ruler, Angle, Region, Marker }                     // Default: Navigate
pub enum Shape { Ruler { a: [f64; 3], b: [f64; 3] }, Angle { a: [f64; 3], vertex: [f64; 3], b: [f64; 3] },
                 Region { centre: [f64; 3], across: [f64; 3], down: [f64; 3], radii_mm: [f64; 2], stats: Option<RegionStats> },
                 Marker { at: [f64; 3] } }                                    // patient millimetres; serde
pub struct Annotation { pub id: u64, pub label: String, pub shape: Shape }   // serde
pub struct Annotations { pub items: Vec<Annotation>, .. }                    // AppState.annotations, with AppState.tool
impl Annotations { pub fn can_undo(&self) -> bool; pub fn can_redo(&self) -> bool; }
pub const MAX_ANNOTATIONS: usize = 200;  pub const MAX_HISTORY: usize = 100;  pub const MAX_LABEL: usize = 60;
pub struct Measured { pub kind: &'static str, pub value: String, pub detail: String }
pub fn measured(shape: &Shape, placement: Option<&Affine<Voxel, Patient>>) -> Measured;
// Action gains: SetTool(Tool), AnnotationAdd(Shape), AnnotationRemove(u64), AnnotationRename { id: u64, label: String }, AnnotationsClear, Undo, Redo
```
An annotation added with no label is named by its kind and number ("Ruler 1", "Marker 2"). Every change to the list is one step of undo; a new change empties redo; history holds `MAX_HISTORY` steps and the list `MAX_ANNOTATIONS` items, beyond which adding posts a notice and adds nothing. The list, its history and the tool are emptied when the scene's series changes or another path is opened. `measured` gives: a ruler "23.4 mm" with the detail "Its ends can be placed to about one voxel: ± 0.9 mm along this line."; an angle "47.2°"; a region "mean 312.0, SD 41.5" with the detail "min 201, max 388, area 1.8 cm², 412 samples", or "outside the scan" without statistics; a marker "L 12.0, P 4.5, S 30.2 mm".

- [ ] **Step 1: Write failing tests.** Adding a ruler from the origin to `(3, 4, 0)` lists "Ruler 1" measuring "5.0 mm", and on voxels `[0.5, 0.5, 3.0]` its detail says ± 0.5 mm, a ruler along z says ± 3.0 mm, and with no placement the detail is empty. An angle of 90 measures "90.0°"; one with an arm of no length "no angle". Area wording: 78.5 mm² is "78.5 mm²", 250 mm² is "2.5 cm²". A marker at `(-12, 4.5, 30.2)` reads "R 12.0, P 4.5, S 30.2 mm". Add, add, `Undo` leaves the first; `Redo` brings the second back with the same id; `Undo` then a new add leaves nothing to redo; `Undo` and `Redo` with nothing to do change nothing and are not errors; 101 changes can be undone 100 times. `AnnotationRename` cuts the label at 60 characters and an empty label restores the default name; renaming or removing an id that is not there changes nothing and adds no step. The 201st annotation is refused with a notice. Selecting another series, and `OpenPath`, empty the list and the history and put the tool down. `SetTool` while the scene has no volume leaves the tool at `Navigate`.
- [ ] **Step 2:** Run `cargo test -p voxa-app annotations`. Expected: FAIL.
- [ ] **Step 3:** Implement. History is kept as earlier copies of the list: the lists are small.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): annotations with undo and redo`.

---

### Task 8: Keeping annotations between sessions

**Files:** `crates/voxa-app/src/{annotations,settings,action}.rs`, `worker/annotations.rs`, `worker.rs`, `reducer/annotations.rs`, `session/tests.rs`

**Interfaces:**
- Consumes: `Annotation` (Task 7); `series_key`, `settings::folder`, the load and store pattern of `worker/views.rs` (milestone 2).
- Produces:

```rust
pub const ANNOTATIONS_VERSION: u32 = 1;
pub fn load_annotations(folder: &Path, key: &str) -> Vec<Annotation>;        // a missing, damaged or newer-version file is an empty list
pub fn save_annotations(folder: &Path, key: &str, items: &[Annotation]) -> std::io::Result<()>;
// Action gains: AnnotationsLoaded { series: SeriesId, items: Vec<Annotation> }, AnnotationsNotStored
// Effect gains: LoadAnnotations { series: SeriesId }, StoreAnnotations { series: SeriesId, items: Vec<Annotation> }
```
The file is `annotations.json` beside `views.json`: `{ "version": 1, "series": { "<key>": [ .. ] } }`. Every change to the list emits `StoreAnnotations`; a scene started on a series emits `LoadAnnotations`; what is loaded becomes the list with no history. Writing keeps the other series in the file, and goes by way of a temporary file renamed into place, so a write cut short leaves the old file whole. A file that could not be read is left alone until the person next changes the list.

- [ ] **Step 1: Write failing tests.** Save then load in a temporary folder returns the same items, region statistics included; two series in one file do not disturb each other; a file of `not json`, one with `"version": 2`, and half a file all load as an empty list; a missing folder is created. Read as text, the file contains no series identifier, description or study date of the testkit series. Reducer: adding emits `StoreAnnotations` with the whole list; `Undo` emits it too; `AnnotationsLoaded` for another series is ignored; loaded items cannot be undone away. Session: add two annotations, open the same zip in a second session that keeps the same folder, and both are listed once its scene is ready; a folder that cannot be written posts one warning that annotations will not be kept, as saved views do.
- [ ] **Step 2:** Run `cargo test -p voxa-app annotations`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): annotations are kept between sessions`.

---

### Task 9: Measuring in the plane views

**Files:** `crates/voxa-app/src/ui/planes/{tools,shapes,tools_tests}.rs`, `ui/planes.rs`

**Interfaces:**
- Consumes: `Tool`, `Shape`, `AnnotationAdd`, `measured` (Task 7); `PlaneView`, `Plane` (Tasks 1 and 3); `ellipse_stats` (Task 2); `Session::volume`.
- Produces:

```rust
pub enum Draft { Idle, RulerFrom(Point<Patient>), AngleArm(Point<Patient>), AngleVertex(Point<Patient>, Point<Patient>), RegionFrom(Point<Patient>) }
pub enum Pointer { Pressed(Point<Patient>), Moved(Point<Patient>), Released(Point<Patient>), Cancelled }
pub fn drafted(tool: Tool, draft: Draft, pointer: Pointer, plane: &Plane) -> (Draft, Option<Shape>);
pub fn shown_on(shape: &Shape, plane: &Plane, half_layer_mm: f64) -> bool;   // every point of it within half a layer of the plane
pub enum Drawn { Line([[f32; 2]; 2]), Arms([[f32; 2]; 3]), Ellipse { centre: [f32; 2], radii: [f32; 2] }, Dot([f32; 2]) }
pub fn drawn(shape: &Shape, view: &PlaneView, viewport: [f32; 2]) -> Drawn;
```
A ruler is drawn by pressing at one end and releasing at the other; an angle by three clicks (an arm, the vertex, the other arm); a region by dragging from its middle to a corner of the box round it; a marker by one click. A ruler shorter than 1 mm and a region with a radius under 0.5 mm are nothing. While a draft is open the shape so far follows the pointer; `Esc`, a change of tool or of series cancels it. A region's statistics are worked out when it is finished, from the volume. Each annotation shown on a panel is drawn in the accent colour with its label and value beside it; one that is not on the panel's layer is not drawn there.

- [ ] **Step 1: Write failing tests.** `drafted` with `Ruler`: press at A gives `RulerFrom(A)` and no shape; release at B 5 mm away gives `Idle` and `Shape::Ruler { a: A, b: B }`; release 0.4 mm away gives `Idle` and nothing. With `Angle`: three presses give the shape at the third, in the order arm, vertex, arm. With `Region`: press at C, release 4 mm across and 2 mm down gives radii `[4, 2]` about C with the plane's across and down. With `Marker`: a press gives the marker at once. `Cancelled` gives `Idle` from every draft. With `Navigate` nothing is ever drafted. `shown_on` a ruler lying in the plane is true; with one end 2 mm off it and a half layer of 0.5 it is false, and of 3.25 true. `drawn` of a ruler from `plane.at([0, 0])` to `plane.at([10, 0])` is a line `10 / mm_per_px` long across the middle of the panel; of a region its radii in points; of a marker one dot.
- [ ] **Step 2:** Run `cargo test -p voxa-app tools`. Expected: FAIL.
- [ ] **Step 3:** Implement, and capture the window on the made-up study with a ruler, an angle, a region and a marker in place (a capture word `measured` that adds them through actions).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): ruler, angle, region and marker in the plane views`.

---

### Task 10: Measuring and markers in 3D

**Files:** `crates/voxa-app/src/ui/scene/{tools,input}.rs`, `ui/scene.rs`

**Interfaces:**
- Consumes: `pick`, `on_screen` (milestone 2); `Tool`, `Shape`, `Draft`, `drafted` (Tasks 7 and 9).
- Produces: `pub fn scene_drawn(shape: &Shape, camera: &Camera, viewport: [f32; 2]) -> Drawn;` and the scene's handling of `Tool::Ruler` and `Tool::Marker`.

With the ruler, two clicks on surfaces give the straight-line distance between them; with the marker, one click places it. A click over empty space places nothing and says so in a notice ("There is no surface there to measure from."). Angle and region are plane tools: chosen in 3D they do nothing and the scene says "Angles and regions are drawn in the plane views." Every annotation of the series is drawn in the scene where its points fall, in front of the picture whatever is between it and the eye, which the caption says ("Measurements are drawn in front of the picture, even where tissue would hide them."). Turning, panning and zooming work as before whatever the tool.

- [ ] **Step 1: Write failing tests.** With `Tool::Ruler`, a click on the ball's left edge and one on its right edge from the front give a ruler between 31 and 33 mm (the ball is 32 mm across); a click on empty space between them leaves the draft open and asks for the notice. With `Tool::Marker` one click gives a marker within half a voxel of the surface. With `Tool::Angle` a click gives nothing. A drag of 6 points orbits whatever the tool. `scene_drawn` of a ruler along x seen from the front is a line across the screen as long as `distance / mm_per_px`; seen from the side it is a line of no length.
- [ ] **Step 2:** Run `cargo test -p voxa-app scene`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): distances and markers on surfaces in the 3D scene`.

---

### Task 11: The measurements window and export

**Files:** `crates/voxa-app/src/{export,measurements}.rs`, `ui/{measure,toolbar,help,mod}.rs`, `ui/overlay.rs`

**Interfaces:**
- Consumes: `Annotations`, `measured`, the annotation actions (Task 7); `ExportError`, `write`, the notices of `ui/scene/export.rs` (milestone 2).
- Produces:

```rust
pub struct Row { pub kind: String, pub label: String, pub value: String, pub detail: String, pub points: Vec<[f64; 3]> }
pub fn rows(items: &[Annotation], placement: Option<&Affine<Voxel, Patient>>) -> Vec<Row>;
pub fn measurements_csv(rows: &[Row]) -> String;      // kind,label,value,detail,x1,y1,z1,x2,y2,z2,x3,y3,z3 ; RFC 4180 quoting; CRLF
pub fn measurements_json(rows: &[Row]) -> String;     // { "voxa": "measurements", "version": 1, "space": "DICOM patient coordinates (LPS), millimetres", "notice": <the disclaimer>, "items": [ .. ] }
pub const MEASUREMENTS_NAME: &str = "voxa-measurements";
```
A "Measure" button on the toolbar opens a window with: the five tools; Undo and Redo, greyed when there is nothing to do; the list, each row with its label (editable in place), value, detail in quiet type, and a delete button; "Save as CSV…", "Save as JSON…" and "Clear all", greyed when the list is empty. Clicking a row moves the position to the annotation's first point. With no volume the window says "Measuring needs a 3D view of the series." The keys of Conventions are added to `shortcut_action` and to the help window.

- [ ] **Step 1: Write failing tests.** `measurements_csv` of a ruler labelled `He said "wide", I think` and a marker: the first line is the header; the ruler's label is quoted with its quotes doubled; the marker's row has three coordinates and six empty fields; every line ends CRLF; a label starting with `=`, `+`, `-` or `@` is written with a leading apostrophe so a spreadsheet does not run it. `measurements_json` parses, has version 1, the space string, the disclaimer verbatim and two items with their points. Neither contains the testkit series' identifier or description. Clicking down the window with two annotations listed finds each tool, Undo, both delete buttons and the three buttons at the bottom, and with an empty list the three are greyed. `shortcut_action`: Ctrl+Z is `Undo`, Ctrl+Y and Ctrl+Shift+Z are `Redo`, `M` toggles the window, and `Esc` with a tool in hand is `SetTool(Navigate)` and nothing else.
- [ ] **Step 2:** Run `cargo test -p voxa-app measure`. Expected: FAIL.
- [ ] **Step 3:** Implement. Capture the window with the measurements window open at 1280 x 800 and 900 x 600 and read the pictures.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): a measurements window, with CSV and JSON export`.

---

### Task 12: Stack viewports that follow the position

**Files:** `crates/voxa-volume/src/plane.rs`, `crates/voxa-app/src/reducer/{planes,viewport}.rs`, `ui/viewport/annotate.rs`, `ui/planes.rs`, `action.rs`

**Interfaces:**
- Consumes: `nearest_slice`, `Plane::of` (Task 1); `Planes.linked` (Task 4); `Scroll`, `SetSlice` (milestone 1).
- Produces: `Action::SetLinked(bool)`; in `voxa-volume`, `pub fn position_on_slice(placement: &Affine<Voxel, Patient>, slice: usize, position: Point<Patient>) -> Option<([f64; 2], f64)>;` (column and row of the position on a slice, and how many slices away it is).

A "Link" toggle beside the column's tabs, on by default. While it is on, every stack viewport showing a series of the same study as the scene's, with geometry, goes to the slice nearest the position whenever the position moves, and scrolling such a viewport moves the position onto the slice it now shows, keeping its place across the slice. The position is drawn on each linked viewport as a cross in the accent colour when it is within half a slice of the slice shown. The toggle's hover text says "Linked views take it that the patient did not move between series."

- [ ] **Step 1: Write failing tests.** With two series of one study in two viewports and the first in the scene: `SetPosition` sends the second viewport to its nearest slice and emits `RequestFrames` for it; with `linked` off it does not move; a series of another study does not move; a series with no geometry does not move. `Scroll` by 2 in a linked viewport moves the position by two of that series' slices along its slice axis and leaves the other two coordinates on the slice alone. Scrolling does not bounce: the viewport that was scrolled ends on the slice it was scrolled to. `position_on_slice` for a position on slice 7 at column 5, row 6 is `([5, 6], 0)`; 1.4 slices off it reports 1.4; with a placement that cannot be turned round, `None`.
- [ ] **Step 2:** Run `cargo test -p voxa-app planes` and `viewport`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): stack viewports of the same study follow the position`.

---

### Task 13: Smooth reslicing

**Files:** `crates/voxa-volume/src/bspline.rs`, `crates/voxa-render/src/{prepared,plane_pipeline}.rs`, `plane.wgsl`, `crates/voxa-app/src/worker/volume.rs`, `volume_store.rs`, `ui/planes.rs`

**Interfaces:**
- Consumes: `Volume` (milestone 2); `PlanePipeline`, `HeadlessScene::render_plane` (Task 3); `Planes.smooth` (Task 4).
- Produces:

```rust
// voxa-volume: cubic B-spline interpolation, Thévenaz, Blu and Unser, IEEE TMI 2000, equations 4 to 6
pub fn coefficients(volume: &Volume) -> Vec<f32>;                      // the prefilter along each axis, pole sqrt(3) - 2, mirrored ends
pub fn sample_cubic(dims: [u32; 3], coefficients: &[f32], voxel: [f64; 3]) -> Option<f32>;
// voxa-render
pub struct Smoothed { .. }                                             // the coefficients as the device will hold them
impl Smoothed { pub fn of(volume: &Volume, like: &Prepared) -> Self; }  // halved as often as the volume was
impl PlanePipeline { pub fn smooth(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, with: Option<&Smoothed>); }
impl HeadlessScene { pub fn smooth_planes(&mut self, with: Option<&Smoothed>); }
// Action gains: SetSmooth(bool); Effect gains: BuildSmooth { series: SeriesId }
```
The shader reads the coefficient texture with eight blended reads (Sigg and Hadwiger, GPU Gems 2, 2005). A "Smooth" toggle beside Link, off by default; turning it on has a worker make the coefficients for the volume in use, and the panels go on drawing the plain way until they are ready. While it is on each panel says "Smoothed: a curve through the measured values, not more detail." The coefficients are dropped when it is turned off or the volume changes.

- [ ] **Step 1: Write failing tests.** `sample_cubic` at every voxel centre of a random 6³ volume equals the voxel within 1e-3. On a volume whose value is its slice number squared, half way between slices 4 and 5 the cubic gives 20.25 within 0.05 where `sample` gives 20.5; on one whose value is `2x + 3y - z` both give the same within 1e-3. Outside the volume it is `None`. A volume of one voxel each way gives that value. Pictures: on the slice-squared volume the sagittal picture smoothed is within 2 grey levels of the true value midway between slices, where the plain picture is at least 5 levels off; at voxel centres the two pictures agree within 2 levels; `smooth_planes(None)` brings the plain picture back exactly. App: `SetSmooth(true)` with a volume ready emits `BuildSmooth`; with none it only sets the flag, and the effect comes when the volume is ready; a newer volume leaves no coefficients of the old one in the store.
- [ ] **Step 2:** Run `cargo test -p voxa-volume bspline`, `-p voxa-render plane` and `-p voxa-app planes`. Expected: FAIL.
- [ ] **Step 3:** Implement. Add a `criterion` case for `coefficients` on 512 x 512 x 64.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat: smooth reslicing by cubic B-spline, as a choice`.

---

### Task 14: Acceptance and release 0.3.0

**Files:** `docs/acceptance-0.3.0.md`, `CHANGELOG.md`, `README.md`, `docs/architecture.md`, `docs/images/`, `crates/voxa-app/examples/check_planes.rs`, the workspace version

- [ ] **Step 1:** Run `cargo xtask check` and `cargo xtask coverage`. Expected: all pass, the three non-GPU crates at or above 80%. Fix anything that fails.
- [ ] **Step 2:** Write `check_planes`, which opens an export with no window, and for every series with a 3D view draws its three planes through the middle, plain and smooth, checks each picture is not blank and that a ruler across the widest part of what is drawn agrees between the axial and coronal pictures within one voxel, and times a picture of each. It prints counts and times and nothing from the files.
- [ ] **Step 3: Acceptance** on the made-up study, recorded in `docs/acceptance-0.3.0.md`, and on a real export locally, of which the record gives nothing and pictures stay out of the repository. Anything a person must try by hand (drawing with the mouse, the handles, the save dialogs, how it feels) is listed as not checked.
- [ ] **Step 4:** Update `CHANGELOG.md` with 0.3.0, `docs/architecture.md` with the position and the plane path, `README.md` with a picture of the made-up study in the four-panel view, and the workspace version to `0.3.0`.
- [ ] **Step 5:** Commit: `chore: acceptance record and 0.3.0 changelog`, then `git tag v0.3.0`. Do not push, and do not merge into `main`.

---

## Out of scope

Turning a plane by hand, slabs and curved planes; moving or reshaping an annotation after it is made; measuring in the stack viewports; reference lines on locating scans; linking across studies; a cut-away to the planes; labels and everything that needs them; registration; models.
