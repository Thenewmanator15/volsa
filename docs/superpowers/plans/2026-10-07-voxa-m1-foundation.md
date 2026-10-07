# Voxa Milestone 1 (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Windows desktop app that opens a scan export (zip, folder or DICOMDIR), lists its studies and series, and shows any series as a 2D stack with correct orientation, contrast controls, overlays, tag inspection and cine playback, with every quality gate running from the first commit.

**Architecture:** A Cargo workspace of small crates that talk only through their public interfaces: `voxa-core` (data model, typed coordinates, errors), `voxa-dicom` (sources, indexing, decoding), `voxa-volume` (histogram, automatic window), `voxa-render` (GPU slice pipeline), `voxa-app` (egui interface, state, background work). State changes only through one `Action` enum handled by one reducer. Indexing and decoding run on worker threads; window/level is applied in the shader so contrast changes never re-decode.

**Tech Stack:** Rust 1.99.0 (pinned), `eframe`/`egui` 0.36 with the wgpu backend (use the `wgpu` re-exported by `egui-wgpu`, never a second copy), `dicom` 0.10 with `dicom-pixeldata` (features `native`, `openjp2`), `zip` (latest stable), `rfd` 0.17, `thiserror` 2, `tracing`, `rayon`, `bytemuck`, `image` (tests and thumbnails), `criterion` 0.8, `proptest`, `cargo-deny`, `cargo-llvm-cov`. Confirm exact feature names on docs.rs at the start of the task that adds each dependency.

**Spec:** `docs/superpowers/specs/2026-10-05-voxa-design.md` (Part B is this milestone; Part A sections A2, A3 and A6 bind every task).

## Global Constraints

- Disclaimer, verbatim, in README, About screen and main window: "Voxa is not a medical device and is not for diagnostic use."
- Dual licence MIT / Apache-2.0: `LICENSE-MIT`, `LICENSE-APACHE`, and `license = "MIT OR Apache-2.0"` in every crate.
- No network access and no telemetry anywhere in this milestone. No dependency that phones home.
- Patient data never enters the repository. Tests use synthetic DICOM built in the test, nothing else.
- The repository stays local: no `git push`, no remote, until the owner says so.
- `#![forbid(unsafe_code)]` in every crate except `voxa-render`; any `unsafe` there carries a `// SAFETY:` comment.
- Library crates never panic: `unwrap`, `expect`, `panic!`, `todo!`, indexing that can panic are denied by lint outside `#[cfg(test)]`. Errors are `thiserror` enums.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, tests, `cargo deny check`, `cargo doc --no-deps -D warnings` must all pass; `cargo xtask check` runs exactly these.
- Soft limits enforced by clippy config: 400 lines per file (`too_many_lines` threshold 60 for functions).
- Every public item has a doc comment (`#![deny(missing_docs)]`).
- Toolchain pinned in `rust-toolchain.toml` to `1.99.0`; `rust-version = "1.99"` in the workspace; `Cargo.lock` committed.
- Coverage floor 80% lines on `voxa-core`, `voxa-dicom`, `voxa-volume` (checked with `cargo llvm-cov`).
- Logging through `tracing`; never log patient name, ID, birth date or any tag value from groups 0010.
- Conventional commit messages. Each task ends with one commit.
- Crates depend only downward: core ← dicom, volume ← render ← app. `voxa-app` contains no image mathematics.
- `Patient`, `Voxel` and `Screen` coordinates are distinct types; converting needs an explicit transform.

## Review Focus

Inputs the spec implies but does not spell out, most likely first. Each has its test in the named task.

1. **Enhanced (multi-frame) DICOM and multi-volume series.** A whole series in one file must appear as a series; a series with repeated slice positions (diffusion, fMRI) must open as a plain stack flagged with the reason, not be mis-stacked. Task 4.
2. **Exports with junk in them.** A zip holding a PDF, a `DICOMDIR`, nested folders and non-DICOM files must list its series and report "n files skipped", never fail. Tasks 3 and 4.
3. **Tilted acquisitions.** With direction cosines such as (1, 0, 0, 0, 0.98, 0.2) the orientation letters must still show the dominant direction on each edge, and left/right must be correct under both display conventions. Task 10.
4. **Awkward pixel encodings.** Signed pixels with a negative rescale intercept, `MONOCHROME1`, and a window width of zero or less must display without panic or inverted contrast; width is clamped to at least 1. Tasks 5 and 6.
5. **A series larger than the memory budget.** The current slice must always be shown, and scrolling must still work, when the frame cache budget is smaller than the series. Task 8.

---

## File Structure

```
Cargo.toml  rust-toolchain.toml  deny.toml  clippy.toml  rustfmt.toml  .gitignore
LICENSE-MIT  LICENSE-APACHE  README.md  CHANGELOG.md  CONTRIBUTING.md
.github/workflows/ci.yml
docs/architecture.md  docs/adr/0001-stack.md  0002-coordinates.md  0003-errors.md
xtask/src/main.rs
crates/voxa-core/src/     lib.rs error.rs space.rs model.rs window.rs
crates/voxa-dicom/src/    lib.rs source.rs index.rs geometry.rs decode.rs tags.rs
crates/voxa-dicom/src/testkit.rs          (feature "testkit": synthetic DICOM builders)
crates/voxa-volume/src/   lib.rs histogram.rs auto_window.rs
crates/voxa-render/src/   lib.rs slice_pipeline.rs slice.wgsl view.rs offscreen.rs
crates/voxa-app/src/      main.rs lib.rs state.rs action.rs reducer.rs worker.rs cache.rs
crates/voxa-app/src/ui/   mod.rs theme.rs browser.rs viewport.rs overlay.rs toolbar.rs tags.rs about.rs
crates/voxa-app/benches/  decode.rs
```

---

### Task 1: Workspace, quality gates and project documents

**Files:** create everything at the repository root listed above, `xtask/`, `.github/workflows/ci.yml`, `docs/architecture.md`, `docs/adr/*`, and each crate as an empty library with its lint header.

**Interfaces:**
- Produces: `cargo xtask check` (runs fmt, clippy, test, deny, doc in that order and stops at the first failure); `cargo xtask coverage` (runs `cargo llvm-cov` and fails under 80% on the three non-GPU crates). Every crate's `lib.rs` starts with `#![forbid(unsafe_code)]` (not `voxa-render`), `#![deny(missing_docs)]`, and `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]`.

- [ ] **Step 1:** Create the workspace `Cargo.toml` with `[workspace.package]` (edition 2024, `rust-version = "1.99"`, `license = "MIT OR Apache-2.0"`, version `0.1.0`), `[workspace.dependencies]` for every shared dependency, and `[workspace.lints.clippy]` (`pedantic = "warn"`, `too_many_lines = "deny"`). `clippy.toml`: `too-many-lines-threshold = 60`. `rust-toolchain.toml`: channel `1.99.0`, components `rustfmt`, `clippy`, `llvm-tools-preview`.
- [ ] **Step 2:** Create the five crates and `xtask` with one documented placeholder item each and a test `fn crate_builds() {}`.
- [ ] **Step 3:** `deny.toml`: allow licences MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0; deny advisories; deny multiple versions of `wgpu`.
- [ ] **Step 4:** `xtask/src/main.rs` with subcommands `check` and `coverage`, each printing the command before running it and exiting non-zero on failure.
- [ ] **Step 5:** `.github/workflows/ci.yml`: jobs on `windows-latest` and `ubuntu-latest` running `cargo xtask check`, plus `cargo xtask coverage` on Ubuntu. No job uploads anything off the runner except build artefacts.
- [ ] **Step 6:** `README.md` (what Voxa is, the disclaimer verbatim, build and run, `cargo xtask check`), `CONTRIBUTING.md` (test-first, commit style, the constraints above), `CHANGELOG.md` (Unreleased), both licence files, `docs/architecture.md` (crate table and data flow from spec A3 and B4), and three ADRs: stack choice, coordinate conventions (DICOM LPS, typed spaces), error policy.
- [ ] **Step 7:** Run `cargo xtask check`. Expected: every stage prints and passes.
- [ ] **Step 8:** Commit: `chore: scaffold workspace, quality gates and project docs`.

---

### Task 2: `voxa-core` — errors, typed coordinates, data model

**Files:** `crates/voxa-core/src/{lib,error,space,model,window}.rs`

**Interfaces — Produces:**

```rust
pub struct Patient;  pub struct Voxel;  pub struct Screen;          // marker types
pub struct Point<S> { pub x: f64, pub y: f64, pub z: f64, .. }       // z is 0.0 for Screen
pub struct Affine<From, To> { .. }                                    // 4x4, row-major f64
impl<A, B> Affine<A, B> {
    pub fn from_columns(x: [f64; 3], y: [f64; 3], z: [f64; 3], origin: [f64; 3]) -> Self;
    pub fn apply(&self, p: Point<A>) -> Point<B>;
    pub fn inverse(&self) -> Option<Affine<B, A>>;                    // None when singular
    pub fn then<C>(&self, next: &Affine<B, C>) -> Affine<A, C>;
}
pub struct WindowLevel { pub center: f32, pub width: f32 }
impl WindowLevel { pub fn new(center: f32, width: f32) -> Self; }     // width clamped to >= 1.0
pub struct SeriesId(pub String);  pub struct StudyId(pub String);
pub enum SeriesProblem { NotMr(String), TooFewSlices(usize), MixedSizes, UnevenSpacing,
    RepeatedPositions, MissingGeometry, UnsupportedTransferSyntax(String) }
pub struct SeriesInfo { pub id: SeriesId, pub number: i32, pub description: String, pub modality: String,
    pub rows: u32, pub cols: u32, pub slices: usize, pub spacing_mm: [f64; 3],
    pub voxel_to_patient: Option<Affine<Voxel, Patient>>, pub problems: Vec<SeriesProblem> }
impl SeriesInfo { pub fn has_geometry(&self) -> bool; pub fn slice_thickness_mm(&self) -> f64; }
pub struct StudyInfo { pub id: StudyId, pub date: Option<String>, pub description: String, pub series: Vec<SeriesInfo> }
pub enum VoxaError { Io(..), NotDicom, Decode(String), UnsupportedTransferSyntax(String), EmptySource, .. }
```

- [ ] **Step 1: Write failing tests** in `space.rs` and `window.rs`:

```rust
#[test] fn affine_maps_voxel_to_patient() {
    let a: Affine<Voxel, Patient> = Affine::from_columns([0.9,0.,0.],[0.,0.9,0.],[0.,0.,5.],[-100.,-120.,0.]);
    let p = a.apply(Point::new(10., 20., 3.));
    assert_eq!((p.x, p.y, p.z), (-91.0, -102.0, 15.0));
}
#[test] fn inverse_round_trips() { /* apply then inverse.apply returns the input within 1e-9 */ }
#[test] fn singular_affine_has_no_inverse() { /* zero z column -> inverse() is None */ }
#[test] fn window_width_is_clamped() { assert_eq!(WindowLevel::new(40., 0.).width, 1.0);
                                       assert_eq!(WindowLevel::new(40., -5.).width, 1.0); }
```
plus a `proptest` that `a.then(&b).apply(p)` equals `b.apply(a.apply(p))`.
- [ ] **Step 2:** Run `cargo test -p voxa-core`. Expected: FAIL, items not defined.
- [ ] **Step 3:** Implement the items above. `Point<S>` and `Affine<A, B>` hold `PhantomData` so that `Affine<Voxel, Patient>::apply` rejects a `Point<Screen>` at compile time; add a `compile_fail` doctest proving it.
- [ ] **Step 4:** Run `cargo test -p voxa-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): typed coordinate spaces, window/level and series model`.

---

### Task 3: `voxa-dicom` — sources (folder, zip, DICOMDIR)

**Files:** `crates/voxa-dicom/src/{lib,source}.rs`

**Interfaces — Produces:**

```rust
pub struct Entry { pub name: String, pub bytes: Vec<u8> }
pub trait SeriesSource: Send {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_>;
    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError>;     // re-read one entry later, for decoding
}
pub fn open_source(path: &Path) -> Result<Box<dyn SeriesSource>, VoxaError>;
```
`open_source` returns a folder source for a directory (recursive walk), a zip source for a file with the zip signature (entries read in place, nothing extracted to disk), and a folder source over the parent directory when given a file named `DICOMDIR`. Directory entries are skipped; names use `/`.

- [ ] **Step 1: Write failing tests** using `tempfile`: a folder with nested files yields every file once with `/`-separated relative names; a zip built in the test with the `zip` crate yields the same names and bytes without creating any file beside it (assert the temp directory listing is unchanged); a `DICOMDIR` path yields its siblings; a missing path gives `VoxaError::Io`; an empty folder gives `Ok` with zero entries.
- [ ] **Step 2:** Run `cargo test -p voxa-dicom source`. Expected: FAIL.
- [ ] **Step 3:** Implement `FolderSource`, `ZipSource`, `open_source`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(dicom): folder, zip and DICOMDIR sources`.

---

### Task 4: `voxa-dicom` — header indexing, grouping and geometry

**Files:** `crates/voxa-dicom/src/{index,geometry,testkit}.rs`

**Interfaces:**
- Consumes: `SeriesSource`, `Entry` (Task 3); `SeriesInfo`, `StudyInfo`, `SeriesProblem`, `Affine` (Task 2).
- Produces:

```rust
pub struct SliceRef { pub entry: String, pub frame: Option<u32> }      // frame is Some for enhanced DICOM
pub struct IndexedSeries { pub info: SeriesInfo, pub slices: Vec<SliceRef> }   // sorted along the slice normal
pub struct Index { pub studies: Vec<StudyInfo>, pub series: Vec<IndexedSeries>, pub skipped: Vec<String> }
pub fn index(source: &mut dyn SeriesSource, on_progress: &mut dyn FnMut(&Index)) -> Result<Index, VoxaError>;
pub fn stack_geometry(positions: &[[f64; 3]], orientation: [f64; 6], pixel_spacing: [f64; 2])
    -> Option<StackGeometry>;       // None when fewer than one position
pub struct StackGeometry { pub order: Vec<usize>, pub step_mm: f64, pub steps_mm: Vec<f64>,
                           pub voxel_to_patient: Affine<Voxel, Patient> }
// feature "testkit":
pub fn classic_slice(opts: &SliceOptions) -> Vec<u8>;     // one valid single-frame MR file
pub fn enhanced_series(opts: &EnhancedOptions) -> Vec<u8>; // one valid multi-frame MR file
```
Rules, in this order: group classic files by SeriesInstanceUID and each enhanced file as its own series; sort by position along the normal (row cosine × column cosine), instance number as fallback when geometry is absent; step is the median gap; `UnevenSpacing` when any gap differs from the median by more than 10%; `RepeatedPositions` when any gap is under 0.001 mm; `MixedSizes` when rows or columns differ; `MissingGeometry` when position, orientation or pixel spacing is absent (then `voxel_to_patient` is `None`); `TooFewSlices` is not a problem in this milestone (a stack of one is viewable). Unparseable entries go to `skipped` by name. Studies are ordered by date then UID; series by number then UID. `on_progress` is called after each entry that changes the index.

- [ ] **Step 1: Write the testkit** (`classic_slice`, `enhanced_series`) writing explicit-VR little-endian files with the tags the indexer reads, and a test that `dicom::object::from_reader` parses both.
- [ ] **Step 2: Write failing tests**:
  - 30 classic slices shuffled, 5 mm apart: one series, `slices` in ascending position, `spacing_mm == [0.9, 0.9, 5.0]`, no problems, and `voxel_to_patient` maps `(10, 20, 3)` to `(-91, -102, 15)`.
  - Two SeriesInstanceUIDs under two StudyInstanceUIDs: two studies, ordered by date.
  - A gap in the stack → `UnevenSpacing`; two slices sharing a position → `RepeatedPositions`; one slice of a different size → `MixedSizes`; files lacking ImagePositionPatient → `MissingGeometry`, sorted by InstanceNumber.
  - **Review Focus 1:** `enhanced_series` with 24 frames stored out of order → one series, 24 `SliceRef`s with `frame: Some(_)` in ascending position, no problems.
  - **Review Focus 2:** a source holding the 30 slices plus `report.pdf`, `DICOMDIR` and `notes/readme.txt` → one series and `skipped == ["notes/readme.txt", "report.pdf"]` (the `DICOMDIR` is ignored silently).
  - `on_progress` is called at least once per series and the final call equals the returned index.
  - A `proptest` feeding random bytes as entries: `index` returns `Ok` and never panics.
- [ ] **Step 3:** Run `cargo test -p voxa-dicom --features testkit index`. Expected: FAIL.
- [ ] **Step 4:** Implement `stack_geometry` then `index`, reading only header elements (`OpenFileOptions::read_until(PixelData)`).
- [ ] **Step 5:** Run the tests. Expected: PASS.
- [ ] **Step 6:** Commit: `feat(dicom): index headers into studies and series with geometry checks`.

---

### Task 5: `voxa-dicom` — pixel decoding and tag listing

**Files:** `crates/voxa-dicom/src/{decode,tags}.rs`

**Interfaces:**
- Consumes: `SeriesSource::read`, `SliceRef` (Tasks 3, 4).
- Produces:

```rust
pub struct Frame { pub rows: u32, pub cols: u32, pub values: Vec<f32> }   // rescaled; MONOCHROME1 already inverted
pub fn decode_slice(source: &mut dyn SeriesSource, slice: &SliceRef) -> Result<Frame, VoxaError>;
pub struct TagRow { pub tag: String, pub keyword: String, pub vr: String, pub value: String }
pub fn list_tags(source: &mut dyn SeriesSource, slice: &SliceRef) -> Result<Vec<TagRow>, VoxaError>;
pub fn is_patient_identifying(tag: &str) -> bool;       // true for every (0010,xxxx) tag
```
`values[r * cols + c]` is `stored * slope + intercept`. For `MONOCHROME1` each value becomes `max + min - value` over the frame. Supported transfer syntaxes: implicit and explicit VR little endian, RLE lossless, JPEG baseline, JPEG lossless, JPEG 2000 lossless and lossy; anything else returns `UnsupportedTransferSyntax(name)`. `list_tags` renders sequences as `"<n items>"` and truncates values at 256 characters; binary VRs show their length.

- [ ] **Step 1: Write failing tests** with testkit files: unsigned 16-bit with slope 2 and intercept 10 → `values` equal `2 * stored + 10`; **Review Focus 4:** signed 16-bit with intercept −1024 containing negative stored values → exact expected floats; `MONOCHROME1` → the brightest stored pixel has the smallest value; an enhanced file → frame 5 decodes to frame 5's pixels with its per-frame rescale; a file with a made-up transfer syntax UID → `UnsupportedTransferSyntax`; truncated pixel data → `Decode(_)`, no panic; `list_tags` includes PatientName and `is_patient_identifying("(0010,0010)")` is true while `"(0008,0060)"` is false.
- [ ] **Step 2:** Run `cargo test -p voxa-dicom --features testkit decode`. Expected: FAIL.
- [ ] **Step 3:** Implement with `dicom-pixeldata`'s `decode_pixel_data` and its frame access, converting through `to_vec::<f32>` or the raw stored values plus the modality LUT, whichever the test values require.
- [ ] **Step 4:** Add a JPEG 2000 lossless round-trip test only if an encoder is available in dev-dependencies; otherwise record in `docs/architecture.md` that JPEG 2000 is covered by the manual acceptance test in Task 13.
- [ ] **Step 5:** Run the tests. Expected: PASS.
- [ ] **Step 6:** Commit: `feat(dicom): decode slices to rescaled frames and list tags`.

---

### Task 6: `voxa-volume` — histogram and automatic window

**Files:** `crates/voxa-volume/src/{lib,histogram,auto_window}.rs`

**Interfaces — Produces:**

```rust
pub struct Histogram { .. }
impl Histogram { pub fn from_values(values: &[f32], bins: usize) -> Option<Self>;   // None for empty or all-NaN input
                 pub fn percentile(&self, p: f64) -> f32; }                          // p in 0.0..=100.0
pub fn auto_window(values: &[f32]) -> WindowLevel;       // 1st to 99.5th percentile; fallback center 0, width 1
pub struct Preset { pub name: &'static str, pub low_pct: f64, pub high_pct: f64 }
pub const PRESETS: [Preset; 3];                           // "Auto" 1..99.5, "Soft" 0.5..99.9, "Hard" 5..98
```

- [ ] **Step 1: Write failing tests**: for values `0..=999`, `percentile(50.0)` is within one bin width of 499.5 and `auto_window` gives center within 2 of 502.2 and width within 4 of 984.5; a constant image gives width exactly 1.0 (**Review Focus 4**); an empty slice gives `WindowLevel::new(0.0, 1.0)`; NaN values are ignored; a `proptest` that `percentile` is non-decreasing in `p`.
- [ ] **Step 2:** Run `cargo test -p voxa-volume`. Expected: FAIL.
- [ ] **Step 3:** Implement with 4096 bins over the finite min..max range.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(volume): histogram percentiles and automatic window`.

---

### Task 7: `voxa-render` — GPU slice pipeline

**Files:** `crates/voxa-render/src/{lib,slice_pipeline,view,offscreen}.rs`, `slice.wgsl`

**Interfaces:**
- Consumes: `Frame` shape (rows, cols, `&[f32]`), `WindowLevel`.
- Produces:

```rust
pub struct ViewTransform { pub zoom: f32, pub pan: [f32; 2], pub flip_h: bool, pub flip_v: bool, pub quarter_turns: u8 }
impl ViewTransform { pub fn fit() -> Self;
    pub fn image_to_screen(&self, image: [u32; 2], viewport: [f32; 2], pixel_aspect: f32) -> Affine<Voxel, Screen>; }
pub struct SlicePipeline { .. }
impl SlicePipeline {
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self;
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rows: u32, cols: u32, values: &[f32]);
    pub fn set_params(&mut self, queue: &wgpu::Queue, wl: WindowLevel, view: &ViewTransform, viewport: [f32; 2], pixel_aspect: f32);
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>);
}
pub fn render_to_rgba(rows: u32, cols: u32, values: &[f32], wl: WindowLevel, view: &ViewTransform, size: [u32; 2])
    -> Result<Vec<u8>, RenderError>;        // headless; used by tests
```
The frame is uploaded once as an `R32Float` texture; the shader reads it with `textureLoad`, interpolates bilinearly itself, and maps `(v - (center - width/2)) / width` to grey, clamped to 0..1. Changing window/level or the view updates only a uniform buffer. `wgpu` comes from `egui_wgpu::wgpu`. `render_to_rgba` requests any adapter with `force_fallback_adapter` as a second try and returns `RenderError::NoAdapter` when neither exists.

- [ ] **Step 1: Write failing tests** for `ViewTransform::image_to_screen` (pure): `fit` centres a 200×100 image in a 400×400 viewport at zoom 2 with voxel (0,0) at screen (0,100); `flip_h` mirrors x; one quarter turn swaps the fitted extents; `pixel_aspect` 2.0 doubles the on-screen height.
- [ ] **Step 2: Write failing golden tests** using `render_to_rgba` on a 4×4 ramp `0..16`: with `WindowLevel::new(7.5, 16.0)` the centre pixels of each cell are within 2 grey levels of `value * 16`; with width 1.0 every pixel is 0 or 255; a second call with a different window on the same pipeline changes the output (proving no re-upload is needed is covered by the app test in Task 9). Tests return early with a printed notice when `RenderError::NoAdapter` is returned, and CI on Ubuntu installs a software Vulkan driver so they run there.
- [ ] **Step 3:** Run `cargo test -p voxa-render`. Expected: FAIL.
- [ ] **Step 4:** Implement `view.rs`, then the pipeline and shader, then `offscreen.rs`.
- [ ] **Step 5:** Run the tests. Expected: PASS.
- [ ] **Step 6:** Commit: `feat(render): GPU slice pipeline with shader window/level`.

---

### Task 8: `voxa-app` — state, actions, workers and frame cache

**Files:** `crates/voxa-app/src/{lib,state,action,reducer,worker,cache}.rs`

**Interfaces:**
- Consumes: everything produced by Tasks 2 to 6.
- Produces:

```rust
pub enum Layout { One, TwoAcross, TwoByTwo }                      // 1, 2 or 4 viewports
pub struct ViewportState { pub series: Option<SeriesId>, pub slice: usize, pub wl: WindowLevel, pub wl_auto: bool,
    pub view: ViewTransform, pub cine: Option<Cine> }
pub struct Cine { pub fps: f32, pub looping: bool, pub playing: bool }      // fps clamped to 1.0..=60.0
pub struct AppState { pub index: Option<Index>, pub layout: Layout, pub viewports: [ViewportState; 4],
    pub active: usize, pub radiological: bool /* default true */, pub show_patient: bool /* default false */,
    pub tag_filter: String, pub notices: Vec<Notice> }
pub enum Action { OpenPath(PathBuf), IndexProgress(Index), IndexFailed(String), SelectSeries { viewport: usize, series: SeriesId },
    Scroll { viewport: usize, delta: i32 }, SetSlice { viewport: usize, slice: usize }, Zoom { viewport: usize, factor: f32, about: [f32; 2] },
    Pan { viewport: usize, by: [f32; 2] }, Flip { viewport: usize, horizontal: bool }, Rotate { viewport: usize }, Fit { viewport: usize },
    AdjustWindow { viewport: usize, d_center: f32, d_width: f32 }, SetWindow { viewport: usize, wl: WindowLevel },
    ApplyPreset { viewport: usize, preset: usize }, SetLayout(Layout), SetActive(usize), ToggleRadiological, TogglePatient,
    SetTagFilter(String), CineToggle { viewport: usize }, CineSet { viewport: usize, fps: f32, looping: bool }, CineTick { viewport: usize },
    FrameReady { series: SeriesId, slice: usize }, FrameFailed { series: SeriesId, slice: usize, reason: String }, DismissNotice(usize) }
pub fn reduce(state: &mut AppState, action: Action) -> Vec<Effect>;
pub enum Effect { StartIndex(PathBuf), RequestFrames { series: SeriesId, around: usize }, None }
pub struct FrameCache { .. }
impl FrameCache { pub fn new(budget_bytes: usize) -> Self;
    pub fn insert(&mut self, series: &SeriesId, slice: usize, frame: Arc<Frame>, keep_near: usize);
    pub fn get(&self, series: &SeriesId, slice: usize) -> Option<Arc<Frame>>; pub fn bytes(&self) -> usize; }
pub struct Workers { .. }
impl Workers { pub fn new(sender: Sender<Action>) -> Self; pub fn run(&self, effect: Effect, index: Option<&Index>); }
```
`reduce` is pure. `Scroll` clamps to the series' slice range. `SelectSeries` resets slice to the middle, view to `fit`, sets `wl_auto` and emits `RequestFrames`. `AdjustWindow` clears `wl_auto`. `CineTick` advances one slice and wraps when looping, or stops at the last slice. `IndexProgress` replaces `index` and adds a notice "n files skipped" when `skipped` is non-empty. `FrameCache::insert` evicts frames farthest from `keep_near` first until under budget, but never evicts the frame being inserted. `Workers` decodes on a `rayon` pool, nearest slices to `around` first, and sends `FrameReady`/`FrameFailed`; it never logs tag values.

- [ ] **Step 1: Write failing reducer tests**, each building state from a testkit index: scrolling past either end clamps; selecting a series centres the slice and emits `RequestFrames`; `Rotate` four times returns the original view; `AdjustWindow` with a delta that would make width negative leaves width at 1.0; `CineTick` at the last slice wraps when looping and stops (`playing == false`) when not; `ToggleRadiological` flips the flag; `IndexProgress` with two skipped files adds the notice "2 files skipped"; an out-of-range `viewport` index is ignored without panic.
- [ ] **Step 2: Write failing cache tests**: inserting within budget keeps everything; **Review Focus 5:** with a budget smaller than one frame, `insert` then `get` of the same slice still returns it and `bytes()` equals one frame; with a budget of three frames, inserting slices 0..10 with `keep_near = 9` retains 7, 8, 9.
- [ ] **Step 3: Write a failing worker test**: `Workers::run(RequestFrames{..})` on a testkit folder source sends `FrameReady` for every slice, the requested one first; a slice with truncated pixel data sends `FrameFailed` and the others still arrive.
- [ ] **Step 4:** Run `cargo test -p voxa-app`. Expected: FAIL.
- [ ] **Step 5:** Implement `state.rs`, `action.rs`, `reducer.rs`, `cache.rs`, `worker.rs`.
- [ ] **Step 6:** Run the tests. Expected: PASS.
- [ ] **Step 7:** Commit: `feat(app): state, actions, frame cache and background workers`.

---

### Task 9: `voxa-app` — window, theme, browser and viewport grid

**Files:** `crates/voxa-app/src/main.rs`, `ui/{mod,theme,browser,viewport,toolbar,about}.rs`

**Interfaces:**
- Consumes: `AppState`, `Action`, `reduce`, `Workers`, `FrameCache` (Task 8); `SlicePipeline`, `ViewTransform` (Task 7).
- Produces: `pub struct VoxaApp` implementing `eframe::App`; `pub fn run(initial: Option<PathBuf>) -> eframe::Result`; `pub fn thumbnail(frame: &Frame, wl: WindowLevel, size: u32) -> egui::ColorImage` (pure, in `browser.rs`).

Each frame: drain the worker channel into `reduce`, run returned effects, then draw. Layout: top toolbar; left browser panel 240 px wide listing studies by date with one card per series (thumbnail 64 px, description, "n slices · t mm"); centre grid of 1, 2 or 4 viewports with a 2 px accent border on the active one; a status line carrying the disclaimer verbatim. Series with problems show a warning mark whose hover text lists the problems in plain words. Each viewport owns a `SlicePipeline` drawn through `egui_wgpu::Callback`; it uploads only when the displayed (series, slice) changes and otherwise calls `set_params`. A slice not yet in the cache shows "Loading…"; a failed slice shows "This slice could not be decoded" with the reason. Theme: dark, one accent colour, a bundled open-licence UI font and monospace font (record their licences in `README.md`), 8 px rounding, defined once in `theme.rs`. The About window shows version, licences and the disclaimer.

- [ ] **Step 1: Write failing tests**: `thumbnail` of a 4×4 ramp at size 2 returns a 2×2 image whose pixels increase left to right and top to bottom; `theme::apply` sets `visuals.dark_mode == true` and the accent colour on `selection.bg_fill`; a headless `egui::Context::run` of the browser panel with a two-study index emits `Action::SelectSeries` when the first card's response is clicked (drive it with `egui::Event::PointerButton` at the card's rect centre).
- [ ] **Step 2:** Run `cargo test -p voxa-app ui`. Expected: FAIL.
- [ ] **Step 3:** Implement `theme.rs`, `browser.rs`, `toolbar.rs`, `about.rs`, `viewport.rs` (drawing only; input comes in Task 10), `VoxaApp` and `run`.
- [ ] **Step 4:** Run the tests, then `cargo run -p voxa-app -- <a folder of testkit files written by a small example binary>` and confirm by eye: series listed, clicking one shows a grey ramp, layout buttons change the grid. Record what was seen in the commit body.
- [ ] **Step 5:** Commit: `feat(app): window, theme, study browser and viewport grid`.

---

### Task 10: Viewport interaction and overlays

**Files:** `crates/voxa-app/src/ui/{viewport,overlay}.rs`

**Interfaces:**
- Consumes: `Action` variants `Scroll`, `Zoom`, `Pan`, `Flip`, `Rotate`, `Fit`, `AdjustWindow`, `SetWindow`, `ApplyPreset`, `SetActive` (Task 8); `SeriesInfo::voxel_to_patient` (Task 2).
- Produces (pure functions in `overlay.rs`):

```rust
pub struct EdgeLabels { pub left: String, pub right: String, pub top: String, pub bottom: String }
pub fn orientation_labels(voxel_to_patient: &Affine<Voxel, Patient>, view: &ViewTransform, radiological: bool) -> EdgeLabels;
pub fn scale_bar(mm_per_screen_px: f32, max_px: f32) -> (f32 /* length px */, String /* "10 mm" */);
pub fn corner_text(study: &StudyInfo, series: &SeriesInfo, slice: usize, position_mm: Option<f64>, wl: WindowLevel, show_patient: bool) -> [Vec<String>; 4];
pub fn input_to_actions(viewport: usize, input: &ViewportInput) -> Vec<Action>;
```
Input mapping: wheel → `Scroll` (one slice per notch); Ctrl+wheel → `Zoom` about the cursor (factor 1.1 per notch); middle-drag → `Pan`; right-drag → `AdjustWindow` (horizontal changes width, vertical changes centre, each scaled by the current width / 256 per pixel); double-click → `Fit`; any press → `SetActive`. Keys: `H`/`V` flip, `R` rotate, `F` fit, `1`/`2`/`4` layout, `Space` cine, `?` shortcut reference. Orientation letters use the patient axis with the largest absolute component along each screen edge, in DICOM LPS: +x is L, +y is P, +z is S. In radiological convention the patient's left is on screen right; the toggle mirrors the image and the labels together. The scale bar picks the largest of 1, 2, 5, 10, 20, 50, 100 mm that fits `max_px`. Corner text never contains patient name or ID unless `show_patient` is true. Series without geometry show no letters and no scale bar, and the corner text says "No position data: orientation and scale are unavailable".

- [ ] **Step 1: Write failing tests**:
  - Axial identity orientation, radiological, no flips: `left == "R"`, `right == "L"`, `top == "A"`, `bottom == "P"`. Neurological: `left == "L"`, `right == "R"`.
  - **Review Focus 3:** orientation (1, 0, 0, 0, 0.98, 0.2): same four letters as the untilted case.
  - Sagittal (0, 1, 0, 0, 0, −1): `left == "A"`, `right == "P"`, `top == "S"`, `bottom == "I"`.
  - `flip_h` swaps left and right labels; one quarter turn rotates all four.
  - `scale_bar(0.5, 150.0) == (100.0, "50 mm")`; `scale_bar(3.0, 150.0)` has label `"100 mm"` and length within 0.01 of 33.33; for 20 sampled scales the length never exceeds `max_px` and the label is one of the seven allowed values.
  - `corner_text(.., show_patient = false)` contains neither the testkit patient name nor its ID in any corner; with `true` it contains both.
  - `input_to_actions`: one wheel notch down → `[Scroll { delta: 1 }]`; right-drag by (+10, 0) at width 256 → `AdjustWindow { d_width: 10.0, d_center: 0.0 }`.
- [ ] **Step 2:** Run `cargo test -p voxa-app overlay`. Expected: FAIL.
- [ ] **Step 3:** Implement the pure functions, then wire them into `viewport.rs`; add the window/level numeric fields and preset buttons to the toolbar, the convention and patient toggles with visible labels, and a shortcut reference window opened by `?` that lists every key above.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): viewport interaction, orientation labels, scale bar and overlays`.

---

### Task 11: Tag browser and cine

**Files:** `crates/voxa-app/src/ui/tags.rs`, `ui/toolbar.rs`, `reducer.rs`

**Interfaces:**
- Consumes: `list_tags`, `TagRow`, `is_patient_identifying` (Task 5); `Action::{SetTagFilter, CineToggle, CineSet, CineTick}` (Task 8).
- Produces: `pub fn filter_tags<'a>(rows: &'a [TagRow], query: &str, show_patient: bool) -> Vec<&'a TagRow>` (case-insensitive match on tag, keyword or value; patient-identifying rows show `"hidden"` as their value unless `show_patient`); `pub fn cine_due(last_tick: Instant, now: Instant, fps: f32) -> u32` (number of ticks owed).

- [ ] **Step 1: Write failing tests**: `filter_tags(rows, "modal", false)` returns only the Modality row; with `show_patient == false` the PatientName row's displayed value is `"hidden"` and searching for the real name finds nothing; `cine_due` at 10 fps over 250 ms returns 2; over 0 ms returns 0; fps outside 1..=60 is clamped.
- [ ] **Step 2:** Run `cargo test -p voxa-app tags cine`. Expected: FAIL.
- [ ] **Step 3:** Implement; the tag window loads rows on a worker when the active slice changes; cine requests a repaint at the next due time instead of continuously.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): DICOM tag browser and cine playback`.

---

### Task 12: Opening scans — drag and drop, dialog, command line, notices

**Files:** `crates/voxa-app/src/main.rs`, `ui/mod.rs`, `ui/toolbar.rs`

**Interfaces:**
- Consumes: `Action::OpenPath`, `Effect::StartIndex`, `Notice` (Task 8); `open_source`, `index` (Tasks 3, 4).
- Produces: `pub fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Option<PathBuf>, String>` (zero or one path; `--version` and `--help` handled; more than one path is an error message naming the extras).

Dropping a file or folder on the window, choosing one through the `rfd` dialog, or passing a path on the command line all dispatch `OpenPath`. Indexing runs on a worker and the browser fills as `IndexProgress` actions arrive. Failures become notices in plain words: "That file is not a zip or DICOM export", "No images were found in …", "n files skipped" with a details list. Opening a second source replaces the first after its workers are cancelled.

- [ ] **Step 1: Write failing tests**: `parse_args` for no arguments, one path containing spaces and non-ASCII characters, `--version`, and two paths; an end-to-end smoke test without a window that dispatches `OpenPath` for a testkit zip, pumps the worker channel until the index holds one series and the middle frame is in the cache, and asserts no notice of kind `Error`; opening a text file yields the notice "That file is not a zip or DICOM export"; opening an empty folder yields "No images were found in" followed by the folder name.
- [ ] **Step 2:** Run `cargo test -p voxa-app open`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): open scans by drop, dialog or command line with plain-language notices`.

---

### Task 13: Benchmarks, acceptance and release 0.1.0

**Files:** `crates/voxa-app/benches/decode.rs`, `CHANGELOG.md`, `README.md`, `.github/workflows/ci.yml`, `docs/acceptance-0.1.0.md`

- [ ] **Step 1:** Add `criterion` benches: index a 200-slice testkit zip; decode one 512×512 16-bit frame; `auto_window` on 512×512 values. Run `cargo bench -p voxa-app` and record the numbers in `docs/acceptance-0.1.0.md`.
- [ ] **Step 2:** Add a CI job that builds `voxa-app` in release mode on Windows and keeps the executable as a build artefact.
- [ ] **Step 3:** Run `cargo xtask check` and `cargo xtask coverage`. Expected: all pass; coverage at or above 80% on the three non-GPU crates. Fix anything that fails before continuing.
- [ ] **Step 4: Manual acceptance on a real export** (local only, never committed): open the export, as a zip file, by drag and drop. Record in `docs/acceptance-0.1.0.md`, without any patient details: time from drop to the browser listing (target under 2 s); that every study and series is listed; that every slice of every series displays; frame rate while scrolling a 64-slice series (target 60 fps); and that the left/right labels match the vendor viewer for one axial slice. A missed target is recorded as a known issue with its measured value, not hidden.
- [ ] **Step 5:** Update `CHANGELOG.md` with the 0.1.0 entry and `README.md` with a screenshot of a testkit series (never patient data).
- [ ] **Step 6:** Commit: `chore: benchmarks, acceptance record and 0.1.0 changelog`, then `git tag v0.1.0`. Do not push.

---

## Out of scope (spec B3)

Reslicing and three-plane views, measurements, 3D, filters, registration, regions, segmentation, any network access, editing or anonymising DICOM, colour and ultrasound cine, macOS packaging, undo and redo.
