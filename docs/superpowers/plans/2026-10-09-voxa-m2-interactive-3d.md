# Voxa Milestone 2 (Interactive 3D) Implementation Plan

> Volsa was earlier called Voxa. This record keeps the name it was written with.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The series being looked at appears as a 3D volume in the main area of the window, drawn by ray-casting on the graphics device, which the person can turn, move, zoom, cut open at any angle, save views of, and export as a picture or a rotating animation.

**Architecture:** Workers decode every slice of a series and assemble a `Volume` in patient space (millimetres, DICOM LPS). `voxa-render` uploads it once as a 3D texture and a fragment shader marches a ray through it for every pixel of an off-screen picture, which `egui` shows as an image; the same picture is what gets exported. The camera is orthographic and lives in patient space, so nothing is ever mirrored and a scale bar is true across the whole picture. All scene state (camera, look, cut planes, saved views) is in `AppState` and changes only through `reduce`.

**Tech Stack:** As milestone 1 (Rust 1.99.0, `eframe`/`egui`/`egui-wgpu` 0.36 with its re-exported wgpu 30, `dicom` 0.10, `rayon`, `image`, `criterion`, `proptest`), plus `half` (16-bit floats for the 3D texture), `serde` and `serde_json` (saved views), `directories` (where to keep them), and the `gif` feature of `image`. Confirm each new crate's licence with `cargo deny check` in the task that adds it; read the installed source of `egui-wgpu` and `wgpu` before writing against them, as in milestone 1.

**Spec:** `docs/superpowers/specs/2026-10-05-voxa-design.md`: the milestone 2 row of A4, all of A8 that needs no labels, the 3D lines of A5, A7 and A9. Sections A2, A3 and A6 bind every task.

## Scope

In: volume assembly; ray-cast rendering with three looks: Surface (a lit, shaded surface), VR (volume rendering, see-through) and MIP (maximum-intensity projection); orbit, pan, zoom, six named views and saved views; double-click a surface to centre on it; up to three cut planes whose cut face shows the scan; the slice shown in the 2D viewport drawn inside the scene; a lower-quality picture while the camera moves; snapshot and turntable export; the window as one large 3D viewport with the 2D viewports in a column beside it, either of which can be enlarged.

Out, with the milestone that owns it: hover names, selecting, hiding, fading and peeling structures (4, they need labels); plane views, measurements and notes pinned to points (3); perspective projection; more than one volume in a scene (5).

## Conventions

The owner's instruction for this milestone is to keep it standardised. Where medical imaging has an established convention Voxa follows it; everywhere else it does what milestone 1 already does. No task may invent a third way.

- **Coordinates and letters.** DICOM patient coordinates (LPS, millimetres). Edges are lettered L, R, A, P, S, I exactly as in the 2D viewport, by the same function.
- **Named views** are the six anatomical ones: Anterior, Posterior, Left, Right, Superior, Inferior. Their buttons carry the letter (A, P, L, R, S, I) and say in the hover text what that means in plain words ("From the front", and so on).
- **Looks** carry the names radiology uses: Surface, VR and MIP, each with a plain-words hover text.
- **Projection** is parallel, the convention for MIP and for anything that is measured.
- **Mouse.** An action that exists in the 2D viewport is done the same way in 3D: middle drag pans, right drag changes the window (across for width, down for centre), Ctrl with the wheel zooms about the pointer, a press makes the panel active. What is new follows the common 3D convention: left drag turns the scene, the wheel alone zooms.
- **Keys.** `F` fits in both. `H`, `V` and `R` act on the 2D viewport only: a 3D scene is never mirrored.
- **Window and presets.** The 3D view uses the window of the active viewport, so the numbers, the right drag and the Auto, Soft and Hard presets of milestone 1 act on both at once.
- **Files written.** PNG for a picture, GIF for an animation, JSON for settings. Each is an open, documented format any other program can read.
- **Wording, theme, notices and errors** are milestone 1's: plain sentences, the same colours and type sizes from `theme.rs`, failures as notices.
- **Process** is milestone 1's: tests first, one commit a task, `cargo xtask check` green at every commit, a ledger of every ruling, a review of the whole branch at the end.

Decisions this plan makes where the spec is silent:

- The camera is orthographic. A scale bar is then valid everywhere in the picture.
- What is solid and what is see-through follows the window: below the window's low end nothing is drawn. Changing contrast is therefore also how a person peels the outside away, with no new control to learn.
- The rotating "video" is an animated GIF. MP4 is the more usual video file, but its encoders carry patent and licence terms that an MIT / Apache program cannot simply bundle; a GIF is an open format every viewer and browser reads, and a grey picture loses nothing in its 256 levels.
- Saved views are kept in a small file in the user's settings folder, keyed by a hash of the series identifier. The file holds camera numbers, a look and cut planes; no name, date or description.
- Frame-rate target (A7 asks this milestone to set one): for a 512 × 512 × 64 volume in a 1280 × 800 viewport on integrated graphics, at least 30 pictures a second while the camera moves and a full-quality picture within 250 ms of it stopping.

## Global Constraints

Every constraint of the milestone 1 plan still holds: the disclaimer verbatim; dual MIT / Apache-2.0; no network, no telemetry; patient data never in the repository; the repository stays local until the owner says so; no `unsafe` outside `voxa-render`; library crates never panic; `cargo xtask check` passes at every commit (format, clippy pedantic as errors, 400-line files, 60-line functions, tests, `cargo deny`, docs); every public item documented; coverage floor 80% of lines on `voxa-core`, `voxa-dicom`, `voxa-volume`; conventional commits, one per task; crates depend only downward (core ← dicom, volume ← render ← app); `Patient`, `Voxel` and `Screen` stay distinct types. In addition:

- Nothing from a real export goes into a test, a document or a commit message: no file name, date, description or identifier. Made-up values only. (Milestone 1 had to rewrite history for this.)
- `voxa-app` contains no image mathematics. Ray, camera and volume arithmetic live in `voxa-volume` and `voxa-render`, where they are tested without a window.
- Tests that need a graphics device return early with a printed notice when `RenderError::NoAdapter` comes back, as in milestone 1.
- An exported picture or animation contains the rendered scene and nothing else: no patient name, identifier, date or description, and the suggested file name is `voxa-3d` with no series text in it.
- A background task that panics must not stop the program, and tag rows and patient details never enter an `Action` (both from milestone 1's review).
- Every sentence shown to the person is plain words, and says what the picture cannot show as well as what it can.

## Review Focus

Inputs the spec implies but does not spell out, most likely first. Each has its test in the named task.

1. **Handedness.** A 3D scene that is mirrored looks right and is wrong. From the front the patient's left must be on the right of the screen, from the back on the left, in every look. Tasks 3 and 4.
2. **Thick and tilted slices.** A series with slices 7.5 mm apart and pixels 0.7 mm across, or with a tilted stack, must be drawn in its true proportions, and the view must say that detail between thick slices is interpolated. Tasks 1, 4 and 9.
3. **A volume larger than the graphics device can hold.** It is shown at reduced resolution with a sentence saying so; it never fails or crashes. Tasks 2, 4 and 9.
4. **A series that cannot be a volume.** Locating images, uneven spacing, repeated positions, one slice: the 3D area says why in plain words and the 2D viewport carries on. Task 8.
5. **Things going wrong while a volume is being built.** A slice that will not decode, or another export opened half way through: a message, never a volume of one series shown under the name of another. Task 8.

---

### Task 1: `Volume` and its assembly from slices

**Files:** `crates/voxa-core/src/volume.rs`, `crates/voxa-core/src/lib.rs`, `crates/voxa-volume/src/assemble.rs`, `crates/voxa-volume/src/lib.rs`

**Interfaces:**
- Consumes: `Affine`, `Point`, `Patient`, `Voxel`, `SeriesInfo`, `SeriesProblem` (milestone 1).
- Produces:

```rust
// voxa-core
pub struct Volume { .. }      // values[(slice * rows + row) * cols + col], in the units decode_slice gives
impl Volume {
    pub fn new(dims: [u32; 3], values: Vec<f32>, voxel_to_patient: Affine<Voxel, Patient>) -> Option<Self>;
        // dims are columns, rows, slices. None when a dimension is 0, the length is not their product,
        // or the transform cannot be undone.
    pub fn dims(&self) -> [u32; 3];
    pub fn values(&self) -> &[f32];
    pub fn voxel_to_patient(&self) -> Affine<Voxel, Patient>;
    pub fn patient_to_voxel(&self) -> Affine<Patient, Voxel>;
    pub fn get(&self, col: u32, row: u32, slice: u32) -> Option<f32>;
    pub fn corners(&self) -> [Point<Patient>; 8];   // outer corners: voxel -0.5 and dims - 0.5 on each axis
    pub fn centre(&self) -> Point<Patient>;
    pub fn spacing_mm(&self) -> [f64; 3];           // lengths of the transform's three columns
}
// voxa-volume
pub enum VolumeError { TooFewSlices(usize), NoGeometry, Problems(Vec<SeriesProblem>), SizeMismatch { slice: usize } }
pub fn can_be_volume(info: &SeriesInfo) -> Result<(), VolumeError>;
pub fn assemble(info: &SeriesInfo, slices: &[&[f32]]) -> Result<Volume, VolumeError>;
pub fn value_range(volume: &Volume) -> Option<(f32, f32)>;   // smallest and largest finite value
```
`VolumeError` reads: `TooFewSlices(n)` "A 3D view needs at least two slices; this series has {n}."; `NoGeometry` "The files do not say where the slices are, so a 3D view cannot be built."; `Problems(_)` "This series cannot be shown in 3D."; `SizeMismatch { slice }` "Slice {slice + 1} is not the size the series says, so a 3D view cannot be built."

- [ ] **Step 1: Write failing tests.** `Volume::new([2, 2, 3], (0..12).map(f32::from).collect(), identity)`: `get(1, 0, 2) == Some(9.0)`, `get(2, 0, 0) == None`; `new` is `None` for `[0, 2, 3]`, for 11 values, and for a transform whose third column is all zero. With columns (0.5, 0, 0), (0, 0.5, 0), (0, 0, 7.5) and origin (-100, -130, 10): `spacing_mm() == [0.5, 0.5, 7.5]`, the first corner is (-100.25, -130.25, 6.25), and `patient_to_voxel` applied to `voxel_to_patient` of (1, 1, 2) gives (1, 1, 2). `assemble` on a three-slice 2 × 2 series built with the testkit's geometry gives dims `[2, 2, 3]`, `get(1, 0, 2)` equal to the second value of the third slice, and the series' own transform. `can_be_volume`: one slice → `TooFewSlices(1)`; no `voxel_to_patient` → `NoGeometry`; `problems == [UnevenSpacing]` → `Problems` holding it. A second slice of 3 values → `SizeMismatch { slice: 1 }`. `value_range` of `[1.0, NaN, -3.0, inf]` is `(-3.0, 1.0)` and of all-NaN is `None`. Each error's sentence is asserted word for word.
- [ ] **Step 2:** Run `cargo test -p voxa-core volume` and `cargo test -p voxa-volume assemble`. Expected: FAIL.
- [ ] **Step 3:** Implement. `Volume` keeps its fields private so its three invariants always hold.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(volume): a volume in patient space, assembled from a series' slices`.

---

### Task 2: Sampling, rays and reduction on the processor

**Files:** `crates/voxa-volume/src/{sample,ray,reduce}.rs`

**Interfaces:**
- Consumes: `Volume` (Task 1).
- Produces:

```rust
pub fn sample(volume: &Volume, at: Point<Patient>) -> Option<f32>;    // trilinear; None outside the box of voxel centres widened by half a voxel
pub struct Ray { pub origin: Point<Patient>, pub direction: [f64; 3] }   // direction of length 1
impl Ray { pub fn at(&self, mm: f64) -> Point<Patient>; }
pub fn enter_and_leave(volume: &Volume, ray: &Ray) -> Option<(f64, f64)>;  // mm along the ray; entry is 0 when it starts inside
pub fn first_hit(volume: &Volume, ray: &Ray, threshold: f32) -> Option<Point<Patient>>;
    // steps half the smallest spacing, then bisects eight times
pub fn halved(volume: &Volume) -> Volume;        // mean of each 2 x 2 x 2 block; an odd edge averages what is there
pub fn halvings_needed(dims: [u32; 3], max_side: u32, max_bytes: u64) -> u32;   // at two bytes a voxel; stops at 1 x 1 x 1
pub fn block_maxima(volume: &Volume, block: u32) -> (Vec<f32>, [u32; 3]);     // largest value in each block, for skipping empty space
```

- [ ] **Step 1: Write failing tests** on a 21³ volume of zeros with 1 mm voxels, its first voxel centre at the patient origin, holding a ball of value 100 at every voxel within 3 of (10, 10, 10). `sample` at (10, 10, 10) is 100; at (6.5, 10, 10) is 50; at (-0.6, 10, 10) is `None`. `enter_and_leave` for a ray from (-5, 10, 10) along +x is `(4.5, 25.5)`; from (10, 10, 10) along +x is `(0.0, 10.5)`; for a ray along +x from (-5, 40, 10) is `None`. `first_hit` with threshold 50 along the first ray has x within 0.05 of 6.5; along a ray from (-5, 2, 2) it is `None`. `halved` of a 4³ volume whose value is its column number has dims `[2, 2, 2]`, values 0.5 and 2.5, and the same eight corners; of a 5 × 4 × 4 volume has dims `[3, 2, 2]`. `halvings_needed([512, 512, 300], 256, u64::MAX) == 1`; `([512, 512, 64], 2048, 16 << 20) == 1`; `([64, 64, 64], 2048, u64::MAX) == 0`; `([1, 1, 1], 0, 0) == 0`. `block_maxima` of a 4³ volume of zeros with 9.0 at (3, 3, 3), block 2, gives dims `[2, 2, 2]` and 9.0 in the last block only. A `proptest`: for random small volumes `sample` at every voxel centre equals `get`.
- [ ] **Step 2:** Run `cargo test -p voxa-volume`. Expected: FAIL.
- [ ] **Step 3:** Implement. The box test works in voxel space, where the box is axis-aligned whatever the tilt of the stack.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(volume): sampling, rays, first hit and reduction`.

---

### Task 3: The camera

**Files:** `crates/voxa-render/src/camera.rs`, `crates/voxa-render/Cargo.toml` (add `voxa-volume`)

**Interfaces:**
- Consumes: `Ray` (Task 2), `Affine`, `Point`, `Patient`, `Screen`.
- Produces:

```rust
pub enum NamedView { Anterior, Posterior, Left, Right, Superior, Inferior }
pub struct Camera { pub target: Point<Patient>, pub yaw: f64, pub pitch: f64, pub mm_per_px: f64 }   // radians; Debug, Clone, Copy, PartialEq
pub const ORBIT_DEGREES_PER_PX: f64 = 0.5;
pub const MIN_MM_PER_PX: f64 = 0.01;
pub const MAX_MM_PER_PX: f64 = 10.0;
impl Camera {
    pub fn looking(view: NamedView, target: Point<Patient>, mm_per_px: f64) -> Self;
    pub fn fitted(view: NamedView, corners: &[Point<Patient>; 8], viewport: [f32; 2]) -> Self;  // the box fills 90% of the tighter side
    pub fn axes(&self) -> [[f64; 3]; 3];                       // screen right, screen down, forward, in the patient
    pub fn screen_to_patient(&self, viewport: [f32; 2]) -> Affine<Screen, Patient>;   // z is mm in front of the target
    pub fn ray_through(&self, viewport: [f32; 2], at: [f32; 2]) -> Ray;
    pub fn orbited(self, by_px: [f32; 2]) -> Self;             // pitch held within a quarter turn either way
    pub fn panned(self, by_px: [f32; 2]) -> Self;              // the scene follows the pointer
    pub fn zoomed_about(self, factor: f32, about_px: [f32; 2]) -> Self;   // about is from the middle of the viewport
    pub fn centred_on(self, point: Point<Patient>) -> Self;
}
```
Conventions, which the tests pin: yaw 0, pitch 0 is `Anterior`, facing the patient's face. Forward is `cos(pitch) * (-sin(yaw), cos(yaw), 0) + sin(pitch) * (0, 0, -1)`; screen right is `(cos(yaw), sin(yaw), 0)`; screen down is forward × right. `Left` is yaw 90°, `Posterior` 180°, `Right` 270°, `Superior` yaw 180° pitch 90°, `Inferior` yaw 0 pitch -90°. Dragging right lowers the yaw; dragging down raises the pitch.

- [ ] **Step 1: Write failing tests.** `axes` for `Anterior` is `[(1, 0, 0), (0, 0, -1), (0, 1, 0)]`; for `Left` the screen right is (0, 1, 0); for `Superior` it is `[(-1, 0, 0), (0, 1, 0), (0, 0, -1)]`; for `Inferior` `[(1, 0, 0), (0, 1, 0), (0, 0, 1)]`, each within 1e-12. **Review Focus 1:** for all six views right × down equals forward, so no view is mirrored. `screen_to_patient` for `Anterior`, target at the origin, 1 mm a pixel, viewport 200 × 100: (100, 50, 0) → (0, 0, 0); (110, 50, 0) → (10, 0, 0); (100, 60, 0) → (0, 0, -10); (100, 50, 5) → (0, 5, 0). `orbited([180, 0])` from `Anterior` has the axes of `Right`; `orbited([0, 180])` has forward (0, 0, -1); `orbited([0, 1000])` has the same pitch. `panned([10, 0])` at 2 mm a pixel from `Anterior` moves the target to (-20, 0, 0). `zoomed_about(2.0, [50, 0])` from `Anterior` at 1 mm a pixel gives 0.5 mm a pixel and target (25, 0, 0); a factor of 1e9 stops at `MIN_MM_PER_PX`; NaN changes nothing. `fitted(Anterior, ..)` for a box 100 × 200 × 50 mm centred on the origin in a 400 × 400 viewport gives `mm_per_px` within 1e-9 of 100 / 360 and the target at the origin; `fitted(Left, ..)` gives 200 / 360. `ray_through` the middle of the viewport starts behind the target and points forward. A `proptest`: `screen_to_patient` can always be undone, and undoing it returns the screen point.
- [ ] **Step 2:** Run `cargo test -p voxa-render camera`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): an orthographic camera in patient space`.

---

### Task 4: The volume pipeline: Vr and Mip

**Files:** `crates/voxa-render/src/{volume_pipeline,scene,offscreen_scene}.rs`, `volume.wgsl`, `crates/voxa-render/src/testing.rs` (test only)

**Interfaces:**
- Consumes: `Volume`, `halved`, `halvings_needed`, `value_range` (Tasks 1, 2); `Camera` (Task 3); `WindowLevel`, `RenderError` (milestone 1).
- Produces:

```rust
pub enum Look { Surface, Vr, Mip }
pub struct CutPlane { pub point: Point<Patient>, pub normal: [f64; 3] }    // the side the normal points to is removed
pub struct SlicePlane { pub point: Point<Patient>, pub normal: [f64; 3] }
pub const MAX_CUTS: usize = 3;
pub struct SceneParams { pub camera: Camera, pub window: WindowLevel, pub look: Look, pub cuts: Vec<CutPlane>,
    pub slice: Option<SlicePlane>, pub quality: f32 /* 1.0 is full */, pub skip_empty: bool }   // Clone, PartialEq
pub struct VolumePipeline { .. }
impl VolumePipeline {
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self;
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, volume: &Volume) -> Result<u32, RenderError>;
        // Ok(times halved to fit the device); calls upload_within with the device's own limits
    pub fn upload_within(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, volume: &Volume,
        max_side: u32, max_bytes: u64) -> Result<u32, RenderError>;
    pub fn has_volume(&self) -> bool;
    pub fn set_params(&mut self, queue: &wgpu::Queue, params: &SceneParams, viewport: [f32; 2]);
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>);
}
pub struct HeadlessScene { .. }
impl HeadlessScene { pub fn new() -> Result<Self, RenderError>;
    pub fn upload(&mut self, volume: &Volume) -> Result<u32, RenderError>;
    pub fn render(&mut self, params: &SceneParams, size: [u32; 2]) -> Result<Vec<u8>, RenderError>; }   // RGBA, opaque
```
The volume is uploaded once as an `R16Float` 3D texture holding `(value - smallest) / (largest - smallest)`, read with the device's own trilinear filtering. For each pixel the shader builds a ray from `screen_to_patient`, clips it to the volume's box, and steps along it in millimetres (half the smallest spacing at full quality, longer as `quality` falls). The grey of a sample is the window applied to its value. In `Vr`, a sample's opacity is `1 - exp(-0.05 * ramp * step_mm)`, where `ramp` runs from 0 at the window's low end to 1 at its high end, so the picture does not change with the step. In `Mip`, the pixel is the largest sample on the ray. Outside the box and where nothing is hit the pixel is opaque black. `Surface`, cut planes, the slice plane and `skip_empty` are accepted and ignored until Tasks 5 to 7.

The test volume, `testing::ball()`, used from here to Task 7: 48³ voxels of 1 mm with the box centred on the patient origin; value 100 within 16 mm of the centre, 300 within 6 mm, and a bead of value 400 within 3 mm of (12, -14, 0), which pokes out at the front on the patient's left. The test window is `WindowLevel::new(200.0, 400.0)`, which makes the shell grey 64, the core 191 and the bead 255. Pictures are 96 × 96 with `Camera::fitted`.

- [ ] **Step 1: Write failing tests.** `Mip` from `Anterior`: the corner pixel is `[0, 0, 0, 255]`; the centre pixel is 191 ± 3; the brightest pixel is 255 and lies in the right half. **Review Focus 1:** `Mip` from `Posterior`: the brightest pixel lies in the left half. `Vr` from `Anterior`: the centre pixel is brighter than the pixel 14 mm to the left of it, and the picture's right half sums brighter than its left. **Review Focus 2:** the same ball sampled on voxels of 1 × 1 × 3 mm (48 × 48 × 16) gives a silhouette whose width and height in `Mip` from `Anterior` differ by at most 2 pixels; and with the third column of the transform tilted to (0, 0.6, 3), the ball being sampled at the patient position of each voxel so that it is still a ball, the silhouette is still within 2 pixels of round. `quality` 0.5 against 1.0 in `Vr`: the mean difference is under 4 grey levels. **Review Focus 3:** `upload_within` of a 64³ ball with `max_side` 32 returns `Ok(1)` and draws a silhouette within 3 pixels of the one drawn at full size. Nothing uploaded draws black. A changed window changes the picture with no new upload.
- [ ] **Step 2:** Run `cargo test -p voxa-render scene`. Expected: FAIL.
- [ ] **Step 3:** Implement. `half` converts the values; add it to the workspace and confirm `cargo deny check`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): ray-cast a volume, translucent and brightest-value`.

---

### Task 5: The Surface look

**Files:** `crates/voxa-render/src/volume.wgsl`, `volume_pipeline.rs`

**Interfaces:** Consumes and produces as Task 4; `Look::Surface` now draws.

In `Surface` the opacity ramp is steep (0 at the window's low end, 1 a tenth of the way up), so the first tissue the ray meets hides what is behind it. Each sample is lit from the camera by the slope of the values around it (six extra reads, taken only where the sample is not transparent), between 35% and 100% of its grey. A ray stops once it is 98% opaque. Each ray starts a fraction of a step in, by a fixed pattern over the pixels, so that slices through the volume do not show as rings.

- [ ] **Step 1: Write failing tests** with the Task 4 ball and window. From `Anterior`: the centre pixel is opaque with grey between 40 and 64 (the lit shell, not the core behind it); the pixel 14 mm above the centre is darker than the centre (the surface turns away); the pixel 12 mm to the right of the centre, on the bead, is brighter than the pixel 12 mm to the left of it. From `Posterior`: no pixel is above 70, the bead being hidden behind the ball. `quality` 0.5 against 1.0: mean difference under 6 grey levels.
- [ ] **Step 2:** Run `cargo test -p voxa-render solid`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): a solid, lit look with early stopping`.

---

### Task 6: Cut planes and the slice inside the scene

**Files:** `crates/voxa-render/src/volume.wgsl`, `volume_pipeline.rs`, `scene.rs`

**Interfaces:**
- Produces, on `CutPlane`: `pub fn facing(camera: &Camera) -> Self` (through the camera's target, removing the half nearer the viewer); `pub fn moved(self, by_mm: f64) -> Self` (along its normal, positive removing more); `pub fn turned(self, camera: &Camera, by_px: [f32; 2]) -> Self` (the normal turns about the camera's screen axes, `ORBIT_DEGREES_PER_PX` a pixel).

A ray starts where it leaves the removed side of every cut plane. Where it starts on a cut plane inside the tissue, that first sample is drawn opaque and unlit, so the cut face shows the scan as a 2D slice would. Only the first `MAX_CUTS` planes count. The slice plane is an opaque sheet showing the scan where the ray meets it: what is in front of it is drawn over it, and the ray stops there.

- [ ] **Step 1: Write failing tests** with the Task 4 ball and window. `Surface` from `Anterior` with one cut through the origin, normal (0, -1, 0): the centre pixel is 191 ± 3 and a pixel 10 mm to the right of it is 64 ± 3. A second cut through the origin with normal (-1, 0, 0) removes the patient's right half as well: the pixel 10 mm to the right of the centre is still 64 ± 3 and the pixel 10 mm to the left is black. A fourth cut changes nothing. `CutPlane::facing` a `Anterior` camera has normal (0, -1, 0) and passes through the target; `moved(5.0)` shifts its point 5 mm against its normal; `turned(camera, [180, 0])` from that gives a normal within 1e-9 of (1, 0, 0), the tip of the normal having followed the pointer to the right, and `turned(camera, [0, 180])` gives (0, 0, -1). `Mip` from `Inferior` with a slice plane through (0, 0, -10), normal (0, 0, 1): the centre pixel is 64 ± 3 where it was 191 without the plane, and no pixel is above 70.
- [ ] **Step 2:** Run `cargo test -p voxa-render cut slice_plane`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): cut planes that show the scan, and the slice inside the scene`.

---

### Task 7: Skipping empty space, and benchmarks

**Files:** `crates/voxa-render/src/volume.wgsl`, `volume_pipeline.rs`, `crates/voxa-render/benches/raycast.rs`

**Interfaces:** Consumes `block_maxima` (Task 2). `SceneParams::skip_empty` now acts.

Beside the volume the pipeline keeps a second, small 3D texture: the largest value in each block of 8³ voxels. A ray crossing a block whose largest value is below the window's low end jumps to the block's far side. `Mip` never skips.

- [ ] **Step 1: Write failing tests.** For `Surface` and `Vr` from `Anterior` and from a view orbited by (40, 25) pixels, the picture with `skip_empty` true differs from the one with it false by at most 2 grey levels at any pixel. A unit test on a pure function `skippable(block_max: f32, window_low: f32) -> bool` at its boundary.
- [ ] **Step 2:** Run `cargo test -p voxa-render skip`. Expected: FAIL.
- [ ] **Step 3:** Implement. Add `criterion` benches drawing a 256 × 256 × 64 made-up head (a ball with a core) at 1280 × 800: each look at quality 1.0 and 0.5, with and without skipping. Run `cargo bench -p voxa-render` and keep the numbers for Task 14.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `perf(render): skip empty space; ray-cast benchmarks`.

---

### Task 8: Building volumes in the background, and scene state

**Files:** `crates/voxa-app/src/{scene,volume_store}.rs`, `state.rs`, `action.rs`, `reducer.rs`, `reducer/scene.rs`, `worker.rs`, `worker/volume.rs`, `session.rs`

**Interfaces:**
- Consumes: `can_be_volume`, `assemble`, `value_range`, `VolumeError` (Task 1); `Camera`, `NamedView`, `Look`, `CutPlane`, `MAX_CUTS` (Tasks 3 to 6); `Workers`, `Session`, `AppState`, `reduce` (milestone 1).
- Produces:

```rust
pub enum MainPanel { Scene, Stack }
pub enum SceneStatus { Empty, Building { done: usize, of: usize }, Ready, Unavailable(Vec<String>) }
pub struct Scene { pub series: Option<SeriesId>, pub status: SceneStatus, pub camera: Camera, pub untouched: bool,
    pub viewport: [f32; 2], pub corners: Option<[Point<Patient>; 8]>, pub halved: u32, pub look: Look,
    pub cuts: Vec<CutPlane>, pub slice_shown: bool }
// AppState gains: pub scene: Scene, pub main: MainPanel (default Scene)
// Action gains:
SceneResized { viewport: [f32; 2] }, SceneOrbit { by: [f32; 2] }, ScenePan { by: [f32; 2] },
SceneZoom { factor: f32, about: [f32; 2] }, SceneView(NamedView), SceneCentre(Point<Patient>), SceneLook(Look),
SceneUploaded { halved: u32 }, CutAdd, CutMove { cut: usize, by_mm: f64 }, CutTurn { cut: usize, by: [f32; 2] },
CutRemove { cut: usize }, ShowSliceInScene(bool), SetMain(MainPanel),
VolumeProgress { series: SeriesId, done: usize, of: usize },
VolumeReady { series: SeriesId, corners: [Point<Patient>; 8] }, VolumeFailed { series: SeriesId, reason: String }
// Effect gains: BuildVolume { series: SeriesId }
pub struct VolumeStore { .. }      // the one volume in use
impl VolumeStore { pub fn get(&self, series: &SeriesId) -> Option<Arc<Volume>>; pub fn put(&mut self, series: SeriesId, volume: Arc<Volume>); pub fn clear(&mut self); }
// Workers::new and Session::new gain an Arc<Mutex<VolumeStore>>; Session gains pub fn volume(&self, series: &SeriesId) -> Option<Arc<Volume>>
```
The scene shows the series of the active viewport. When that changes (`SelectSeries`, `SetActive`, `IndexDone` opening the best series): if `can_be_volume` allows it, the scene becomes `Building { 0, n }` with looks, cuts and camera reset and `BuildVolume` is emitted; otherwise it becomes `Unavailable` holding the error's sentence followed by the sentence of each of the series' problems, and nothing is built. `VolumeReady` for the scene's series makes it `Ready`, stores the corners, and fits a `Anterior` camera; reports about any other series are ignored. Camera actions clear `untouched`; `SceneResized` refits only while `untouched`. `SceneView` fits that view. `CutAdd` adds `CutPlane::facing` the camera and is ignored at `MAX_CUTS`. A worker decodes every slice (taking those already in the frame cache), reports `VolumeProgress` no more often than every eighth slice, assembles, stores, then reports `VolumeReady`. A slice that fails gives `VolumeFailed` with "Slice {n} could not be decoded, so a 3D view cannot be built" followed by the decoder's sentence. A newer request, or another source, stops the work, and nothing is stored for it. `OpenPath` clears the store.

- [ ] **Step 1: Write failing reducer tests** from a testkit index: selecting the 30-slice series makes the scene `Building { done: 0, of: 30 }` and emits `BuildVolume` after `RequestFrames`. **Review Focus 4:** selecting a three-slice locating series (three planes) gives `Unavailable` whose first line is "This series cannot be shown in 3D." and whose second is the mixed-orientations sentence, with no `BuildVolume`; selecting a one-slice series gives the too-few-slices sentence. `VolumeReady` for the scene's series gives `Ready` and a camera equal to `Camera::fitted(Anterior, corners, viewport)`; for another series it changes nothing. `SceneOrbit` then `SceneResized` leaves the camera's scale alone; `SceneResized` on an untouched scene refits. Four `CutAdd` leave three cuts; `CutRemove { cut: 9 }` changes nothing. `SetMain(Stack)` then `SetMain(Scene)` round-trips. `OpenPath` empties the scene.
- [ ] **Step 2: Write failing worker and session tests.** `BuildVolume` on a testkit folder of 30 slices reports progress that never goes down, ends with `VolumeReady`, and the store holds a volume of dims `[4, 4, 30]` whose `get(1, 0, 29)` is the right stored value. **Review Focus 5:** with slice 7's pixel data cut short the only end is `VolumeFailed` naming "Slice 7", and the store is empty; a second export opened while the first volume is building leaves nothing of the first in the store; a `BuildVolume` for series B issued straight after one for series A leaves only B. A task that panics does not stop the workers. End to end without a window: opening a testkit zip ends with the scene `Ready` for the most detailed series and its volume in the store.
- [ ] **Step 3:** Run `cargo test -p voxa-app scene volume`. Expected: FAIL.
- [ ] **Step 4:** Implement.
- [ ] **Step 5:** Run the tests. Expected: PASS.
- [ ] **Step 6:** Commit: `feat(app): build volumes in the background and keep the scene in state`.

---

### Task 9: The 3D viewport

**Files:** `crates/voxa-app/src/ui/scene.rs`, `ui/scene/{input,captions}.rs`, `ui/mod.rs`

**Interfaces:**
- Consumes: `VolumePipeline`, `SceneParams` (Tasks 4 to 7); `first_hit`, `Ray` (Task 2); the scene actions (Task 8); `scale_bar` and the letter function of milestone 1's `overlay.rs`.
- Produces (pure, in `input.rs` and `captions.rs`):

```rust
pub struct SceneInput { pub left_drag: [f32; 2], pub middle_drag: [f32; 2], pub right_drag: [f32; 2], pub shift: bool,
    pub wheel: i32, pub pointer: [f32; 2], pub pressed: bool, pub double_clicked_surface: Option<Point<Patient>>,
    pub window_width: f32 }
pub fn scene_input_to_actions(active: usize, input: &SceneInput) -> Vec<Action>;
pub fn captions(spacing_mm: [f64; 3], halved: u32) -> Vec<String>;
pub fn edge_letters(camera: &Camera) -> EdgeLabels;
pub fn quality(since_moved: Duration) -> f32;        // 0.5 until 150 ms after the camera last moved, then 1.0
pub fn surface_threshold(window: WindowLevel) -> f32;   // a tenth of the way up the window, as the Surface look
pub fn scene_params(state: &AppState, quality: f32) -> Option<SceneParams>;
    // the scene's camera, look and cuts with the active viewport's window; the slice plane when slice_shown;
    // None unless the scene is Ready
```
The volume is drawn into an off-screen picture the size of the viewport times `quality`, registered with `egui` as a texture and shown stretched over the viewport. It is drawn again only when `SceneParams`, the size or the uploaded volume changes. The volume is uploaded when the scene becomes `Ready`, and `SceneUploaded` reports how often it was halved. Input: left drag orbits; middle drag, or left drag with Shift, pans; the wheel, with or without Ctrl, zooms about the pointer by 1.1 a notch; a right drag changes the window of the active viewport at the 2D viewport's rate; a double-click on a surface (`first_hit` at `surface_threshold`) centres on that point, and over nothing does nothing. Over the picture: a letter at each edge from the camera's axes, a scale bar from `mm_per_px`, and at the bottom left the captions. While building: "Building the 3D view: {done} of {of} slices". When unavailable: its sentences, one a line.

- [ ] **Step 1: Write failing tests.** `scene_input_to_actions`: a left drag of (4, -2) → `[SceneOrbit { by: [4.0, -2.0] }]`; the same with Shift → `[ScenePan { .. }]`; a middle drag → `ScenePan`; two notches away → `SceneZoom` with factor 1.21 about the pointer; a right drag of (10, 0) at a window width of 256 → `AdjustWindow { viewport: active, d_center: 0.0, d_width: 10.0 }`, the same as the 2D viewport gives; a double-click with a surface point → `SceneCentre`, without one → nothing. `edge_letters` for the `Anterior` camera is left R, right L, top S, bottom I; for `Left` it is left A, right P; for `Inferior` left R, right L, top A, bottom P. **Review Focus 2:** `captions([0.7, 0.7, 7.5], 0)` is `["Slices are 7.5 mm apart, so detail between them is interpolated, not measured."]`; `captions([1.0, 1.0, 1.0], 0)` is empty. **Review Focus 3:** `captions([1.0, 1.0, 1.0], 1)` is `["Shown at half resolution to fit this graphics device."]` and with 2 says "a quarter". `quality` is 0.5 at 0 and 149 ms and 1.0 at 150 ms. `scene_params` for a ready scene showing slice 3 of a testkit series with slices 5 mm apart along the patient's head-foot axis has a slice plane through z = 15 with normal (0, 0, 1) and the active viewport's window; with `slice_shown` false it has no slice plane; while the scene is building it is `None`. A headless `run_ui` pass of the viewport in each `SceneStatus` draws without a graphics device and emits no action.
- [ ] **Step 2:** Run `cargo test -p voxa-app ui::scene`. Expected: FAIL.
- [ ] **Step 3:** Implement. Then run the app on the `write_phantom` study with `VOXA_CAPTURE` and look at the picture: the ball is in the main area, lit, with the bead at the upper right from the front and the four letters correct.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): the 3D viewport, with camera, letters, scale and captions`.

---

### Task 10: The window around it

**Files:** `crates/voxa-app/src/ui/{layout,toolbar,mod}.rs`, `ui/help.rs`

**Interfaces:**
- Consumes: `MainPanel`, `Layout`, the scene actions (Task 8); the viewport grid of milestone 1.
- Produces: `pub struct Panels { pub main: Rect, pub side: Rect }` and `pub fn panels(area: Rect) -> Panels` in `layout.rs` (the side column is 360 points wide, or a third of the area if that is less, with the milestone 1 gap between).

The main area shows the 3D viewport and the side column the 2D viewports, or the other way about when `state.main` is `Stack`; a button at the top right of each panel enlarges it, and Escape puts the 3D view back. The toolbar gains, on a second line: the three looks, the six named views, "Cut" (adds a plane), a remove button for each cut, and "Slice in 3D". The key list gains the 3D mouse actions and `Esc`.

- [ ] **Step 1: Write failing tests.** `panels` on a 1200 × 700 area gives a side column 360 wide at the right and a main area taking the rest less the gap; on a 600-wide area the column is 200. A sweep of clicks along the new toolbar line emits each of `SceneLook(Surface | Vr | Mip)` except the one in use, `SceneView` for all six, `CutAdd`, and `ShowSliceInScene(false)`; with two cuts in the state it also emits `CutRemove { cut: 0 }` and `CutRemove { cut: 1 }`. The six view buttons read A, P, L, R, S, I and the three look buttons read Surface, VR, MIP. A click on the enlarge button of the side panel emits `SetMain(Stack)`. `shortcut_action(Key::F, _)` still gives `Fit` for the active viewport, and the reducer, on `Fit` while the scene is the main panel, also refits the camera. `shortcut_action(Key::Escape, _)` is `SetMain(Scene)`.
- [ ] **Step 2:** Run `cargo test -p voxa-app ui::layout ui::toolbar`. Expected: FAIL.
- [ ] **Step 3:** Implement, then capture the window with `VOXA_CAPTURE` and check that nothing overlaps at 1440 × 900 and at the minimum size.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): 3D in the main area, 2D beside it, and the scene's toolbar`.

---

### Task 11: Handles for the cut planes and the slice

**Files:** `crates/voxa-app/src/ui/scene/handles.rs`, `ui/scene.rs`

**Interfaces:**
- Consumes: `Camera`, `CutPlane`, `SlicePlane` (Tasks 3, 6); `Action::{CutMove, CutTurn, Scroll}`.
- Produces (pure):

```rust
pub struct Handle { pub centre: [f32; 2], pub knob: [f32; 2] }   // viewport points; the knob is 60 points along the normal as seen
pub fn cut_handle(camera: &Camera, viewport: [f32; 2], cut: &CutPlane) -> Handle;
pub fn drag_to_mm(camera: &Camera, normal: [f64; 3], drag: [f32; 2]) -> f64;   // along the normal as seen; by the vertical drag when it points at the viewer
pub fn slice_drag_to_scroll(camera: &Camera, normal: [f64; 3], spacing_mm: f64, drag: [f32; 2], carried: &mut f64) -> i32;
```
Each cut plane has a square handle at its point: dragging it emits `CutMove` by `drag_to_mm`. A round knob at the end of its normal: dragging it emits `CutTurn`. The outline of the slice shown in the active 2D viewport is drawn in the scene when `slice_shown`; dragging its handle emits `Scroll` for that viewport. A press on a handle does not orbit.

- [ ] **Step 1: Write failing tests.** For a `Left` camera at 0.5 mm a pixel and a cut with normal (0, -1, 0) (pointing to the left of the screen): `cut_handle` puts the knob 60 points to the left of the centre; `drag_to_mm(.., [-20, 0])` is 10.0 and `[20, 0]` is -10.0; a vertical drag gives 0. For a `Anterior` camera, where that normal points at the viewer, `drag_to_mm(.., [0, 20])` is 10.0. `slice_drag_to_scroll` with a normal along the screen's up, spacing 5 mm, at 0.5 mm a pixel: a drag of 10 points up gives 1 with nothing carried; 6 points gives 0 and carries 3 mm; a further 6 gives 1.
- [ ] **Step 2:** Run `cargo test -p voxa-app handles`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): drag cut planes and the slice from inside the scene`.

---

### Task 12: Saved views

**Files:** `crates/voxa-app/src/{views,settings}.rs`, `reducer/scene.rs`, `ui/scene/views.rs`

**Interfaces:**
- Consumes: `Scene` (Task 8).
- Produces:

```rust
pub struct SavedView { pub name: String, pub yaw: f64, pub pitch: f64, pub mm_per_px: f64, pub target: [f64; 3],
    pub look: String, pub window: [f32; 2], pub cuts: Vec<([f64; 3], [f64; 3])> }    // serde; look is "Surface", "Vr" or "Mip", anything else read as Surface
pub const MAX_VIEWS: usize = 20;
pub fn series_key(id: &SeriesId) -> String;          // 16 hex digits of the 64-bit FNV-1a hash of the identifier
pub fn load(folder: &Path, key: &str) -> Vec<SavedView>;      // an unreadable or damaged file is an empty list
pub fn save(folder: &Path, key: &str, views: &[SavedView]) -> std::io::Result<()>;
// Action gains: ViewSave { name: String }, ViewApply(usize), ViewDelete(usize), ViewsLoaded { series: SeriesId, views: Vec<SavedView> }
// Effect gains: LoadViews { series: SeriesId }, StoreViews { series: SeriesId, views: Vec<SavedView> }
```
`ViewSave` captures the camera, look, window of the active viewport and cuts; an empty name becomes "View {n}", a name is cut at 40 characters, and at `MAX_VIEWS` the oldest goes. `ViewApply` restores all of it and counts as touching the camera. The file is `views.json` in the folder `directories` gives for Voxa's settings, one entry per `series_key`. The workers read and write it; `Session::new` takes the folder as an `Option<PathBuf>`, so tests use a temporary one and `None` turns the file off.

- [ ] **Step 1: Write failing tests.** `series_key` of "1.2.3" is the same on every call, differs from that of "1.2.4", is 16 hex digits and does not contain the identifier. `save` then `load` in a temporary folder returns the same views; two series in one file do not disturb each other; a file of `not json` loads as an empty list; a missing folder is created. Reducer: `ViewSave` then `SceneOrbit` then `ViewApply(0)` restores the camera exactly and emits `StoreViews`; the 21st save drops the first; `ViewDelete(5)` with two views changes nothing; selecting a series emits `LoadViews`, and `ViewsLoaded` for another series is ignored. The saved file, read as text, contains none of the series identifier, description or study date of the testkit series.
- [ ] **Step 2:** Run `cargo test -p voxa-app views`. Expected: FAIL.
- [ ] **Step 3:** Implement, with a "Views" menu on the scene's toolbar line: Save this view, then each saved view with a delete button.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): saved views, kept between sessions`.

---

### Task 13: Snapshot and turntable export

**Files:** `crates/voxa-render/src/camera.rs`, `crates/voxa-app/src/export.rs`, `ui/scene/export.rs`

**Interfaces:**
- Consumes: `VolumePipeline` drawing into an off-screen picture through the window's device (Tasks 4, 9); `SceneParams`.
- Produces:

```rust
// voxa-render
pub fn turntable(camera: &Camera, frames: u32) -> Vec<Camera>;       // one full turn of yaw, the first frame as now
// voxa-app
pub fn png(rgba: &[u8], size: [u32; 2]) -> Result<Vec<u8>, ExportError>;
pub fn gif(frames: &[Vec<u8>], size: [u32; 2], seconds: f32) -> Result<Vec<u8>, ExportError>;   // grey, looping
pub enum ExportError { Size, Encode(String), Write(String) }
pub const SUGGESTED_NAME: &str = "voxa-3d";
```
"Save picture…" draws the scene at full quality at the viewport's size and writes a PNG where the person chooses. "Save turntable…" draws 72 frames at up to 512 points on the longer side, shows progress with a Cancel, and writes a GIF lasting 6 seconds. Neither draws letters, captions, handles or any text.

- [ ] **Step 1: Write failing tests.** `png` of a 3 × 2 picture decodes with the `image` crate to 3 × 2 with the same pixels; a buffer of the wrong length is `ExportError::Size`. `turntable(camera, 72)` has 72 cameras, the first equal to `camera`, each next with yaw 5° less, and all with the same pitch, target and scale; `turntable(camera, 0)` is empty. `gif` of four 8 × 8 frames decodes to four frames of 8 × 8 whose greys are within 1 level of the originals; no frames is `ExportError::Size`. In `voxa-render`, the Task 4 ball drawn in `Mip` through `turntable(front, 72)` has its brightest pixel in the right half in frame 0 and in the left half in frame 36.
- [ ] **Step 2:** Run `cargo test -p voxa-app export` and `cargo test -p voxa-render turntable`. Expected: FAIL.
- [ ] **Step 3:** Implement. Enable the `gif` feature of `image` and confirm `cargo deny check`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): save a picture or a turntable of the 3D view`.

---

### Task 14: Acceptance and release 0.2.0

**Files:** `docs/acceptance-0.2.0.md`, `CHANGELOG.md`, `README.md`, `docs/architecture.md`, `docs/images/`

- [ ] **Step 1:** Run `cargo xtask check` and `cargo xtask coverage`. Expected: all pass, the three non-GPU crates at or above 80%. Fix anything that fails.
- [ ] **Step 2:** Run `cargo bench -p voxa-render` and `-p voxa-app`. Record in `docs/acceptance-0.2.0.md` the frame time of each look at full and half quality against the 30-a-second target, and say plainly which graphics device they were measured on and that integrated graphics was or was not available to measure.
- [ ] **Step 3: Acceptance on a real export** (local only; the record names nothing from it): every series that can be a volume builds and draws in all three looks; every series that cannot says why; time from selecting the largest series to a full-quality picture; a cut plane and the slice-in-scene on one series; a snapshot and a turntable saved and opened. Pictures of real scans stay out of the repository. Anything a person must try by hand (dragging, the handles, the save dialogs, how it feels) is listed as not checked.
- [ ] **Step 4:** Update `CHANGELOG.md` with 0.2.0, `docs/architecture.md` with the volume path, and `README.md` with a picture of the made-up study in 3D.
- [ ] **Step 5:** Commit: `chore: acceptance record and 0.2.0 changelog`, then `git tag v0.2.0`. Do not push.

---

## Out of scope

Labels and everything that needs them (hover names, selection, visibility, peeling by structure), plane views, measurements and notes, registration, models, perspective projection, two volumes in one scene, undo and redo.
