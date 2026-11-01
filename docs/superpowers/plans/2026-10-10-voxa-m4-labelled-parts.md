# Voxa Milestone 4 (Labelled parts, from files) Implementation Plan

> Volsa was earlier called Voxa. This record keeps the name it was written with.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person opens a file of labels made elsewhere for the scan they are looking at, and sees the named parts of the head: outlined over the scan in the three planes, as coloured solids in 3D, and as a list they can search, show, hide, fade and select, with the volume of each part and how far each name can be trusted.

**Architecture:** A new crate, `voxa-labels`, reads a NIfTI label volume and a table of names, places the labels in the patient by the file's own geometry, and works out each part's volume. The labels keep their own grid: nothing is resampled. `voxa-render` puts them on the graphics device once, as a texture of part numbers and a small table of colours, and two shaders read them: the plane shader, which tints and outlines, and a new one that follows a line of sight to the first part shown. The app holds what is shown, faded and selected in `AppState`; changing those rewrites the small table only.

**Tech Stack:** As milestone 3, plus `flate2` (already in the tree through `zip`) to read `.nii.gz`. NIfTI is read by Voxa's own code: a header of 348 bytes.

**Spec:** `docs/superpowers/specs/2026-10-05-voxa-design.md`: the milestone 4 row of A4 from "Label volumes rendered as coloured solids" on; "Hover and select" and "Show, hide, fade" in A8; in A9 the whole-head parts list, "Check on scan", the trust marker per part, and the left against right table.

**Branch:** `m4-labelled-parts`, cut from `main` at `v0.3.0`.

## Scope

This is the half of the spec's milestone 4 that shows labels. The other half, running a labelling model inside Voxa (an ONNX runtime, converting SynthSeg, downloading weights whose licence is unsettled), is a milestone of its own and is not started here. Label files are already made by programs outside Voxa; showing labelled parts in Voxa was asked for, and was told twice that the quick route is to open those files.

In: NIfTI-1 label volumes (`.nii`, `.nii.gz`); three kinds of table of names; one label set at a time, belonging to the study it was opened on; the Parts list; the overlay in the plane views; the Parts look in 3D with hover, click, fade and cuts; volumes, left against right, saving the list as CSV; a made-up labelled study for pictures; a check that runs on a whole export.

Out, with where it goes: running models (milestone 4, second half); more than one label set at once, so both ears together or labels of two routes side by side (milestone 6 in the spec; one ear at a time works now); labels drawn in the 2D stack viewports; painting and hand corrections (milestone 8); FreeSurfer `.mgz`, NRRD and DICOM SEG (milestone 8); surfaces as meshes and STL (milestone 8); remembering between sessions which labels were open.

## Conventions

The owner's standing instruction, to keep it standardised, is this plan's reading too: where medical imaging has a convention Voxa follows it.

- **Files.** A label volume is a single-file NIfTI-1 (`n+1`), plain or gzipped, whole numbers, at most three dimensions that are not 1. NIfTI's world is right-anterior-superior; Voxa's patient space is DICOM's left-posterior-superior, so x and y change sign and nothing else does. The `sform` places the voxels if its code is above 0, otherwise the `qform`; a file with neither is refused, since labels that cannot be placed would be drawn in the wrong place.
- **Tables.** Three are read: the pipeline's `regions` JSON (names with a group, a colour, the numbers for left and right or for the midline, whether approximate and how made, and the labelling model's own check); a plain JSON of `parts` and `colours` by number; and a 3D Slicer colour table (`.ctbl`: number, name, red, green, blue, alpha). A number in the volume that the table does not name becomes a part called "Label 17" in the group "Not named in the table": nothing in the file is dropped silently. With no table every part is named that way.
- **Finding the table.** Beside `x.nii.gz`: `x.json`, then `x.ctbl`; for `labels<rest>.nii.gz` also `regions<rest>.json`; then `regions.json`, then `labels.ctbl`. The Parts list has a "Names…" button to choose another.
- **A part** is one named structure on one side: "hippocampus", left. Parts are numbered from 1 in the table's order; 0 is nothing. A name with a left and a right is two parts, shown as one row with two sides.
- **One label set at a time.** It belongs to the study of the series in the scene when it was opened, on the same assumption as Link: within a study the patient did not move. It is shown on any series of that study and on no other. Opening another export closes it.
- **Trust** is said in words on every part: "model" (a trained model made it), "approximate" with how it was made, or "not named". The model's own check, when the table has one, is said in a sentence at the top of the list, and a failed one is said first.
- **Volumes** are voxels counted times the voxel's size, of the whole part whether shown or hidden: millilitres to one decimal, cubic millimetres below 1 ml. Left against right says the difference as a percentage of their mean, and says that Voxa cannot tell how much of a difference is noise.
- **Overlay** in the plane views: Off, Outline (the default), or Filled (outline and a tint of a third). **Parts** is a fourth look of the 3D scene beside Surface, VR and MIP: the parts shown, as lit solids in their colours.
- **One click means one point.** A click in a plane view or on a solid moves the position there, as in milestone 3, and selects the part there; a click on nothing clears the selection. Hovering says the part's name. A hidden part is not there: not drawn, not outlined, not named, not picked.
- **Colours** come from the table. Parts not named get hues spaced by the golden angle, the same every time.
- Limits: 65 535 different numbers, 256 million voxels (512 MiB as held), names cut at 80 characters.

## Global Constraints

Every constraint of the earlier plans still holds: the disclaimer verbatim; dual MIT / Apache-2.0; no network, no telemetry; patient data never in the repository; nothing from a real export in a test, a document or a commit message, and that includes label files made from a real export, their names, titles and volumes; the repository stays local until the owner says so; no `unsafe` outside `voxa-render`; library crates never panic; `cargo xtask check` passes at every commit; every public item documented; conventional commits; crates depend only downward (core; then dicom, volume and labels; then render; then app); `Patient`, `Voxel` and `Screen` stay distinct types; `voxa-app` contains no image mathematics; tests that need a graphics device return early when there is none; a background task that panics must not stop the program; every sentence shown is plain words and says what the picture cannot show. In addition:

- `voxa-labels` joins the crates held to 80% line coverage.
- Nothing a label file or a table contains may stop the program: every way they can be wrong is an error in plain words.
- No path or file name of a label file is written to a log or to a file Voxa keeps.
- Whatever a label is drawn or listed with, Voxa did not make it. The list says where its names came from, and the caption of the Parts look says the solids are labels made elsewhere, not something Voxa found in the scan.

## Review Focus

1. **Left and right.** Labels mirrored left to right look right and are wrong. The sign change from NIfTI's world, files stored in either voxel order, `qform` against `sform`, a big-endian file. Tasks 1, 4 and 5.
2. **A grid of their own.** Labels coarser or finer than the scan, tilted against it, covering part of it, or nowhere near it (labels of another scan: said so, not drawn in a corner). Tasks 1, 3, 4 and 7.
3. **Files that are wrong.** A gzip cut short, a NIfTI-2 or two-file NIfTI, numbers that are not whole, a header that claims more voxels than the file has or than Voxa will hold, a table that is not JSON, names for numbers that are not there. Tasks 1, 2 and 8.
4. **Nothing to show them on.** No volume yet, another study chosen, another export opened while labels are being read: the labels of one study never appear on another, and the Parts look falls back to Surface when there is nothing for it to draw. Tasks 7, 8 and 11.
5. **Hidden means hidden.** A hidden part is not drawn, outlined, named on hover, picked by a click or left selected; its volume is still its volume. Tasks 3, 4, 5, 7 and 10.

---

### Task 1: Reading a NIfTI label volume

**Files:** `crates/voxa-labels/{Cargo.toml,src/lib.rs,src/nifti.rs,src/nifti/tests.rs,src/testkit.rs}`, root `Cargo.toml`, `xtask/src/main.rs`

**Interfaces:**
- Produces:

```rust
pub struct LabelGrid { pub dims: [u32; 3], pub ids: Vec<u32>, pub placement: Affine<Voxel, Patient> }   // columns fastest, as stored
pub enum NiftiError { NotNifti, Nifti2, TwoFiles, NotThreeD, Kind(i16), Scaled, NotWhole, NoPlace, TooLarge, CutShort }   // each Display is a plain sentence
pub const MAX_LABEL_VOXELS: u64 = 256 << 20;
pub fn read_labels(bytes: &[u8]) -> Result<LabelGrid, NiftiError>;   // gzip or plain, told apart by the first two bytes
// behind the `testkit` feature, and in this crate's own tests:
pub struct NiftiOptions { pub dims: [u16; 3], pub ids: Vec<u32>, pub kind: i16, pub srow: Option<[[f32; 4]; 3]>, pub quatern: Option<([f32; 3], [f32; 3], [f32; 3], f32)>, pub big_endian: bool, pub slope: f32 }
pub fn nifti(options: &NiftiOptions) -> Vec<u8>;
pub fn gzipped(bytes: &[u8]) -> Vec<u8>;
```
Kinds read: 2 (u8), 256 (i8), 4 (i16), 512 (u16), 8 (i32), 768 (u32), and 16 / 64 (float) when every value is a whole number. A negative value, or one that is not whole, is `NotWhole`. `scl_slope` other than 0 or 1, or an intercept, is `Scaled`. `dim[0]` from 3 to 7 with every dimension past the third equal to 1. The data begin at `vox_offset` (at least 352). The reader takes exactly as many bytes as the header says and no more, so a gzip that would unpack without end cannot fill memory.

- [ ] **Step 1: Write failing tests.** A 2 x 3 x 4 volume of u16 counting from 0, `srow` rows `[1,0,0,10] [0,1,0,20] [0,0,1,30]`: dims, ids in order, and voxel (1, 2, 3) lies at patient (-11, -22, 33). The same gzipped, big-endian, and as each other kind give the same grid. With `srow` absent and a quaternion for a quarter turn about z, offsets (10, 20, 30), pixdim (2, 3, 4), qfac 1: voxel (1, 0, 0) is 2 mm along world +y, so at patient (-10, -22, 30); qfac -1 turns the third axis round. `sform` is used when both are given. Refused, each with its own error: ten bytes of nothing; a header of 540; magic `ni1`; a fourth dimension of 2; kind 128 (RGB); slope 2; a float of 1.5; a value of -1; no `sform` and no `qform`; dims 4000 x 4000 x 4000; the data cut one byte short; a gzip cut short. No error's sentence contains a Rust type name.
- [ ] **Step 2:** Run `cargo test -p voxa-labels nifti`. Expected: FAIL (the crate does not exist).
- [ ] **Step 3:** Implement. Add the crate to the workspace and to `COVERED_CRATES`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(labels): read a NIfTI label volume and place it in the patient`.

---

### Task 2: Tables of names, and a label set

**Files:** `crates/voxa-labels/src/{table.rs,table/tests.rs,set.rs,words.rs}`

**Interfaces:**
- Consumes: `LabelGrid`.
- Produces:

```rust
pub enum Side { Left, Right, Midline }
pub enum Trust { Model, Approximate(String), NotNamed }            // Approximate carries how it was made, possibly empty
pub struct Entry { pub name: String, pub group: String, pub side: Side, pub ids: Vec<u32>, pub colour: [u8; 3], pub trust: Trust }
pub struct Verdict { pub passed: bool, pub fail_below: f64, pub low: Vec<(String, f64)> }
pub struct Table { pub title: String, pub entries: Vec<Entry>, pub verdict: Option<Verdict> }
pub enum TableError { NotATable, TooLong }
pub fn read_table(file_name: &str, text: &str) -> Result<Table, TableError>;   // `.ctbl` by its name, otherwise JSON of either shape
pub struct Part { pub name: String, pub group: String, pub side: Side, pub colour: [u8; 3], pub trust: Trust, pub voxels: u64, pub ml: f64 }
pub struct LabelSet { pub dims: [u32; 3], pub numbers: Vec<u16>, pub placement: Affine<Voxel, Patient>, pub parts: Vec<Part>, pub title: String, pub verdict: Option<Verdict> }
pub enum SetError { TooManyParts }
pub fn label_set(grid: &LabelGrid, table: Option<&Table>) -> Result<LabelSet, SetError>;   // numbers[i] is 0 or 1 + the part's place in `parts`
pub const NOT_NAMED: &str = "Not named in the table";
pub fn verdict_in_words(verdict: &Verdict) -> String;
pub fn volume_in_words(ml: f64) -> String;       // "4.0 ml", "48 mm³"
pub fn trust_in_words(trust: &Trust) -> String;  // "model", "approximate", "not named"
```
A table entry none of whose numbers is in the volume makes no part. A Slicer table's names lose their underscores, and a name ending "(left)" or "(right)" gives the side; number 0 is skipped. In the `regions` JSON, `ids` with `left` and `right` make two entries and `midline` one; `approximate: true` makes `Trust::Approximate(how)`. `qc.scores` below `fail_below` are the verdict's `low`.

- [ ] **Step 1: Write failing tests.** A `regions` JSON with "hippocampus" (left 17, right 53, "Deep brain", `#dcd814`), "brain stem" (midline 16) and "eyeball" (left 101, right 102, approximate, how "Rules.") and a `qc` with `fail_below` 0.65, scores 0.8 and 0.58 for "general csf", passed false: five entries in that order with sides, colours and trust; the verdict has one low score. The plain JSON `{"parts":{"1":"cochlea","2":"vestibule"},"colours":{"1":"#E69F00"}}`: two midline entries, the second with a generated colour. A `.ctbl` with a comment line, `0 background 0 0 0 0`, `17 hippocampus_(left) 220 216 20 255` and a line of three fields: one entry, "hippocampus", left; the short line is passed over. Text that is not a table, and one of 5 MiB, are errors. A grid holding 0, 16, 17, 17, 53, 99 with that table and voxels of 1 x 2 x 3 mm: parts hippocampus left (2 voxels, 0.012 ml), hippocampus right, brain stem, then "Label 99" in `NOT_NAMED` with `Trust::NotNamed`; no eyeball; `numbers` is `[0, 3, 1, 1, 2, 4]`. With no table: "Label 16", "Label 17", "Label 53", "Label 99", their colours all different and the same on a second run. 70 000 different numbers is `TooManyParts`. `verdict_in_words` for the failed one names "general csf" and both numbers and begins "The labelling model's own check failed"; for a passed one it says it passed and that this is the model's opinion of itself. `volume_in_words(4.04)` is "4.0 ml", `(0.0478)` is "48 mm³".
- [ ] **Step 2:** Run `cargo test -p voxa-labels`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(labels): tables of names, parts and their volumes`.

---

### Task 3: Finding a part: at a point, along a line of sight, on either side

**Files:** `crates/voxa-labels/src/{find.rs,pairs.rs}`

**Interfaces:**
- Consumes: `LabelSet`.
- Produces:

```rust
impl LabelSet {
    pub fn part_at(&self, point: Point<Patient>) -> Option<usize>;        // the nearest voxel's part; None outside the volume or on 0
    pub fn corners(&self) -> [Point<Patient>; 8];
    pub fn finest_mm(&self) -> f64;
}
pub fn first_part(set: &LabelSet, shown: &[bool], from: Point<Patient>, towards: [f64; 3]) -> Option<(usize, Point<Patient>)>;
pub fn boxes_meet(a: &[Point<Patient>; 8], b: &[Point<Patient>; 8]) -> bool;   // by their extents along the patient's axes
pub struct Pair { pub name: String, pub group: String, pub left: Option<usize>, pub right: Option<usize>, pub midline: Option<usize> }
pub fn pairs(parts: &[Part]) -> Vec<Pair>;                                  // one for each name within a group, in the order first met
pub fn difference_percent(left_ml: f64, right_ml: f64) -> Option<f64>;       // (left - right) over their mean; None when both are 0
```
`first_part` walks the line in steps of half the finest voxel from where it enters the volume's box to where it leaves, and gives the first voxel whose part is shown, with the point reached.

- [ ] **Step 1: Write failing tests** on a 10 x 10 x 10 set of 1 mm voxels whose placement turns x and y round (as a NIfTI's would), with part 1 filling voxels x < 5 and part 2 the rest. `part_at` at the patient point of voxel (2, 3, 4) is 0 and of (7, 3, 4) is 1; half a voxel outside any face it is `None`; a point that is not a number is `None`. A line from outside along the patient's +x meets part 2 first if that is the side it comes from, and part 1 with part 2 hidden; with both hidden, `None`; a line that misses the box, `None`; a direction of no length, `None`. The point given lies within 1 mm of the face it entered by. `boxes_meet` for two boxes sharing a corner region is true and for two 100 mm apart is false. `pairs` of hippocampus left and right, brain stem and a second group's "hippocampus": three pairs. `difference_percent(11, 9)` is 20, `(0, 0)` is `None`.
- [ ] **Step 2:** Run `cargo test -p voxa-labels`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(labels): the part at a point and along a line of sight`.

---

### Task 4: Labels on the graphics device, and over the planes

**Files:** `crates/voxa-render/src/{labels.rs,plane_pipeline.rs,plane.wgsl,offscreen_scene.rs,offscreen_scene/label_tests.rs,testing.rs}`, `crates/voxa-render/Cargo.toml`

**Interfaces:**
- Consumes: `LabelSet` (Task 2); `PlanePipeline`, `HeadlessScene::render_plane` (milestone 3).
- Produces:

```rust
pub struct PartLook { pub colour: [u8; 3], pub opacity: f32 }       // 0 is hidden, 1 solid
pub enum Overlay { Off, Outline, Filled }
pub const FILL: f32 = 1.0 / 3.0;
pub struct LabelsPrepared { .. }   impl LabelsPrepared { pub fn of(set: &LabelSet) -> Self; pub fn on(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<LabelsOnDevice>; }
#[derive(Clone)] pub struct LabelsOnDevice { .. }
impl LabelsOnDevice { pub fn set_looks(&self, queue: &wgpu::Queue, looks: &[PartLook], selected: Option<usize>); }
impl PlanePipeline { pub fn labels(&mut self, with: Option<&LabelsOnDevice>, overlay: Overlay); }
impl HeadlessScene { pub fn label(&mut self, set: Option<&LabelSet>) -> bool; pub fn set_looks(&mut self, looks: &[PartLook], selected: Option<usize>); pub fn overlay(&mut self, overlay: Overlay); }
```
The part numbers go on the device as a 3D texture of 16-bit unsigned numbers read without blending; the looks as a 256-wide texture of colours, one texel a part, alpha the opacity. In the plane shader a pixel takes its part from the nearest label voxel to the point of the patient it shows. A pixel is on an outline when the part at the pixel to its right or beneath differs from its own and either is shown; an outline is drawn in the part's colour at its opacity, and in white for the part selected. Filled also tints the inside by `FILL` times the opacity. Uploading a volume takes the labels off the planes of the headless scene, as it does the curve.

- [ ] **Step 1: Write failing tests** on the test ball with a made-up set on a grid of its own (2 mm voxels, x and y turned round): part 1 the ball's left half, part 2 its right half, part 3 a small box at the bead. Filled, axial through the middle: a pixel well inside the left half is its grey mixed a third of the way to part 1's colour, on the side of the picture where the letter L is; with Radiological off the two halves change sides. Outline: that pixel is exactly the plain picture's, and a pixel on the line between the halves is a part's colour. Off, and with no labels: the plain picture exactly. Part 1 hidden: no tint and no outline round it except where it borders part 2. Opacity a half: half the tint. Part 2 selected: its outline white. A set lying 500 mm away changes nothing. On a ball tilted 20 degrees the boundary between the halves still passes through the middle of the picture.
- [ ] **Step 2:** Run `cargo test -p voxa-render label`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): labels outlined and tinted over the plane views`.

---

### Task 5: The parts as solids in 3D

**Files:** `crates/voxa-render/src/{parts_pipeline.rs,parts.wgsl,scene.rs,volume_params.rs,offscreen_scene.rs,offscreen_scene/parts_tests.rs}`

**Interfaces:**
- Consumes: `LabelsOnDevice` (Task 4); `SceneParams`, `Camera`, `CutPlane` (milestone 2).
- Produces: `Look::Parts`; 

```rust
pub struct PartsPipeline { .. }
impl PartsPipeline {
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self;
    pub fn show(&mut self, device: &wgpu::Device, labels: Option<&LabelsOnDevice>);
    pub fn has_labels(&self) -> bool;
    pub fn set_params(&self, queue: &wgpu::Queue, params: &SceneParams, viewport: [f32; 2]);
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>);
    pub fn picture(&self, device: &wgpu::Device, queue: &wgpu::Queue, params: &SceneParams, viewport: [f32; 2], size: [u32; 2]) -> Result<Vec<u8>, RenderError>;
}
impl HeadlessScene { pub fn render_parts(&mut self, params: &SceneParams, size: [u32; 2]) -> Result<Vec<u8>, RenderError>; }
```
For every pixel the line of sight is clipped to the labels' own box and walked in steps of half the finest label voxel. Where it first enters a part that is shown, the surface is placed between the last two steps where the blended share of shown voxels crosses a half, and lit by the slope of that share, with the light the Surface look uses. A part with opacity below 1 lets the line go on, adding what lies behind in front-to-back order until nothing more shows. The part selected is lit with a pale rim where its surface turns away, so it can be told from its neighbours without changing its colour. Cut planes shorten the line as in the other looks; the window plays no part. The volume pipeline, asked for `Look::Parts`, draws Surface.

- [ ] **Step 1: Write failing tests** with the set of Task 4. From the front (anterior view) the left half of the ball, in part 1's hue, is on the right of the picture and part 2's on the left; from behind they change sides; from the patient's left only part 1's hue is seen. The bead's box shows in part 3's hue from the front. With part 1 hidden, from the patient's left the flat face of part 2 is seen, in part 2's hue. With part 1 at opacity 0.5 that view is a mix of the two hues. Every pixel is opaque; outside the solids the picture is black. A cut plane facing the camera through the middle shows the two halves' colours on a flat face. The silhouette is as wide as the ball within 2 pixels. With part 2 selected its edge pixels are paler than without, and part 1's are unchanged. Half quality gives nearly the same picture. With no labels the picture is black, and `has_labels` is false.
- [ ] **Step 2:** Run `cargo test -p voxa-render parts`. Expected: FAIL.
- [ ] **Step 3:** Implement. Add a criterion case: the parts of a 200 x 230 x 150 set in a 1280 x 800 picture.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): labelled parts as lit solids in 3D`.

---

### Task 6: Labels in the state

**Files:** `crates/voxa-app/src/{labels.rs,action.rs,state.rs,reducer.rs,reducer/labels.rs,reducer/labels_tests.rs,lib.rs}`

**Interfaces:**
- Consumes: `Part`, `Verdict`, `Pair` (Tasks 2, 3); `Overlay`, `PartLook`, `Look::Parts` (Tasks 4, 5).
- Produces:

```rust
pub enum LabelsStatus { None, Reading, Ready, Failed(String) }
pub struct PartShown { pub shown: bool, pub opacity: f32 }
pub struct Labels { pub status: LabelsStatus, pub title: String, pub parts: Vec<Part>, pub shown: Vec<PartShown>, pub selected: Option<usize>, pub overlay: Overlay, pub verdict: Option<Verdict>, pub study: Option<StudyId>, pub corners: Option<[Point<Patient>; 8]>, pub names_from: NamesFrom }
pub enum NamesFrom { Table, Nothing }
impl Labels { pub fn looks(&self) -> Vec<PartLook>; pub fn on_this_series(&self, state: &AppState) -> bool; }
// Action gains:
OpenLabels { labels: PathBuf, table: Option<PathBuf> }, LabelsRead { title, parts, verdict, corners, names_from }, LabelsFailed(String), CloseLabels,
PartShow { part: usize, shown: bool }, GroupShow { group: String, shown: bool }, PartsShowAll, PartIsolate(usize), PartOpacity { part: usize, opacity: f32 }, PartSelect(Option<usize>), SetOverlay(Overlay)
// Effect gains:
ReadLabels { labels: PathBuf, table: Option<PathBuf> }, DropLabels
```
`OpenLabels` needs a volume ready in the scene; it records that series' study, sets `Reading` and asks for `ReadLabels`. `LabelsRead` whose box does not meet the scene's volume fails with "These labels lie nowhere near this series. They were probably made from another scan." Hiding the selected part clears the selection. `on_this_series` is true when the scene's series is of the labels' study and the status is `Ready`. When it is not, a scene whose look is Parts is drawn as Surface (`Scene::look_drawn`). Opening another export, or `CloseLabels`, empties the labels and asks for `DropLabels`. `SceneLook(Look::Parts)` with no labels on this series is ignored.

- [ ] **Step 1: Write failing tests** for each sentence above, and: opening with no volume ready does nothing; every part starts shown at opacity 1 with the overlay on Outline; `PartIsolate` shows one part and hides the rest and `PartsShowAll` brings all back at the opacity each had; `GroupShow` acts on a group's parts only; an opacity outside 0 to 1, or not a number, is held or ignored; a part number past the end is ignored; `looks()` gives opacity 0 for a hidden part; selecting another series of the same study keeps the labels on and one of another study takes them off without forgetting them; `LabelsRead` arriving after another export was opened is ignored.
- [ ] **Step 2:** Run `cargo test -p voxa-app labels`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): labels, and what is shown of them, in the state`.

---

### Task 7: Reading labels on a worker

**Files:** `crates/voxa-app/src/{label_store.rs,session.rs,worker.rs,worker/labels.rs,worker/labels_tests.rs,args.rs,main.rs,ui/mod.rs}`

**Interfaces:**
- Consumes: `read_labels`, `read_table`, `label_set` (Tasks 1, 2); `LabelsPrepared` (Task 4); the actions and effects of Task 6.
- Produces:

```rust
pub struct LabelStore { .. }   impl LabelStore { pub fn get(&self) -> Option<Arc<LabelSet>>; pub fn take_prepared(&mut self) -> Option<LabelsPrepared>; pub fn clear(&mut self); }
impl Session { pub fn labels(&self) -> Option<Arc<LabelSet>>; pub fn take_labels_prepared(&self) -> Option<LabelsPrepared>; }
pub fn table_beside(labels: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf>;     // the rule of Conventions
pub const MAX_LABEL_FILE_BYTES: u64 = 1 << 30;
// Command::Run gains the labels to open: Run { scan: Option<PathBuf>, labels: Option<PathBuf> }, from `--labels PATH`
```
A worker reads the file (no more than `MAX_LABEL_FILE_BYTES`), finds the table if none was named, builds the set and what the device needs, keeps both, and reports `LabelsRead`. Whatever goes wrong is `LabelsFailed` with a sentence: the file cannot be read, is not labels (the `NiftiError`'s own sentence), or holds more parts than Voxa shows. A table that cannot be read does not fail the labels: they are shown unnamed, and a notice says the names could not be read. A read overtaken by a newer one, or by another export, keeps and reports nothing. `--labels` is opened once the first volume is ready. A `.nii` or `.nii.gz` dropped on the window opens as labels; anything else opens as a scan.

- [ ] **Step 1: Write failing tests.** A set and table written to a temporary folder are read, kept and reported with their parts and volumes. `table_beside` for `labels_a.nii.gz` tries `labels_a.json`, `labels_a.ctbl`, `regions_a.json`, `regions.json`, `labels.ctbl` in that order. A file of noise reports the sentence for `NotNifti`; a missing file says it cannot be read and does not give its path; a table of noise reports the labels unnamed and posts the notice. Two reads one after the other leave the second's set. A read followed by opening another export leaves the store empty. `parse_args` of `scan --labels l.nii.gz` gives both; `--labels` with nothing after it is an error in words; the usage text names it.
- [ ] **Step 2:** Run `cargo test -p voxa-app labels` and `args`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): label files read on a worker, from the command line or dropped on the window`.

---

### Task 8: The Parts list

**Files:** `crates/voxa-app/src/ui/{parts.rs,parts/tests.rs,parts/rows.rs,layout.rs,pictures.rs}`, `planes.rs` (`ColumnShows::Parts`)

**Interfaces:**
- Consumes: `Labels` and its actions (Task 6); `pairs`, `volume_in_words`, `trust_in_words`, `verdict_in_words` (Tasks 2, 3).
- Produces: `ColumnShows::Parts`; `ui::parts::show(ui, area, state, view: &mut PartsView, actions) -> Option<Asked>` with `Asked::{OpenLabels, OpenNames, SaveCsv}`; `pub fn rows(labels: &Labels, search: &str) -> Vec<Row>` (pure: the groups and rows the list shows for a search).

A third tab on the column, "Parts". With no labels it says what labels are and has an "Open labels…" button. With labels: the title; where the names came from; the model's check in a sentence, in the warning colour when it failed; a search box; "Show all" and "Close"; the overlay's three choices; then the groups, each with a switch for the whole group, and under each its rows. A row has a swatch of its colour, the name, a switch for each side it has (L, R, or one), the volume of each side in quiet type, and the word for its trust when that is not "model", which says how the part was made when pointed at. Clicking a name selects the part (its left side first if it has two); the selected row is marked, and beneath the list are its opacity and "Show only this". While labels are being read the tab says so; when they failed, why, with the button again.

- [ ] **Step 1: Write failing tests.** `rows` of the Task 2 set: groups in table order, "hippocampus" one row with two sides; a search for "hip" keeps that row and its group and nothing else, whatever the case of the letters; a search that finds nothing gives no rows. Clicking down the list finds, for each part, its switch, and `PartSelect`, `GroupShow`, `PartsShowAll`, `CloseLabels`, the three overlays, and with a part selected `PartIsolate` and a change of opacity. With no labels the sweep finds only `Asked::OpenLabels`. A failed verdict's sentence is written; so is "Names from the table" or that there were none. The strip's sweep finds the third tab.
- [ ] **Step 2:** Run `cargo test -p voxa-app parts`. Expected: FAIL.
- [ ] **Step 3:** Implement. Capture the window with the made-up labels (written by hand into the scratch folder until Task 12) at 1440 x 900 and 900 x 600 and read the pictures.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): the Parts list: search, show, hide and select`.

---

### Task 9: Labels over the plane views

**Files:** `crates/voxa-app/src/ui/{labels.rs,planes.rs,planes/gpu.rs,pictures.rs,mod.rs}`

**Interfaces:**
- Consumes: `Session::labels`, `take_labels_prepared` (Task 7); `PlanePipeline::labels`, `LabelsOnDevice::set_looks` (Task 4); `LabelSet::part_at` (Task 3).
- Produces: `ui::labels::Held`, the labels on the device as the window keeps them: `Held::keep(&mut self, render, state, set, take) -> bool` (true when the device had no room) and `Held::on_device(&self) -> Option<&LabelsOnDevice>`; `PlanesView::label(&mut self, with: Option<&LabelsOnDevice>, overlay: Overlay)`; `pub fn part_under(state: &AppState, set: &LabelSet, at: Point<Patient>) -> Option<usize>` (the part there if it is shown).

The window takes the prepared labels once and puts them on the device; the same copy serves the planes and, in Task 10, the scene. The looks are rewritten only when what is shown, faded or selected has changed. A panel showing labels names the part under the pointer beside it. A click that moves the position also selects the part there, or clears the selection. A panel says "Labels: made elsewhere, shown as given" in its note when it draws any.

- [ ] **Step 1: Write failing tests.** `part_under` gives a shown part, nothing for a hidden one, nothing off the labels. The click's actions are `SetPosition` then `PartSelect(Some(n))`, and `PartSelect(None)` on nothing; with a measuring tool in hand a press selects nothing. The looks are sent once for a state and again only after `PartShow`, `PartOpacity` or `PartSelect` (counted). Labels of another study are taken off the panels.
- [ ] **Step 2:** Run `cargo test -p voxa-app planes`. Expected: FAIL.
- [ ] **Step 3:** Implement. Capture the planes with labels, outlined and filled, at both sizes and read the pictures.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): labels outlined over the plane views, named on hover, selected by a click`.

---

### Task 10: The Parts look in the scene

**Files:** `crates/voxa-app/src/ui/{scene.rs,scene/gpu.rs,scene/parts.rs,scene_bar.rs,scene/captions.rs,saving.rs}`

**Interfaces:**
- Consumes: `PartsPipeline` (Task 5); `first_part` (Task 3); `Scene::look_drawn` (Task 6); the device's labels (Task 9).
- Produces: a "Parts" choice on the scene's bar, greyed with the reason when there are no labels on this series; the scene drawn by `PartsPipeline` while that is the look; `pub fn part_seen(state, set, viewport, at) -> Option<(usize, Point<Patient>)>` (the first shown part on the line of sight through a point of the picture, past the cuts).

Hovering a solid names it beside the pointer; a click selects it and moves the position to the point clicked; a click on nothing clears the selection. A ruler and a marker are placed on the solids as on any surface. "Save picture…" and "Save turntable…" save what is on the screen, so the Parts look when that is the look. The caption says "Solids are labels made elsewhere, drawn as given: Voxa did not find them in this scan."

- [ ] **Step 1: Write failing tests.** `part_seen` on the made-up set from the front gives the half on each side and, on the bead, part 3; nothing beside the ball; with a part hidden, what is behind it; with a cut, what the cut uncovers. The bar's sweep finds `SceneLook(Look::Parts)` with labels and not without. The caption's sentence is there in the Parts look and not in the others. A click's actions are `SetPosition` and `PartSelect`.
- [ ] **Step 2:** Run `cargo test -p voxa-app scene`. Expected: FAIL.
- [ ] **Step 3:** Implement. Capture the scene in the Parts look, whole and with a cut and a part faded, at both sizes, and read the pictures.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): the Parts look: labelled solids in the 3D scene, named and selected`.

---

### Task 11: Left against right, and saving the list

**Files:** `crates/voxa-app/src/{parts_table.rs}`, `ui/parts.rs`, `ui/help.rs`

**Interfaces:**
- Consumes: `pairs`, `difference_percent` (Task 3); `export::write`, the notices of milestone 2.
- Produces: `pub fn parts_csv(labels: &Labels) -> String` (`group,name,side,volume_ml,voxels,trust,how` ; RFC 4180; CR LF; a name a spreadsheet would run as a formula written as text); `pub const PARTS_NAME: &str = "voxa-parts"`; in the list, under a selected part that has two sides, "Left 4.0 ml, right 6.0 ml: the left is 40% smaller. Voxa cannot tell how much of a difference like this is noise in the labelling."; "Save as CSV…" at the foot of the list.

- [ ] **Step 1: Write failing tests.** The table's header and rows for the Task 2 set, hidden parts included; the comparison sentence for 4.0 against 6.0, for equal sides ("the same"), and none for a part with one side; the file name offered; nothing in the table but what the label file and its table held.
- [ ] **Step 2:** Run `cargo test -p voxa-app parts_table`. Expected: FAIL.
- [ ] **Step 3:** Implement. Add the look, the tab and what a click does to the keys-and-mouse list.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): left against right for a part, and the list saved as a table`.

---

### Task 12: Acceptance and release 0.4.0

**Files:** `crates/voxa-app/examples/{write_phantom.rs,check_labels.rs}`, `docs/acceptance-0.4.0.md`, `CHANGELOG.md`, `README.md`, `docs/architecture.md`, `docs/adr/`, `docs/images/`, the workspace version

- [ ] **Step 1:** `write_phantom` also writes `labels.nii.gz` and `regions.json` for its ball: a core, the shell's left and right halves, and the bead, on a 1.5 mm grid of their own in the other voxel order, with one part marked approximate and a made-up check that fails. A test reads them back.
- [ ] **Step 2:** `check_labels EXPORT LABELS`: opens both with no window, and for each series of the labels' study that has a 3D view draws the three planes with the overlay and the Parts look, checks that the overlay changes the picture, that the solids fall within the scan's box, and that left parts lie on the patient's left of right parts of the same name; prints counts, the share of the label box inside the scan's, and times, and nothing from the files.
- [ ] **Step 3:** Run `cargo xtask check` and `cargo xtask coverage`. Acceptance on the made-up study, recorded in `docs/acceptance-0.4.0.md`, and on a real export with label files made for it, locally, of which the record gives nothing and pictures stay out of the repository. What a person must try by hand is listed as not checked.
- [ ] **Step 4:** An ADR for labels keeping their own grid. `CHANGELOG.md` with 0.4.0, `docs/architecture.md`, `README.md` with a picture of the made-up study with its labels, the workspace version to `0.4.0`.
- [ ] **Step 5:** Commit: `chore: acceptance record and 0.4.0 changelog`, then `git tag v0.4.0`. Do not push, and do not merge into `main`.

---

## Out of scope

Running a labelling model; more than one label set at once; labels in the 2D stack viewports; hand corrections, painting, renaming and merging; FreeSurfer, NRRD and DICOM SEG files; meshes and STL; a report; remembering the labels of a series between sessions; comparing the volumes of two studies.
