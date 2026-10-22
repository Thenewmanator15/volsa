# Any MRI export: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Any ordinary MRI export opens cleanly in Voxa, each series says what it is, and the everyday gaps the review found are closed.

**Architecture:** The index (`voxa-dicom`) learns more from headers: what kind of scan a series is, and where one series holds several volumes, which it lists as volumes of a set. The model (`voxa-core`) carries that as plain data. The application reads it: the browser describes series, the viewport steps between volumes by switching to the sibling series, and colour frames take a second, simple drawing path. Logging, remembered preferences and the Windows program settings are small modules of `voxa-app`.

**Tech Stack:** As before (Rust 1.99, egui/eframe 0.36, wgpu 30, dicom-rs 0.10). New: `tracing-subscriber` (log file), eframe's `accesskit` feature, the `deflate` feature of the DICOM transfer-syntax registry.

**Spec:** `docs/superpowers/specs/2026-10-13-voxa-any-mri-design.md` (requirement numbers below refer to it)

## Global Constraints

- The disclaimer verbatim: "Voxa is not a medical device and is not for diagnostic use."
- No network, no telemetry. No `unsafe`. Library crates never panic.
- Nothing from a real export in a test, a document or a commit message. Made-up values only, checked before each commit against a real export on the developer's machine, from which nothing is recorded.
- Every dependency carries a licence on the allow-list in `deny.toml`.
- `cargo xtask check` passes before every commit. Files under 400 lines, functions under 60.
- The version number does not change. Nothing is merged into `main` or pushed without the owner.
- Test names for the shared made-up series are `THICK_SCAN` and `FINE_SCAN`.

## Review Focus

1. A series whose repeats are almost a grid (one image missing from one volume): it stays a plain stack with a reason, and no image is put in the wrong volume. Test in Task 3.
2. A series of hundreds of volumes: one card in the browser, and the index stays quick. Test in Task 4.
3. A colour file that is 16-bit, or palette colour, or shorter than it claims: refused in a sentence, never drawn as noise. Test in Task 5.
4. The log never holds a patient's name, identifier or date, nor a path: a test indexes a made-up export with distinctive values of each and searches the log for them. Test in Task 9.
5. A preferences file that is damaged, or a folder that cannot be written: defaults, no crash; a run that takes a picture reads and writes nothing. Test in Task 10.

---

### Task 1: Kinds of series, and what the card says

**Files:**
- Create: `crates/voxa-core/src/kind.rs`
- Modify: `crates/voxa-core/src/model.rs`, `crates/voxa-core/src/lib.rs`, `crates/voxa-app/src/ui/browser.rs` (and its tests), every place that builds a `SeriesInfo`

**Interfaces:**
- Produces: `SeriesInfo` gains `scan: ScanFacts`, `set: Option<VolumeSet>`, `colour: bool`, and `impl Default for SeriesInfo` (empty identifier, one slice, 1 mm spacing) so that later tasks add no more fields to the literals. `ScanFacts` and `VolumeSet` are defined here as plain data with `Default`; Tasks 2 and 3 fill them.
  - `pub struct VolumeSet { pub of: SeriesId, pub at: usize, pub count: usize, pub what: String }`
  - `pub struct ScanFacts { pub plane: Option<Plane>, pub in_3d: Option<bool>, pub family: Vec<Family>, pub repetition_ms: Option<f64>, pub echo_ms: Option<f64>, pub inversion_ms: Option<f64>, pub flip_deg: Option<f64>, pub field_t: Option<f64>, pub body_part: String }`
  - `pub enum SeriesKind { Volume, Locating, Single, Colour, Stack(SeriesProblem), Unreadable(String) }`, `SeriesInfo::kind(&self) -> SeriesKind`, `SeriesKind::says(&self) -> Option<&'static str>` (the neutral words, `None` for `Volume`), `SeriesKind::is_fault(&self) -> bool` (true for `Stack` and `Unreadable`).
  - `Locating` is: the only problems are `MixedOrientations` and, optionally, `MixedSizes`, and there are at most 12 slices.
  - `ui::browser::note(info: &SeriesInfo) -> Option<(String, bool)>`: the card's third line and whether it is a warning. `ui::browser::hover(info: &SeriesInfo) -> String`: the full description, then the caption, then each problem's sentence.

- [ ] **Step 1: failing tests.** `kind.rs`: a series with no problems is `Volume`; three slices with `MixedOrientations` is `Locating` and says "locating images in several planes"; thirteen such slices is `Stack(MixedOrientations)`; one slice is `Single` ("a single image"); `colour: true` is `Colour` ("colour pictures") whatever else; `UnevenSpacing` is `Stack` and a fault; `UnsupportedTransferSyntax("x")` is `Unreadable("x")` and comes before any other problem. `browser/tests.rs`: `note` of a locating series is `("locating images in several planes", false)`; of an uneven stack is `("! The slices are not evenly spaced.", true)`; of a volume `None`. `hover` of a series with a 60-character description holds all of it.
- [ ] **Step 2:** run them; expected: do not compile.
- [ ] **Step 3:** implement; replace the card's "! this series has a problem" with `note`, drawn in the quiet colour unless it is a warning; add `on_hover_text(hover(info))` to the card.
- [ ] **Step 4:** `cargo test -p voxa-core -p voxa-app`; expected: pass.
- [ ] **Step 5:** gate, commit `feat(core): a series says what kind it is, and its card no longer calls a normal one a problem`.

### Task 2: What kind of scan

**Files:**
- Create: `crates/voxa-core/src/scan.rs`, `crates/voxa-dicom/src/header/scan.rs`
- Modify: `crates/voxa-dicom/src/header.rs`, `crates/voxa-dicom/src/index.rs`, `crates/voxa-dicom/src/testkit.rs` and `testkit/build.rs`, `crates/voxa-app/src/ui/browser.rs`

**Interfaces:**
- Consumes: `ScanFacts` from Task 1.
- Produces: `pub enum Plane { Axial, Coronal, Sagittal, Oblique(Nearest) }` with `Plane::of(orientation: [f64; 6]) -> Option<Plane>` (oblique when the normal is more than 15 degrees from every axis); `pub enum Family { InversionRecovery, EchoPlanar, SpinEcho, GradientEcho }`; `ScanFacts::line(&self) -> Option<String>` ("Axial · 3D · gradient echo") and `ScanFacts::lines(&self) -> Vec<String>` (one fact a line, with units). `Header` gains `scan: ScanFacts` read by `header::scan::facts(obj) -> ScanFacts` from `ScanningSequence`, `MRAcquisitionType`, `RepetitionTime`, `EchoTime`, `InversionTime`, `FlipAngle`, `MagneticFieldStrength`, `BodyPartExamined`, and for many-frame files `EchoPulseSequence`, `InversionRecovery`, `EchoPlanarPulseSequence`, `VolumetricProperties`/`MRAcquisitionType` and the shared timing groups. `SliceOptions` gains `scan: Vec<(Tag, VR, String)>`-style extra elements through one field, `extra: Vec<Extra>`.

- [ ] **Step 1: failing tests.** `scan.rs`: `Plane::of([1,0,0,0,1,0])` is `Axial`, `[1,0,0,0,0,-1]` `Coronal`, `[0,1,0,0,0,-1]` `Sagittal`, an axial plane tilted 20 degrees about the left-right axis `Oblique(Axial)`, 10 degrees `Axial`, a degenerate orientation `None`. `line` of plane axial, 3D, gradient echo is "Axial · 3D · gradient echo"; with inversion recovery and spin echo "inversion-recovery spin echo"; of empty facts `None`. `lines` gives "Repetition time 2000 ms", "Echo time 90 ms", "Flip angle 150°", "Field strength 3 T", "Body part: KNEE" for those values and leaves out what is absent. `header` tests: a made-up classic slice with `ScanningSequence` `IR\SE`, `MRAcquisitionType` `2D`, times and field gives those facts; a slice with none of them gives the default. `index` test: the series' `info.scan` is its first image's, with the plane from its orientation.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement. The card draws `scan.line()` as a line of its own in the quiet colour when there is one; `hover` adds `scan.lines()`.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5:** gate, commit `feat(dicom): what kind of scan a series is, from its own tags`.

### Task 3: Several volumes in one series (the index)

**Files:**
- Create: `crates/voxa-dicom/src/index/volumes.rs`, `crates/voxa-dicom/src/index/volumes_tests.rs`
- Modify: `crates/voxa-dicom/src/index.rs`, `crates/voxa-dicom/src/header.rs` (per-image `Told`), `crates/voxa-dicom/src/testkit.rs`

**Interfaces:**
- Consumes: `VolumeSet` from Task 1.
- Produces: `Header.told: Vec<Told>` beside `positions`, one for each frame: `pub(crate) struct Told { temporal: Option<i32>, b_value: Option<f64>, echo_ms: Option<f64>, trigger_ms: Option<f64>, acquisition: Option<i32> }`. `volumes::split(slices: &[Slice], gaps_mm: &[f64]) -> Option<Vec<Vec<usize>>>`: for slices in stack order with the gaps between neighbours, the indices of each volume's slices in order, or `None` when the positions are not each filled the same number of times (at least twice). `volumes::what(volumes: &[Vec<&Told>]) -> Vec<String>`: a name for each ("b 1000", "echo 30 ms", "time 3", "trigger 250 ms", "acquisition 2"), or empty strings. In the index a set of `n` volumes becomes `n` `IndexedSeries` with identifiers `"{id}@{k}"` (k from 1), each with `info.set = Some(VolumeSet { of: SeriesId(id), at: k - 1, count: n, what })`, each run through the same checks as any series. `best_series` breaks a tie between volumes of one set towards the lower `at`.

- [ ] **Step 1: failing tests** (`volumes_tests.rs`, made-up files): 3 positions × 2 volumes told apart by `EchoTime` 30 and 90, shuffled → two series `…@1` and `…@2`, 3 slices each, no problems, `what` "echo 30 ms" and "echo 90 ms", each with a transform to the patient; the same told apart only by instance number → `what` empty, order by instance; b-values 0 and 1000 → "b 0", "b 1000"; temporal positions win over echo times when both differ; a many-frame file with per-frame b-values splits the same way; **Review Focus 1:** 3 positions × 2 with one image missing → one series, `RepeatedPositions`, 5 slices; 2 volumes where one position holds three images → one series; a single position repeated 4 times → 4 volumes of one slice each (each `Single`); `best_series` among two equal volumes returns `at == 0`.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement. `series()` in `index.rs` returns a `Vec<IndexedSeries>`; the splitting and naming live in `volumes.rs`.
- [ ] **Step 4:** `cargo test -p voxa-dicom`; expected: pass, with the existing "repeated positions" test still describing a series that is not a grid.
- [ ] **Step 5:** gate, commit `feat(dicom): a series that holds several volumes is listed as its volumes`.

### Task 4: Stepping between volumes (the application)

**Files:**
- Create: `crates/voxa-app/src/reducer/volumes.rs`, `crates/voxa-app/src/reducer/volumes_tests.rs`, `crates/voxa-app/src/ui/volumes.rs`
- Modify: `crates/voxa-app/src/action.rs`, `reducer.rs`, `ui/browser.rs`, `ui/overlay.rs` (corner text), `ui/toolbar.rs`, `ui/mod.rs` (keys), `ui/help.rs`, `testing.rs`

**Interfaces:**
- Consumes: `VolumeSet`.
- Produces: `Action::StepVolume { viewport: usize, by: i32 }`. The reducer finds the sibling (`set.of` equal, `at` moved by `by`, held within the set), puts it in the viewport keeping `slice` (held within its slices), `view` and, when `wl_auto` is false, `wl`; when `wl_auto` is true the window is chosen again. Effects as `SelectSeries` gives for the frames and, when the viewport is the active one, the scene's volume; the scene's camera is not refitted. `testing::two_echoes_files() -> Vec<(String, Vec<u8>)>` and `TWO_ECHOES: &str` (the set's identifier). `ui::browser`: one card for a set, standing for the volume shown in the active viewport or else the first, its caption "2 volumes · 3 slices · 5 mm". `ui::volumes::label(info) -> Option<String>`: "Volume 2 of 5 · b 1000". Keys `[` and `]`.

- [ ] **Step 1: failing tests.** Reducer: stepping by 1 from the first volume shows the second with the same slice and view; by -1 from the first stays; by 1 from the last stays; a hand-set window is kept and an automatic one is marked pending; the effects ask for the second volume's frames; with the scene showing the first volume, stepping asks for the second's volume and the camera is unchanged; `StepVolume` on a series that is not in a set does nothing. Browser: an index with a set of 2 and one plain series gives 2 cards; **Review Focus 2:** a set of 300 volumes gives one card, and `cards` for it is computed without visiting more than the set's members once (assert on the count returned, and that indexing 300 × 2 made-up slices takes under a second in a release test is not asserted: assert only the card count). Corner text holds "Volume 2 of 2 · echo 90 ms".
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement; the toolbar shows "◀ Volume 2 of 5 ▶" for the active viewport when its series is in a set; the keys window lists the two keys.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5:** gate, commit `feat(app): step between the volumes of a series, in 2D and in 3D`.

### Task 5: Colour pictures

**Files:**
- Create: `crates/voxa-dicom/src/decode/colour.rs`, `crates/voxa-app/src/ui/viewport/colour.rs`
- Modify: `crates/voxa-dicom/src/decode.rs`, `header.rs` (colour flag), `index.rs`, `testkit.rs`/`build.rs` (a colour slice), `crates/voxa-app/src/cache.rs`, `ui/browser.rs` (thumbnail), `ui/viewport.rs`, `ui/toolbar.rs`, `reducer` (no window, no scene for a colour series), `crates/voxa-volume/src/assemble.rs` (`can_be_volume`)

**Interfaces:**
- Produces: `Frame` gains `pub colour: Option<Vec<[u8; 3]>>` (`rows × cols` pixels, row by row); for a colour frame `values` holds each pixel's brightness (0.299 R + 0.587 G + 0.114 B) so that everything that reads values still has something sensible. `SeriesInfo.colour` is true when the first image has three samples a pixel. `VolumeError::Colour` ("Colour pictures are shown as a plain stack.") from `can_be_volume`.

- [ ] **Step 1: failing tests.** Decode: a made-up 2 × 2 RGB slice (8-bit, interleaved) decodes to those four colours and to their brightness as values; planar configuration 1 the same; **Review Focus 3:** a 16-bit RGB slice is `VoxaError::Decode` naming 16-bit colour; `PALETTE COLOR` is `Decode` naming palette colour; a colour slice cut short is `Decode`. Index: the series is `colour` and its kind `Colour`. Volume: `can_be_volume` gives `Colour`. App: `thumbnail` of a colour frame keeps the colours; the viewport's draw for a colour frame is a textured mesh whose four corners are where the grey image's would be under the same view (assert the corners for a flipped and a turned view); `SetWindow` and the window presets do nothing for a colour series; the toolbar's window boxes are disabled; the scene says why there is no 3D view.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement. The colour texture is an `egui` texture made from the frame and painted with `egui::Mesh`.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5:** gate, commit `feat: colour pictures are shown, in their own colours`.

### Task 6: A steadier automatic window

**Files:**
- Modify: `crates/voxa-volume/src/auto_window.rs`, `crates/voxa-volume/src/histogram.rs`

**Interfaces:**
- Produces: `Preset::window` unchanged in signature. Percentiles are read from a histogram of the values; when the window found is narrower than 16 of its bins, the histogram is made again over that window widened by one bin each side, up to three times.

- [ ] **Step 1: failing test.** A ramp 0 to 999 with one value of 1e9 added: the automatic window's low is within 2 of 10 and its high within 2 of 995 (it is 1 wide today). A ramp with one value of -1e9: the same. An image of all one value: width 1, as before. The existing ramp test unchanged.
- [ ] **Step 2:** run; expected: the first two fail on the width.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** `cargo test -p voxa-volume`; pass.
- [ ] **Step 5:** gate, commit `fix(volume): the automatic window holds when a few values lie far outside the rest`.

### Task 7: A many-frame file is parsed once

**Files:**
- Create: `crates/voxa-app/src/worker/opened.rs`
- Modify: `crates/voxa-app/src/worker.rs`, `crates/voxa-app/src/worker/tests.rs`

**Interfaces:**
- Produces: `Opened`, a holder of the one many-frame file opened last (`Arc<ImageFile>` under a mutex, with its entry's name and the index's generation): `Opened::get(&self, entry, open: impl FnOnce() -> Result<ImageFile, VoxaError>) -> Result<Arc<ImageFile>, VoxaError>`. The frame worker uses it when the slice asked for has a frame number; single-slice files are opened as now. A new export empties it.

- [ ] **Step 1: failing test.** A source that counts reads of each entry: asking for 8 frames of one many-frame file reads the entry once (it reads it 8 times today); asking for frames of a second file drops the first; opening another export drops what was held; a file that fails to open is not held, and is tried again when asked for again.
- [ ] **Step 2:** run; expected: 8 reads.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5:** gate, commit `perf(app): a file of many frames is read and parsed once, not once a frame`.

### Task 8: Deflated files, and tests of JPEG and JPEG 2000

**Files:**
- Create: `crates/voxa-dicom/src/decode/pictures_tests.rs`, `crates/voxa-dicom/tests/fixtures/ramp.j2k`, `tools/fixtures/make_ramp_j2k.py`
- Modify: `Cargo.toml` (the registry's `deflate` feature; `jpeg-encoder` as a development dependency), `crates/voxa-dicom/Cargo.toml`, `crates/voxa-dicom/src/testkit/build.rs` (encapsulated pixel data), `docs/architecture.md`

**Interfaces:**
- Produces: `testkit::encapsulated(options: &SliceOptions, syntax: &str, fragment: Vec<u8>) -> Vec<u8>`.

- [ ] **Step 1: failing tests.** A made-up 16 × 16 8-bit picture encoded as baseline JPEG in the test, wrapped as a DICOM file: decodes to values within 6 of the picture's. The fixture `ramp.j2k` (a 16 × 16 16-bit ramp, lossless, written by the script from made-up numbers) wrapped as JPEG 2000 lossless: decodes to the ramp exactly. A made-up slice written as deflated explicit little endian (the data set after the file header run through `flate2`): decodes to its values (it is "cannot read" today).
- [ ] **Step 2:** run; expected: the deflated one fails as unsupported; the other two fail to compile until the helper exists.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** `cargo test -p voxa-dicom`; pass. `cargo deny check`; pass.
- [ ] **Step 5:** gate, commit `feat(dicom): deflated files are read; JPEG and JPEG 2000 decoding are tested`.

### Task 9: A log file

**Files:**
- Create: `crates/voxa-app/src/log.rs`, `crates/voxa-app/src/log/tests.rs`
- Modify: `Cargo.toml` (`tracing-subscriber`, features `fmt` and `std` only), `crates/voxa-app/Cargo.toml`, `crates/voxa-app/src/lib.rs` (`run`), `reducer.rs` (one place that posts notices), `ui/about.rs`, `deny.toml` if a new licence appears

**Interfaces:**
- Produces: `log::FILE: &str = "voxa.log"`, `log::MOST_BYTES: u64 = 1 << 20`, `log::folder() -> Option<PathBuf>` (the system's local data folder for Voxa), `log::start(folder: Option<&Path>) -> Option<PathBuf>` (cuts the file back when it is over the limit, installs the writer and a panic hook, returns the file's path), `log::line_for(notice: &Notice) -> String` (the notice's kind and text, never its details), `log::opened(index: &Index)` (counts only). The About window shows the path with a "Copy" button.

- [ ] **Step 1: failing tests.** **Review Focus 4:** a made-up export whose patient name, identifier, study date, descriptions and folder path are distinctive strings is indexed and reported with `log::opened`, and a failed-open notice for a path holding the patient's name is logged with `line_for`: the lines written hold none of those strings, and do hold "2 studies" and the notice's own words. A file of 2 MB is cut back to under 1 MB keeping its last lines whole. `start` with no folder returns `None` and nothing panics. The lines of two starts are both in the file.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement. Logged at start: version, operating system, graphics adapter and backend.
- [ ] **Step 4:** tests pass; `cargo deny check` passes.
- [ ] **Step 5:** gate, commit `feat(app): a log file that never names a patient`.

### Task 10: What is remembered between runs

**Files:**
- Create: `crates/voxa-app/src/prefs.rs`, `crates/voxa-app/src/prefs_tests.rs`
- Modify: `crates/voxa-app/src/lib.rs` (`run`: window size), `action.rs`, `effect.rs`, `reducer.rs`, `worker.rs` (the write), `ui/capture.rs` (no folder for a picture)

**Interfaces:**
- Produces: `pub struct Prefs { pub window: Option<[f32; 2]>, pub layout: Layout, pub radiological: bool, pub linked: bool, pub overlay: Overlay }` with `Default` as the state starts today; `prefs::load(folder) -> Prefs`; `prefs::save(folder, &Prefs) -> io::Result<()>`; `Prefs::of(state: &AppState, window: Option<[f32; 2]>) -> Prefs`; `Prefs::put(&self, state: &mut AppState)`. `Effect::StorePrefs(Prefs)` is given by the reducer when an action changed any of them; the window's size is added by the interface when it has changed and the pointer is up.

- [ ] **Step 1: failing tests.** Round trip through a temporary folder. **Review Focus 5:** a file of nonsense loads as the default; a folder that is a file makes `save` an error and the worker posts no notice more than once; with a capture asked for, `run`'s settings folder is `None` and no file appears. Reducer: `SetLayout`, the convention switch, the link switch and the overlay choice each give `StorePrefs` with the new value; an action that changes none gives no such effect. `put` then `of` gives back what was put. A window size below the minimum is not taken.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** tests pass.
- [ ] **Step 5:** gate, commit `feat(app): the window's size, the layout and three switches are remembered`.

### Task 11: The program on Windows: no console, a message window, an icon

**Files:**
- Create: `crates/voxa-app/src/ui/icon.rs`, `crates/voxa-app/src/said.rs`
- Modify: `crates/voxa-app/src/main.rs`, `crates/voxa-app/src/lib.rs`

**Interfaces:**
- Produces: `said::Outcome { text: String, failed: bool, code: u8 }`, `said::outcome(command: Result<Command, String>) -> Option<Outcome>` (what to tell the person in place of running: the help, the version, or a mistake with the help after it), `said::tell(outcome: &Outcome, windowed: bool)` (a message window when `windowed`, else standard output or error). `main.rs` carries `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]` and passes `windowed = cfg!(all(windows, not(debug_assertions)))`. `ui::icon::icon() -> egui::IconData`: 64 × 64, a pale ring with a bead, on nothing.

- [ ] **Step 1: failing tests.** `outcome` of `--help` is the usage, not failed, code 0; of `--version` the version; of a mistake the mistake, a blank line and the usage, failed, code 2; of a run `None`. The icon is 64 × 64 × 4 bytes, clear at its corners, not clear at the ring.
- [ ] **Step 2:** run; expected: do not compile.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** tests pass. Build the release program and read the subsystem field of its header with a script: expected 2 (windowed), where it is 3 (console) today.
- [ ] **Step 5:** gate, commit `feat(app): on Windows no console opens beside Voxa, and the window has an icon`.

### Task 12: Screen readers

**Files:**
- Modify: `crates/voxa-app/Cargo.toml` (eframe's `accesskit` feature), `deny.toml` if a new licence appears, `README.md`

- [ ] **Step 1:** add the feature; `cargo build -p voxa-app`; `cargo deny check`.
- [ ] **Step 2:** a test that runs one frame of the interface without a window with AccessKit asked for, and finds a node named "Folder…" among what is reported.
- [ ] **Step 3:** gate, commit `feat(app): the interface is described to screen readers`.

### Task 13: Tests that wait on time

**Files:**
- Modify: whichever tests wait a fixed time for another thread (found by searching for `drain(`, `wait(Duration`, `sleep(` and `recv_timeout(` under `crates/*/src` and `crates/*/tests`)

- [ ] **Step 1:** list them in the ledger, each as "waits for an event" or "waits for time"; a wait for time that is there to show that nothing happens is kept and said to be so.
- [ ] **Step 2:** change each wait for time into a wait for the event; run the changed tests 20 times each under load (`cargo test` of the whole workspace running beside them).
- [ ] **Step 3:** gate, commit `test: wait for what another thread does, not for a length of time`.

### Task 14: The made-up study, the documents, the record

**Files:**
- Modify: `crates/voxa-app/examples/write_phantom/main.rs` (and a new `more.rs`), `crates/voxa-app/tests/phantom*.rs`, `README.md`, `CHANGELOG.md`, `docs/architecture.md`, `CONTRIBUTING.md`
- Create: `docs/adr/0005-volumes-of-a-series.md`, `docs/acceptance-any-mri.md`

- [ ] **Step 1: failing test.** The made-up study's folder indexes to: its two ball series as before, a set of two volumes ("echo 20 ms", "echo 80 ms") of the ball, three locating images in three planes, and one colour picture; nothing has a fault.
- [ ] **Step 2:** extend the example; the test passes.
- [ ] **Step 3:** pictures of the window on the made-up study (the browser with the new cards; a volume stepped; the colour picture), looked at; one in the README.
- [ ] **Step 4:** the documents. `CONTRIBUTING.md` no longer says the repository is local.
- [ ] **Step 5:** the record: gates, what was looked at, a real export checked locally with nothing from it recorded, what was not checked.
- [ ] **Step 6:** gate, coverage, commit `chore: the made-up study shows every kind of series; documents and record`.
