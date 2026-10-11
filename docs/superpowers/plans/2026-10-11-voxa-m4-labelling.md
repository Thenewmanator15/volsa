# Voxa Milestone 4, second half (Labelling a scan inside the app) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person presses **Label this scan** in the Parts tab and Voxa labels the brain itself with SynthSeg 2.0, on a graphics card or on the processor, with no Python at run time, and shows the result as the label set of 0.4.0 with the model's own verdict on it.

**Architecture:** A Python converter under `tools/` turns SynthSeg's files into a model folder that Voxa checks and reads. Two new crates: `voxa-models` runs a network through ONNX Runtime behind a trait and works out what fits on which device; `voxa-segment` holds SynthSeg's arithmetic around the networks and is tested against arrays SynthSeg's own Python made. The app gains one worker, a few actions and effects, and a strip in the Parts tab; a result enters the state through the door a label file already uses.

**Tech Stack:** As 0.4.0, plus `ort` (the exact release Task 1 settles, feature `load-dynamic`, no default features), `sha2`, `sysinfo`, and the `windows` crate already in the tree (one module). For the tools only, never for Voxa: Python 3.13, TensorFlow 2.21, tf_keras 2.21, tf2onnx 1.17.0, onnx 1.23.2, onnxruntime-directml 1.24.4, nibabel, scipy.

**Spec:** `docs/superpowers/specs/2026-10-11-voxa-labelling-design.md`. The owner approved it on 2026-10-11; the seven defaults of its section 12 stand.

**Branch:** `labelling-design`, which holds the spec and this plan, cut from `main` at `edd4099`.

**Amended 2026-10-11, after Tasks 1 and 2 were done.** Both met every mark. What they found changed four things below, each marked *(amended)*: how a runtime's version is judged and how answers are fetched from a card (Task 4); what a card has room for, and that the scores network may run on the processor on its own (Tasks 5 and 13); and the memory figures (Conventions). The findings are in `.superpowers/sdd/labelling-findings.md` of the main checkout, with the ledger beside them.

## Scope

In: the converter and the model folder; the runtime loaded from files beside Voxa; the standard model, the robust chain, the cortex areas and the scores; the choice of device and the fall from one to the next; results kept, shown again, deleted and saved; the strip in the Parts tab; a decision record and the documents.

Tasks 1 and 2 prove what the feasibility check left unproved. **If Task 1 misses a pass mark, stop and tell the owner**; nothing after it is worth building.

## Conventions

- **Axes.** A network sees `[1, x, y, z, c]`, last index fastest, where x runs to the patient's right, y to the front and z to the top: SynthSeg's order. A map of classes is `[1, x, y, z]` of bytes. Voxa's patient space is left-posterior-superior, so a volume's transform has its x and y rows negated before SynthSeg's rules are applied, and again when coming back.
- **The model folder** holds `model.json` and six networks. Their inputs and outputs, by name:

  | File | Takes | Gives |
  |---|---|---|
  | `standard.onnx` | `image` f32 `[1,x,y,z,1]` | `probabilities_a` f32 `[1,x,y,z,17]`, `probabilities_b` f32 `[1,x,y,z,16]` (blurred as SynthSeg does) |
  | `robust_1.onnx` | `image` | `classes` u8 `[1,x,y,z]` |
  | `robust_2.onnx` | `classes` u8 | `cleaned` u8 `[1,x,y,z]` |
  | `robust_3.onnx` | `image`, `classes` u8 | `probabilities_a`, `probabilities_b` |
  | `cortex.onnx` | `image`, `cortex` u8 (1 where cortex) | `areas` u8 `[1,x,y,z]`, 1 to 68 |
  | `scores.onnx` | `classes` u8 `[1,224,224,224]` | `scores` f32 `[1,n]` |

- **`model.json`**: `format` 1; `kind` `"synthseg-2.0"`; `files` (for each of `standard`, `robust_1`, `robust_2`, `robust_3`, `cortex`, `scores`: `name`, `bytes`, `sha256`); `structures` (33, in the networks' order, the first being label 0: each `label`, `name` without its side, `side` of `left` / `right` / `midline`, `class`); `flip` (33 places); `stage_classes` (5); `areas` (69, the first label 0: `label`, `name`, `side`); `cortex` (`[3, 42]`); `score_class` (33 values); `score_groups` (the names of the scores); `box` (224); `made` (`converter` 1, the tools' versions, `opset` 17, `source`: the SHA-256 of each `.h5`); `checked` (what the converter's own check measured).
- **Labels** in a label grid are the structures' own numbers (2, 3, 4, … and 1001 to 2035 for cortex areas), so a saved file reads the same in other programs.
- **Fixed values.** Resample when a spacing is more than 0.05 mm from 1 mm. Percentiles 0.5 and 99.5. Sides padded to a multiple of 32 and at least 128. Thresholds of 0.25 in tidying. Scores fail below 0.65. Robust is picked automatically when slices are more than 3 mm apart. Memory *(amended, as measured)*: 1 400 bytes a voxel on a card for every network but the scores; on the processor 2 000, and 2 300 for `cortex`; the scores network a fixed 8.2 GiB on any device, whatever the scan's size; a quarter added to each. No more than 2 GiB is fetched from a card by one run.
- **Agreement marks** (spec, section 9): standard model on the processor, every voxel the same as Python; any other model or device, at least 99.99% of voxels, Dice at least 0.999 for every structure, scores within 0.001.
- **Environment.** `VOXA_MODELS` names a model folder and `VOXA_RUNTIME` a folder holding the runtime's files. Tests that need them are `#[ignore]`d and say so; nothing else reads them.
- **Findings** go in `.superpowers/sdd/labelling-findings.md` (git-ignored), and from there into decision record 0005 in Task 17. Throwaway programs live under `.superpowers/spikes/`, each with its own `[workspace]`, and are never committed.
- **`ort` is newer than anyone's memory of it.** Read its source under `~/.cargo/registry/src` before writing against it. This plan names what must be done, not `ort`'s function names.

## Global Constraints

Every constraint of the earlier plans still holds: the disclaimer verbatim; dual MIT / Apache-2.0; patient data never in the repository; nothing from a real export or a real label file in a test, a document or a commit message; the repository stays local, nothing is pushed or merged into `main`; library crates never panic; every public item documented; conventional commits; `Patient`, `Voxel` and `Screen` stay distinct types; `voxa-app` contains no image mathematics; a background task that panics must not stop the program; every sentence shown is plain words and says what the picture cannot show; no path or file name of a label file in a log or a kept file. In addition:

- No model weights in the repository, in a release, or fetched by Voxa. No `.onnx`, `.h5` or `DirectML.dll` is ever committed.
- No network access and no telemetry, from Voxa or from the runtime: telemetry is switched off before the first session exists.
- `cargo xtask check` passes at every commit **with no runtime and no model folder on the machine**, on Windows and Linux.
- Voxa starts and does everything it did at 0.4.0 when the runtime or the model folder is missing, damaged or of the wrong version.
- Crates depend only downward: core; then dicom, volume, labels and models; then segment and render; then app.
- `unsafe` stays out of every crate but `voxa-render`, and one module, `voxa-models/src/cards.rs`, where each use carries a safety comment.
- `voxa-segment` and `voxa-models` join the crates held to 80% line coverage; `runtime.rs` and `cards.rs`, which need a runtime or a card, are left out of the count.
- The tools under `tools/` are not built, run or tested by cargo.
- A silent change of plan is a defect: a fall to the processor and a cut to the head are both said on the result.

## Review Focus

1. **Left and right.** Labels mirrored left to right look right and are wrong. The sign change between Voxa's space and SynthSeg's, the mirrored run of the standard model and its swap of left and right structures, the turn back to the volume's own axes. Tasks 6, 8, 9 and 11.
2. **A volume stored another way.** Sagittal and coronal stacks, a tilted stack, slices 6 mm apart, voxels of 0.3 mm, three slices, six hundred. The labels must land where Python's do, or the scan be refused in a sentence. Tasks 6 and 11.
3. **Things going away mid-run.** The series closed, another export opened, a label file opened, the button pressed twice, Stop pressed inside a network, Voxa closed. No result on the wrong study, no half-written kept file. Tasks 12, 13 and 14.
4. **Files and answers that are wrong.** A `model.json` that is not JSON, lists of the wrong length, a file name that climbs out of the folder, a network of the wrong size or sum, a runtime that is not the build asked for, an answer of the wrong shape or holding a number that is not finite. A sentence each, never a panic. Tasks 3, 4 and 8.
5. **Nothing to find.** A knee, a phantom, an empty volume: the model returns background everywhere, or specks. Voxa says the model found no brain; it does not show an empty list as a result. Task 9.

---

### Task 1: A network run from Rust (thrown away)

**Files:** `.superpowers/spikes/run-network/{Cargo.toml,src/main.rs}`, `.superpowers/sdd/labelling-findings.md`. Nothing is committed.

**Needs, from the owner:** a folder with Microsoft's ONNX Runtime 1.24.4 DirectML build (`onnxruntime.dll`, `onnxruntime_providers_shared.dll`, `DirectML.dll`); one tidied network from the feasibility check (the standard network on its own); the 160 cube array it was run on and Python's answer for it, made again with the check's own script.

**Produces:** a program `run-network RUNTIME NETWORK INPUT X Y Z DEVICE [EXPECTED]` (`DEVICE` is `processor` or `card N`) that loads the runtime from the folder, switches telemetry off, lists the graphics adapters in the system's own order with their names and own memory, opens the network on the device asked for with DirectML's rules (sequential, no memory pattern), tells the runtime the input's size when the session is made, runs it twice, and prints: seconds to open, seconds for each run, the largest difference from `EXPECTED`, the process's peak memory.

- [ ] **Step 1:** Write the program with `ort` 2.0.0-rc.13 and `load-dynamic`; if that release will not load a 1.24.4 runtime, with rc.12. No default features.
- [ ] **Step 2:** Run it on the processor and on each card. **Pass marks:** the answer is within 0.0001 of Python's everywhere on every device; the second run on the discrete card takes under 3 s; the adapter used is the one asked for by number.
- [ ] **Step 3:** Move `DirectML.dll` out of the folder and run on a card again. Record whether Windows' own copy is found and gives the same answer.
- [ ] **Step 4:** Run `cargo deny check --config ../../../deny.toml` in the spike. **Pass mark:** no licence outside `deny.toml`'s list. Record every crate `ort` brings.
- [ ] **Step 5:** Record in the findings: the `ort` release and features; how each thing was done (loading from a folder, telemetry off, the adapter by number, fixing the size, fetching one output at a time); times and memory; Step 3's answer. **If a pass mark was missed, stop here and tell the owner.**

---

### Task 2: The converter, and the model folder

**Files:** `tools/synthseg-to-onnx/{convert.py,graphs.py,check.py,requirements.txt,README.md}`, `docs/models.md`

**Interfaces:**
- Produces: `python convert.py SYNTHSEG_CHECKOUT OUT_FOLDER`, which writes the model folder of Conventions and, in `OUT_FOLDER/check/`: `case.json` (the dims and RAS transform of `volume` and of `thick`, the dims of `image`, the scores Python gave each model); `volume.f32` (a made-up head painted from SynthSeg's sample label map, 1 mm, columns fastest); `thick.f32` (the same at 1 x 1 x 4 mm); `image.f32` and `thick_image.f32` (what Python's preparing made of each, `[x,y,z]`, last index fastest); `standard.u16` and `robust.u16` (Python's final labels for `volume`, with cortex areas in `robust`, in the volume's own voxel order).

`graphs.py` holds the three changes made to what tf2onnx writes: each upsampling block (three rounds of `Unsqueeze`, `Tile`, `Reshape`) becomes one `Resize` (nearest, scales 1, 1, 2, 2, 2); a network whose answer feeds another gives the place of its largest value as a byte; a byte input is spread inside the file (`Cast`, `OneHot`) and joined to the image. `cortex.onnx` gives 1 plus the place of the largest of channels 1 to 68. The last network of each model is split after its final step into the two outputs. Names are stripped of "left " and "right " into `side`; a cortex name `ctx-lh-x` becomes `x`, left.

`check.py` runs SynthSeg's own program and the converted networks (processor, and every card DirectML offers) on `volume`, and `convert.py` writes `model.json` only if the agreement marks are met.

- [ ] **Step 1:** Write the three files from the feasibility check's scripts. Each network is built standing alone, as SynthSeg's `build_model` builds it, and loaded by layer name; a layer with weights that the `.h5` does not name is an error.
- [ ] **Step 2:** Run the converter. **Pass marks:** the agreement marks, for both models, with cortex areas, on every device; the byte answer of `cortex.onnx` gives the same final labels as SynthSeg's own tidying of the full probabilities.
- [ ] **Step 3:** Run Task 1's program on `standard.onnx` and `cortex.onnx` from this folder, on each device, with the size fixed, fetching the two halves one at a time. Record for each network and device the peak memory over the voxel count.
- [ ] **Step 4:** Write `README.md` (what it needs, how to run it, that it ships nothing) and `docs/models.md` (where SynthSeg's `.h5` files come from and under what terms, the converter, the runtime's three files and where they go, what `sha256` does and does not show).
- [ ] **Step 5:** Commit: `feat(tools): a converter from SynthSeg's files to a model folder`. No `.onnx` and nothing under `check/` is committed.

---

### Task 3: Reading and checking a model folder

**Files:** `crates/voxa-models/{Cargo.toml,src/lib.rs,src/model.rs,src/model/tests.rs,src/testkit.rs}`, root `Cargo.toml`, `xtask/src/main.rs`

**Interfaces:**
- Produces:

```rust
pub const MODEL_FORMAT: u32 = 1;
pub const KIND_SYNTHSEG: &str = "synthseg-2.0";
pub enum Role { Standard, Robust1, Robust2, Robust3, Cortex, Scores }
pub enum ModelSide { Left, Right, Midline }
pub struct Structure { pub label: u32, pub name: String, pub side: ModelSide, pub class: u32 }   // class is 0 for an area
pub struct Lists { pub structures: Vec<Structure>, pub flip: Vec<usize>, pub stage_classes: usize, pub areas: Vec<Structure>,
                   pub cortex: [u32; 2], pub score_class: Vec<usize>, pub score_groups: Vec<String>, pub box_side: usize }
pub struct Model { pub lists: Lists, pub sum: String /* SHA-256 of model.json, hex */ }
impl Model { pub fn file(&self, role: Role) -> &Path; }
pub enum ModelError { NotFound, NotAModel, Format(u32), Kind, Missing(Role), WrongSize(Role), WrongSum(Role), Lists(&'static str) }
pub fn read_model(folder: &Path) -> Result<Model, ModelError>;
// behind the `testkit` feature:
pub struct FolderOptions { /* every field of model.json, with a `Default` that passes */ }
pub fn model_folder(options: &FolderOptions) -> tempfile::TempDir;   // writes model.json and six small files with true sizes and sums
```
`model.json` is read up to 1 MiB; a network file up to 2 GiB. A `name` must be a plain file name: no separator, no `..`. `Lists(why)` says which rule failed: 33 structures, the first label 0, labels all different; `flip` holds each of 0 to 32 once and undoes itself; `stage_classes` from 2 to 16; 69 areas, the first label 0; both `cortex` labels among the structures; 33 `score_class` values, each at most the number of `score_groups`; `box` a multiple of 32 from 32 to 512. Every error's sentence is plain words with no path in it.

- [ ] **Step 1: Write failing tests.** The default folder reads: 33 structures, `file(Role::Cortex)` ends `cortex.onnx`, `sum` is 64 hex digits and changes when `model.json` does. One test for each way to be wrong, each with its own error: no folder; no `model.json`; `model.json` of `{`; of 2 MiB; `format` 2; `kind` `"other"`; `scores.onnx` absent; a file one byte longer than listed; a file with one byte changed; a `name` of `../x.onnx` and of `a/b.onnx` (both `NotAModel`); 32 structures; a label twice; `flip` of all zeros; a `score_class` beyond the groups; `box` 100. No error's sentence holds a Rust type name or a path.
- [ ] **Step 2:** Run `cargo test -p voxa-models model`. Expected: FAIL (the crate does not exist).
- [ ] **Step 3:** Implement. Add the crate and `sha2` to the workspace, and the crate to `COVERED_CRATES`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(models): read a model folder and check it against its own list`.

---

### Task 4: A network behind a trait, the runtime, and the cards

**Files:** `crates/voxa-models/src/{tensor.rs,network.rs,runtime.rs,cards.rs,network/tests.rs}`, `Cargo.toml`, `xtask/src/main.rs` (the two files left out of coverage)

**Interfaces:**
- Consumes: the findings of Task 1.
- Produces:

```rust
pub enum Numbers { F32(Vec<f32>), U8(Vec<u8>) }
pub struct Tensor { pub shape: Vec<usize>, pub numbers: Numbers }
impl Tensor { pub fn f32(shape: Vec<usize>, numbers: Vec<f32>) -> Option<Self>; pub fn u8(shape: Vec<usize>, numbers: Vec<u8>) -> Option<Self>; }   // None when the count is not the shape's
pub enum RunError { NoRoom, DeviceLost, Stopped, Failed(String) }   // Failed carries the runtime's own words, for the log only
pub trait Network: Send { fn run(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError>; }
pub struct Stop;                                   // Clone; shared
impl Stop { pub fn new() -> Self; pub fn stop(&self); pub fn is_set(&self) -> bool; }
pub enum Device { Processor, Card(u32) }
pub enum CardKind { Discrete, Integrated, Software }
pub struct Card { pub index: u32, pub name: String, pub kind: CardKind, pub own_bytes: u64 }
pub fn cards() -> Vec<Card>;                       // in the system's own order; empty where there is no such list
pub fn budget(index: u32) -> Option<u64>;          // (amended) what the system lets this program use on that card now
pub enum RuntimeError { NotFound, WrongBuild, WouldNotLoad }
pub struct Runtime;
impl Runtime {
    pub fn load(folder: &Path) -> Result<Self, RuntimeError>;      // once in the life of the program; telemetry off before it returns
    pub fn open(&self, file: &Path, device: Device, sizes: [usize; 3], stop: &Stop) -> Result<Box<dyn Network>, RunError>;
}
// behind `testkit`:
pub struct Canned;                                 // a Network that answers from a closure and remembers what it was given
impl Canned { pub fn new(answer: impl FnMut(&[Tensor]) -> Result<Vec<Tensor>, RunError> + Send + 'static) -> Self; }
```
*(amended)* `ort` takes any runtime at or above the version of the interface it asks for (1.17 with the features used) and refuses an older one: that is `WrongBuild`, decided by `fn right_build(found: &str) -> bool`. A card whose own memory is under 2 GiB is `Integrated`; one the system marks as software is `Software`. The system's list can change from one minute to the next and can hold one card twice, so a card is found again by name each time and never by a number remembered. On a card a network follows DirectML's rules and is told its input's size when opened, by the names `x`, `y` and `z`. *(amended)* Its outputs are fetched by an ordinary run; when together they would pass 2 GiB, each is fetched by a run of its own that asks for that output alone. `ort`'s `Tensor::to` is never used: with DirectML it ends the program with no error to catch. The runtime's "not enough memory" (`0x8007000E`) is `NoRoom`; "device removed", "hung" or "reset" (`0x887A0005` to `0x887A0007`) is `DeviceLost`. `Stop` is looked at before a run and ends a run in progress where the runtime allows. A panic inside the runtime becomes `Failed`.

- [ ] **Step 1: Write failing tests.** `Tensor::f32(vec![2, 3], vec![0.0; 5])` is `None`. `Runtime::load` on an empty folder is `NotFound`; on a folder whose `onnxruntime.dll` is ten bytes of nothing, `WouldNotLoad`. `right_build` takes 1.24.4 and 1.28.0, and refuses 1.16.3 and a string that is no version. `cards()` returns, and every card has a name. A `Canned` given two tensors remembers both. Ignored, needing both environment folders: `scores.onnx` opened on the processor and run on zeros gives `score_groups + 1` finite numbers; the same on each card not `Software`; a run with an input of the wrong shape is `Failed`, not a panic; a `Stop` set beforehand gives `Stopped`.
- [ ] **Step 2:** Run `cargo test -p voxa-models`. Expected: FAIL.
- [ ] **Step 3:** Implement, with `ort` pinned by `=` to the release in the findings. `cards.rs` is the only file with `unsafe`.
- [ ] **Step 4:** Run the tests, then the ignored ones with the folders set. Expected: PASS. Run `cargo xtask check` with the two variables unset.
- [ ] **Step 5:** Commit: `feat(models): run a network through ONNX Runtime, behind a trait`.

---

### Task 5: What fits where

**Files:** `crates/voxa-models/src/{room.rs,room/tests.rs}`

**Interfaces:**
- Consumes: `Card`, `CardKind`, `Device`; the memory figures of Task 2.
- Produces:

```rust
pub struct Wanted { pub robust: bool, pub cortex: bool }
pub enum RunsOn { Automatic, Card(u32), Processor }
pub enum Cut { Whole, Head }
pub struct Rung { pub device: Device, pub cut: Cut, pub scores_on: Device }   // (amended) where the scores network runs
pub struct Room { pub cards: Vec<Card>, pub drawing_on: Option<u32>, pub held: u64, pub free_memory: u64 }
pub fn needs(voxels: u64, wanted: Wanted, device: Device) -> u64;      // bytes, a quarter added
pub fn rungs(whole: u64, head: u64, wanted: Wanted, room: &Room, runs_on: RunsOn) -> Vec<Rung>;   // in the order to try
pub fn why_not(whole: u64, head: u64, wanted: Wanted, room: &Room) -> String;
```
A rung is offered only if it fits. *(amended)* A `Discrete` card has room for the system's budget for this program on it (`budget`), which already allows for what Voxa and every other program hold there; where that cannot be read, for its own memory less `held` if it is the one Voxa draws on. An `Integrated` card and the processor have `free_memory`. Networks run one at a time, so a run needs the most that any one of its networks needs. *(amended)* The scores network needs a fixed 8.2 GiB with a quarter added: it runs on the rung's own device when it fits there, otherwise on the processor when that much is free (`scores_on`), and when neither, the rung is not offered. An answer over 2 GiB is no bar: it is fetched by two runs (Task 4). Automatic offers: whole on each discrete card, whole on the processor, head on each discrete card, head on the processor. `Card(n)` offers whole then head on that card and nothing else; `Processor` the same on the processor.

- [ ] **Step 1: Write failing tests**, with one 16 GiB discrete card (index 1) that Voxa draws on holding 1 GiB, one integrated card (index 0) of 128 MiB, and 20 GiB free. For 4 096 000 voxels whole: `[card 1 whole, processor whole, card 1 head, processor head]`. For 16 777 216 whole and 8 000 000 head, standard with no cortex areas: `[card 1 head, processor head]`. *(amended)* With the card's budget at 8 GiB and 4 096 000 voxels: the card's rungs are offered with `scores_on` the processor; with 6 GiB free as well, no card rung and no processor rung is offered. With 4 GiB free and the same sizes: `[card 1 head]`. `RunsOn::Card(0)` with 20 GiB free and 4 096 000 voxels: `[card 0 whole, card 0 head]`. `RunsOn::Processor`: only processor rungs. Nothing fits: no rungs, and `why_not` gives the need and what the card and the computer have, in GB to one place. `needs(4_096_000, standard, Card(1))` is `4_096_000 * 1_400 * 5 / 4` until Task 2's figures replace the 1 400.
- [ ] **Step 2:** Run `cargo test -p voxa-models room`. Expected: FAIL.
- [ ] **Step 3:** Implement, with the figures of Conventions.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(models): which device a run fits on, and in what order to try`.

---

### Task 6: Preparing a volume as SynthSeg does

**Files:** `crates/voxa-segment/{Cargo.toml,src/lib.rs,src/prepare.rs,src/prepare/{resample.rs,align.rs,tests.rs},tests/golden/prepare/}`, `tools/golden/{make_golden.py,README.md}`, root `Cargo.toml`, `xtask/src/main.rs`

**Interfaces:**
- Consumes: `voxa_core::Volume`, `Tensor`, `Cut`.
- Produces:

```rust
pub struct Back { /* the order and direction the axes were turned, the padding before each, the grid's size before padding,
                     where a cut began, and the 1 mm grid's dims and placement in the volume's own axes */ }
pub struct Prepared { pub image: Tensor /* [1,x,y,z,1] */, pub back: Back, pub spacing_mm: [f64; 3] }
pub enum PrepareError { TooFewSlices, NothingToSee, TooLarge }
pub fn prepare(volume: &Volume, cut: Cut) -> Result<Prepared, PrepareError>;
pub fn resampled(volume: &Volume) -> Volume;     // 1 mm along its own axes; the same volume when every spacing is within 0.05 mm of 1
```
SynthSeg's rules, in its order:

1. *Resample.* With `f` the spacing in mm along an axis: blur along that axis with a Gaussian of 0.25 / `f` voxels where `f` is 1 or less (kernel out to four of those, reflected at the ends), not at all where it is more; the new count is `ceil(n x f)`; sample `k` is read at `(k - (f - 1) / 2) / f`, held within 0 and `n - 1`, by straight-line blending of its neighbours. The new transform's columns are the old divided by `f`, and its origin moves back by half of `f - 1` new steps along each axis.
2. *Turn.* For each of right, front and top, the volume's axis that lies most along it; the axes are put in that order and any that runs the other way is turned round. No value changes.
3. *Cut* (Task 7), when asked.
4. *Scale.* The 0.5th and 99.5th percentiles, by blending between sorted values; values held between them, then spread from 0 to 1; all 0 when the two are equal.
5. *Pad* each side with zeros to the next multiple of 32, at least 128: half the difference before, rounded down, the rest after.

A side under 4 voxels is `TooFewSlices`, *(amended)* and so is a stack SynthSeg's own program stops on (10 or fewer slices as stored, or 10 or fewer along the image's z once it is at 1 mm and turned); equal percentiles, `NothingToSee`; a grid over 512 on a side, `TooLarge`. A value that is not finite counts as the smallest.

`make_golden.py` runs SynthSeg's own functions on small made-up volumes (seeded blobs and noise, under 40 voxels a side) and writes each case as `case.json` and raw little-endian arrays: `iso` (1 mm, stored as SynthSeg wants it), `sagittal` and `coronal` (other axis orders, one axis reversed), `tilted` (turned 10 degrees about x), `thick` (1 x 1 x 4 mm), `fine` (0.75 mm). The arrays are committed; together they stay under 1 MB.

- [ ] **Step 1: Write failing tests.** Each golden case: `prepare` gives Python's image within 0.00001 where nothing is resampled and within 0.0001 where it is, and the same sizes. Sides of 100, 129 and 160 pad to 128 (14 before), 160 (15 before) and 160. Values 0 to 1000 in order scale so that 5 gives 0 and 995 gives 1. A volume brighter on the patient's left gives an image brighter at low x, stored in each of three axis orders. Three slices is `TooFewSlices`; one value throughout, `NothingToSee`; 600 x 600 x 600 at 1 mm, `TooLarge`. `resampled` of a 1.02 mm volume is that volume.
- [ ] **Step 2:** Run `cargo test -p voxa-segment prepare`. Expected: FAIL.
- [ ] **Step 3:** Write the generator, run it, commit its arrays with it; implement. Add the crate to the workspace and to `COVERED_CRATES`. If the resampled cases cannot be brought within 0.0001, record how close they come and **stop and tell the owner**.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(segment): prepare a volume for SynthSeg, step for step as its own code does`.

---

### Task 7: Cutting to the head

**Files:** `crates/voxa-segment/src/prepare/{head.rs,head_tests.rs}`, `prepare.rs`

**Interfaces:**
- Consumes: Task 6.
- Produces: `prepare(volume, Cut::Head)`; `pub struct Sizes { pub whole: u64, pub head: u64 }` (padded voxel counts) and `pub fn sizes(volume: &Volume) -> Result<Sizes, PrepareError>`.

After the turn and before scaling: with `lo` and `hi` the 0.5th and 99.5th percentiles, the head is the largest connected piece of voxels above `lo + 0.1 x (hi - lo)`; the box is the smallest holding it, with 16 voxels added on every side, kept within the grid. `Back` remembers where the box began; labels outside it come back as 0.

- [ ] **Step 1: Write failing tests.** A bright ball of radius 30 in a 128 cube of faint noise: the box is the ball's with 16 added, to within a voxel, and `sizes` gives 128 cubed for both. The same ball in a 256 cube: `sizes.head` is 128 cubed and `sizes.whole` 256 cubed. A bright speck of three voxels far from the ball does not widen the box. A volume with nothing above the mark is cut to nothing less than the whole grid.
- [ ] **Step 2:** Run `cargo test -p voxa-segment head`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(segment): cut a volume to the head when it must be made smaller`.

---

### Task 8: Between the networks

**Files:** `crates/voxa-segment/src/{find.rs,find/{standard.rs,robust.rs,scores.rs,tests.rs},tests/golden/box/}`, `tools/golden/make_golden.py`

**Interfaces:**
- Consumes: `Tensor`, `Network`, `RunError`, `Role`, `Lists`, `Wanted`, `Canned`.
- Produces:

```rust
pub enum Stage { Preparing, Finding { step: u8, of: u8 }, Cortex, Checking, Tidying }
pub trait Networks { fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError>; }
pub struct Found { pub probabilities: Tensor /* [1,x,y,z,33] */, pub areas: Option<Tensor>, pub scores: Vec<f32> }
pub fn find(image: &Tensor, lists: &Lists, wanted: Wanted, networks: &mut dyn Networks, said: &mut dyn FnMut(Stage)) -> Result<Found, RunError>;
```
- A network that gives two halves of the probabilities has them joined along the last axis as soon as they arrive.
- *Standard:* the network on the image, then on the image reversed along x. The second answer is reversed back along x and its channels put in the order `flip` gives (channel `i` is the old channel `flip[i]`); the two are averaged. `Finding` 1 of 2, then 2 of 2.
- *Robust:* `Robust1` on the image; `Robust2` on its classes; `Robust3` on the image and the cleaned classes. `Finding` 1, 2, 3 of 3.
- *The top structure* at each voxel is the place of the largest of the 33, the first when two are equal.
- *Cortex,* when wanted: the network is given the image and a 1 wherever the top structure's label is one of `cortex`.
- *Scores:* over the voxels whose top structure is neither place 0 nor place 24 (SynthSeg's own rule, by place not by label), from the least place `lo` to one past the greatest `hi` along each axis (the whole grid, up to the box, when there are none): with `e = hi - lo` and `b` the box's side, `lo` moves back by `ceil((b - e) / 2)` and `hi` forward by `floor((b - e) / 2)`; what of that span lies in the grid is copied and the rest is 0. Each place becomes its `score_class`. Of the network's answer the first number is dropped and the rest held between 0 and 1 and rounded to four places.
- One network is open at a time. An answer of the wrong shape, or with a number that is not finite, is `Failed`.

- [ ] **Step 1: Write failing tests** with `Canned` networks on a 4 x 2 x 2 grid and lists of 3 structures whose `flip` swaps the last two. A standard stand-in that puts the image in channel 1 gives, after `find`, the mean of the image and its mirror shared between channels 1 and 2 as the swap requires; the exact twelve numbers are in the test. Robust: `Robust2` was given what `Robust1` returned, `Robust3` the image and what `Robust2` returned, and the stages were said in order. Cortex: the mask given is 1 exactly where the top structure is a cortex label. The box, against Python's own layer for a box of 8: a blob in the middle, one in a corner, one longer than the box, and none. Scores `[9.0, 0.81234, 1.2, -0.1]` come back as `[0.8123, 1.0, 0.0]`. A stand-in answering with the wrong shape, or with a NaN, is `Failed`. A second network is not opened while the first is alive.
- [ ] **Step 2:** Run `cargo test -p voxa-segment find`. Expected: FAIL.
- [ ] **Step 3:** Add the box cases to the generator; implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(segment): the steps between SynthSeg's networks`.

---

### Task 9: Tidying, and the label set

**Files:** `crates/voxa-segment/src/{tidy.rs,pieces.rs,table.rs,label.rs,tidy/tests.rs,tests/golden/tidy/}`, `tools/golden/make_golden.py`, `crates/voxa-labels/src/lib.rs` (export `colour_for`)

**Interfaces:**
- Consumes: `Found`, `Back`, `Lists`, `prepare`, `find`; `voxa_labels::{LabelGrid, Table, Entry, Verdict, Trust, Side, colour_for}`.
- Produces:

```rust
pub const FAIL_BELOW: f64 = 0.65;
pub const THICK_MM: f64 = 3.0;
pub enum ModelChoice { Automatic, Standard, Robust }
pub fn choose(spacing_mm: [f64; 3], choice: ModelChoice) -> bool;                       // true for robust
pub fn largest_piece(mask: &[bool], dims: [usize; 3]) -> Vec<bool>;                    // joined by faces; the first met when two are equal; the mask itself when empty
pub fn tidy(found: &Found, lists: &Lists, back: &Back) -> LabelGrid;
pub fn table(lists: &Lists, wanted: Wanted, scores: &[f32]) -> Table;
pub struct Labelled { pub grid: LabelGrid, pub table: Table, pub notes: Vec<String> }
pub enum LabelError { Prepare(PrepareError), Run(RunError), NothingFound }
pub fn label(volume: &Volume, cut: Cut, lists: &Lists, wanted: Wanted, networks: &mut dyn Networks, said: &mut dyn FnMut(Stage)) -> Result<Labelled, LabelError>;
```
Tidying, on the padded grid: (1) the largest piece of voxels whose probabilities other than the first add to more than 0.25 is kept, and those probabilities are 0 outside it; (2) for each `class` above the lowest, in rising order, the largest piece of voxels where any structure of the class is above 0.25 is kept, and that class's structures are 0 outside it; (3) the padding is taken off, and a cut is put back in its place; (4) each voxel takes the label of its largest probability, the first when equal; (5) where that label is one of `cortex`, it becomes the label of the area there; (6) the axes are turned back, and the grid is given `Back`'s dims and placement, columns fastest.

The table names every structure and area: group "Brain structures" or "Cortex areas", colour `colour_for(label)`, trust `Model`. Its title is "SynthSeg 2.0, standard model" or "SynthSeg 2.0, robust model", with ", with cortex areas" when they were asked for. The verdict fails below 0.65 and names the groups that did. Notes: "The slices of this scan are {n} mm apart, so this is a rough map." when a spacing passes 3 mm; "The scan was cut to the head to fit. Labels at the very edge may differ." for `Cut::Head`. No voxel labelled is `NothingFound`: "The model found no brain in this scan."

- [ ] **Step 1: Write failing tests.** Against SynthSeg's own tidying of made-up probabilities with its real lists (from the generator): two pieces of one structure, a speck far off, and a cortex region with areas; every label the same. `largest_piece` of two pieces of 5 and 3 voxels touching only at a corner keeps the 5; of two of equal size, the first met; of nothing, nothing. A structure placed at low x comes back on the patient's left, for a volume stored in each of two axis orders, and its `Entry` says `Left`. Scores `[0.8, 0.58]` for two groups give a failed verdict naming the second. `choose([1.0, 1.0, 3.5], Automatic)` is true, `([1.0, 1.0, 3.0], Automatic)` false, `(_, Standard)` false. Probabilities that are background everywhere give `NothingFound`. A 6 mm volume has the rough-map note with "6 mm".
- [ ] **Step 2:** Run `cargo test -p voxa-segment`. Expected: FAIL.
- [ ] **Step 3:** Add the tidy cases to the generator; implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(segment): tidy SynthSeg's answer into a label set with its verdict`.

---

### Task 10: Writing a label file, and who made it

**Files:** `crates/voxa-labels/src/{write.rs,write/tests.rs,table.rs,set.rs,lib.rs}`, `crates/voxa-segment/src/table.rs`, `crates/voxa-app/src/{action.rs,labels.rs,reducer/labels.rs,worker/labels.rs}`

**Interfaces:**
- Produces:

```rust
pub struct Made { pub by: String, pub model: String, pub model_sum: String, pub device: String, pub when: String, pub notes: Vec<String> }
// `made: Option<Made>` joins Table, LabelSet, the app's Labels and Action::LabelsRead
pub enum WriteError { TooLarge, NameTwice }
pub fn write_labels(grid: &LabelGrid) -> Result<Vec<u8>, WriteError>;   // gzipped single-file NIfTI-1 with an sform; u16, or i32 when a number passes 65 535
pub fn write_table(table: &Table) -> Result<String, WriteError>;        // the `regions` shape, with `model`, `qc` and a `voxa` object for `made`
```
`read_table(name, &write_table(&t)?)` is `t`. `read_labels(&write_labels(&g)?)` is `g`, its placement to within 0.0001. In the `voxa` object each of `by`, `model`, `device` and `when` is cut at 80 characters, `model_sum` must be 64 hex digits, and at most 8 notes of 300 characters are kept. A table naming one structure twice on one side is `NameTwice`.

- [ ] **Step 1: Write failing tests.** Both round trips, for a grid stored in each voxel order and a table with a left and right pair, a midline entry, a failed verdict and a `Made` with two notes. A table read from 0.4.0's own test files still reads, with `made` `None`. A `voxa` object with a 500-character `by`, a `model_sum` of "x" and 20 notes reads with `by` cut, no sum and 8 notes. A number of 70 000 is written as i32 and read back.
- [ ] **Step 2:** Run `cargo test -p voxa-labels write`. Expected: FAIL.
- [ ] **Step 3:** Implement; carry `made` through the app's label actions, where it is `None` for every file opened today, and through `voxa-segment`'s table, where it is `None` until the worker of Task 13 fills it.
- [ ] **Step 4:** Run `cargo test --workspace --all-features`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(labels): write a label file and its table, and say who made a label set`.

---

### Task 11: The whole path against Python

**Files:** `crates/voxa-segment/tests/with_models.rs`, `crates/voxa-segment/src/testing.rs`, `xtask/src/main.rs`

**Interfaces:**
- Consumes: everything above; the `check/` folder of Task 2.
- Produces: `cargo xtask check-models`. With either variable unset it prints `check-models: skipped, VOXA_MODELS and VOXA_RUNTIME are not both set` and succeeds; otherwise it runs the ignored tests of `voxa-models` and `voxa-segment` one at a time.

- [ ] **Step 1: Write the ignored tests.** For the processor and each card not `Software`: from `check/image.f32`, the standard model and the robust model with cortex areas, tidied, against `standard.u16` and `robust.u16`, held to the agreement marks; scores against `case.json`'s. `prepare` of `check/volume.f32` gives `image.f32` within 0.00001, and of `thick.f32` gives `thick_image.f32` within 0.0001. A structure of the left in `robust.u16` lies on the patient's left of its right-hand twin in Voxa's answer. Each test prints its device, its seconds and its peak memory.
- [ ] **Step 2:** Run `cargo xtask check-models` with the folders set. Expected: PASS on every device. Record the times.
- [ ] **Step 3:** Measure the cut: labels of `volume` with `Cut::Head` against `Cut::Whole`. Record the least and mean Dice. **If any structure over 500 voxels falls under 0.99, stop and tell the owner**, since the note on a cut result must then say more than it does.
- [ ] **Step 4:** Run `cargo xtask check` with the variables unset. Expected: PASS, the ignored tests not run.
- [ ] **Step 5:** Commit: `test(segment): the whole path held to SynthSeg's own answer, where the model files are`.

---

### Task 12: Labelling in the state

**Files:** `crates/voxa-app/src/{labelling.rs,action.rs,effect.rs,state.rs,reducer.rs,reducer/{labelling.rs,labelling_tests.rs}}`

**Interfaces:**
- Consumes: `ModelChoice`, `Stage`, `Wanted`, `RunsOn`, `Card`, `Made`.
- Produces:

```rust
pub enum Ready { Unknown, Yes, No(String) }
pub struct Labelling { pub ready: Ready, pub cards: Vec<Card>, pub drawing_on: Option<String>, pub choice: ModelChoice, pub cortex: bool,
                       pub runs_on: RunsOn, pub keep: bool, pub models: Option<PathBuf>, pub runtime: Option<PathBuf>,
                       pub running: Option<Stage>, pub on: Option<SeriesId>, pub kept: Option<String> /* the date of a kept result */ }
pub fn can_label(state: &AppState) -> Result<(), String>;          // why not, in a sentence
pub fn what_will_run(state: &AppState) -> String;                   // the line under the button
// Action: LabelScan { held: u64 }, LabellingStage(Stage), LabellingFailed(String), StopLabelling, LabellingLooked { ready: Ready, cards: Vec<Card> },
//         DrawingOn(String), SetModelChoice(ModelChoice), SetCortex(bool), SetRunsOn(RunsOn), SetKeep(bool), SetModelsFolder(PathBuf), SetRuntimeFolder(PathBuf)
// Effect: LookForLabelling { models: Option<PathBuf>, runtime: Option<PathBuf> }, LabelScan { series: SeriesId, wanted: Wanted, runs_on: RunsOn, held: u64, keep: bool }, StopLabelling
```
`cortex` and `keep` start true; `choice` and `runs_on` at Automatic. `LabelScan` does something only when `can_label` allows: ready, nothing running, and the scene's series has a volume. It sets `running` to `Preparing` and `on` to that series. A result arrives as `LabelsRead` with `made`; it is taken in as the labels of the study of `on`, and dropped if the scene is now on another study. Opening another export (`OpenPath`), opening a label file, or closing the labels while a run is on asks for `StopLabelling`. Whether a run fits in memory is found when it starts, from the volume itself, so that is said as the run's failure and not on a greyed button: a small departure from the spec's section 5. `StopLabelling` clears `running` and posts "Labelling was stopped." `LabellingFailed` clears it and posts the sentence as an error. Changing a folder asks to look again.

`what_will_run`: "{Standard|Robust} model{ with cortex areas}, on {the graphics card (NAME)|the processor}." With Automatic it adds "Robust was picked because the slices are more than 3 mm apart." or "Standard was picked because the slices are no more than 3 mm apart."

- [ ] **Step 1: Write failing tests**, by actions and state alone. `LabelScan` when ready with a volume: `running` set and one `LabelScan` effect carrying the choices. Not ready, or already running, or no volume: nothing changes and no effect. A second `LabelScan` while running: no second effect. `LabelsRead` with `made` after the scene moved to another study: labels unchanged, `running` cleared. `OpenPath`, `OpenLabels` and `CloseLabels` while running each give `StopLabelling`. Each `Set…` changes its field. `what_will_run` for 1 mm and 6 mm slices with Automatic, and for each choice by hand.
- [ ] **Step 2:** Run `cargo test -p voxa-app labelling`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): labelling in the state: what is chosen, what is running, what came back`.

---

### Task 13: The worker that labels

**Files:** `crates/voxa-app/src/worker/{labelling.rs,labelling_tests.rs}`, `worker.rs`, `Cargo.toml`

**Interfaces:**
- Consumes: `label`, `sizes`, `rungs`, `why_not`, `Runtime`, `read_model`, `cards`, `Networks`, `Stop`; `Workers::spawn`, `shielded`, the label set the workers keep.
- Produces: the three effects of Task 12 carried out. `Workers::new` takes, for tests, a maker of `Networks` in place of the runtime's.

`LookForLabelling` looks for the runtime in a `runtime` folder beside the executable and then in the folder chosen, and for the model in the folder chosen and then in a `models` folder beside the executable. It reads the model folder, loads the runtime (once in the program's life) and reports `LabellingLooked`: `No` with the first thing wrong, in the error's own sentence, followed by "docs/models.md says what goes where." A network's own words for a failure go to the log, with no path, and the person is told "The model could not be run on {device}." `LabelScan` takes the volume kept for the series, works out `sizes` and the `Room` (`cards()`, the card named in `drawing_on`, `held`, free memory from `sysinfo`), and the rungs; with none, it reports `LabellingFailed(why_not(..))`. It tries each rung in turn: on `NoRoom` or `DeviceLost` the next one, and the result's notes gain "Run on the processor: the graphics card did not have room." or "Run on the processor: the graphics card stopped answering."; any other error ends the run. *(amended)* The scores network is opened on the rung's `scores_on`; when that is the processor and the rest ran on a card, the notes gain "The model checked its own work on the processor: the graphics card did not have room for that step." The label set is made with `label_set`, kept as the workers' labels, and reported as `LabelsRead` with `made` (`by` "Voxa" and the version; `model` the table's title; `device` the card's name or "the processor"; `when` today's date). Stages are reported as they are said. A run overtaken, stopped, or whose series has gone keeps and reports nothing. A panic becomes `LabellingFailed`.

- [ ] **Step 1: Write failing tests** with stand-in networks on a made-up volume. A run reports its stages in order and then `LabelsRead` with three parts and `made`. A first rung that answers `NoRoom` is followed by the second, and the note is there. `StopLabelling` after the first stage: nothing more is reported, nothing kept. `DropLabels` during a run drops its result. A stand-in that panics gives `LabellingFailed` and the next run works. A model folder with a wrong sum gives `LabellingLooked` with `No` and a sentence with no path.
- [ ] **Step 2:** Run `cargo test -p voxa-app worker::labelling`. Expected: FAIL.
- [ ] **Step 3:** Implement. Wait for events in tests, never for time.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): a worker labels the series in the scene, falling to the next device when one fails`.

---

### Task 14: What is kept, and what is chosen

**Files:** `crates/voxa-app/src/{kept_labels.rs,choices.rs,kept_labels/tests.rs}`, `worker/{labelling.rs,kept.rs}`, `reducer/labelling.rs`, `action.rs`, `effect.rs`

**Interfaces:**
- Consumes: `write_labels`, `write_table`, `settings::folder`, `views::series_key`, the `ReadLabels` effect.
- Produces:

```rust
pub fn keep(folder: &Path, key: &str, labels: &[u8], table: &str) -> std::io::Result<()>;
pub fn kept(folder: &Path, key: &str) -> Option<(PathBuf, PathBuf)>;      // the label file and its table, when both are whole
pub fn forget(folder: &Path, key: &str) -> std::io::Result<()>;
pub const CHOICES_VERSION: u32 = 1;
pub enum RunsOnKept { Automatic, Processor, Card(String) }                 // a card by its name
pub struct Choices { pub choice: ModelChoice, pub cortex: bool, pub runs_on: RunsOnKept, pub keep: bool, pub models: Option<PathBuf>, pub runtime: Option<PathBuf> }
pub fn load_choices(folder: &Path) -> Choices;  pub fn save_choices(folder: &Path, choices: &Choices) -> std::io::Result<()>;
// Action: KeptFound { series: SeriesId, when: String }, ShowKept, DeleteKept, SaveLabelsAs(PathBuf)
// Effect: FindKept { series }, ForgetKept { series }, SaveLabels { to: PathBuf }, StoreChoices(Choices)
```
A result is kept as `labels/<key>.nii.gz` and `labels/<key>.json` under the settings folder. Each is written beside itself as `.part` and renamed, the table last; a result counts as kept only when both are there. `choices.json` carries its version; a card is remembered by name, since numbers move. The two folders in it are the only paths Voxa keeps, and say nothing of any scan. With `keep` off nothing is written, and what was kept before stays until deleted. A result that cannot be kept is still shown, with the warning "These labels could not be kept on this computer. They are shown now and will be gone when Voxa closes."

When a series' volume is ready the reducer asks `FindKept`; `KeptFound` sets `labelling.kept`. `ShowKept` asks `ReadLabels` for the two files. `DeleteKept` asks `ForgetKept` and clears it. `SaveLabelsAs(path)` writes the labels open now to `path` and their table beside it under the same name with `.json`.

- [ ] **Step 1: Write failing tests**, in temporary folders. `keep` then `kept` gives both paths, and reading them gives the set back with its `made`. A `.part` file alone is not kept. `forget` removes both. Choices round-trip; a file of version 2, or of noise, gives the defaults; a card kept by a name no card has now gives Automatic. A worker's run with `keep` on leaves the two files; with it off, none. A folder that cannot be written gives the warning and the result. `ShowKept` leads to `ReadLabels` for those two paths. No kept file holds a path or a name of a scan.
- [ ] **Step 2:** Run `cargo test -p voxa-app kept`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): a result is kept for its series, shown again, deleted and saved`.

---

### Task 15: The strip in the Parts tab

**Files:** `crates/voxa-app/src/ui/{labelling.rs,labelling/tests.rs,parts.rs,help.rs,capture.rs}`, `ui/scene/captions.rs`

**Interfaces:**
- Consumes: `Labelling`, `can_label`, `what_will_run`, the actions of Tasks 12 and 14, `Made`.
- Produces: `pub fn strip(ui: &mut Ui, state: &AppState, held: u64, actions: &mut Vec<Action>)`, drawn at the top of the Parts tab; `VOXA_CAPTURE_SHOW` gains `labelling` (the strip, ready) and `labelled` (a result made by Voxa).

With no labels open: the button **Label this scan**, the line of `what_will_run` beneath it, and a small menu holding Automatic / Standard / Robust, **Cortex areas**, **Runs on** (Automatic, each card by name with "usually slower than the processor" beside an integrated one, Processor), **Keep results on this computer**, and the two folders. Greyed, with `can_label`'s sentence, when it cannot run. When a result is kept for the series: "Labels made on {when} are kept for this series." with **Show**, **Make again** and **Delete**. While running: the stage ("Preparing the scan", "Finding the structures, {n} of {m}", "Dividing the cortex", "The model is checking its work", "Tidying up"), **Stop**, and "The 3D view may be slow while the graphics card is busy." With labels that carry `made`: "Labels made by Voxa with {model}, on {device}, {when}. Not for diagnostic use." in place of "Made by another program…", each note on its own line beneath, and **Save as…**. The Parts look's caption says the same of its solids. Labels with no `made` are shown exactly as in 0.4.0.

- [ ] **Step 1: Write failing tests**, sweeping the strip as the Parts list is swept today. Ready with no labels: the button is found and gives `LabelScan`. Not ready: the button is greyed and the sentence shown. Running: the stage's words and a Stop that gives `StopLabelling`; no button. Kept: the line and its three buttons, each giving its action. Labels with `made`: the caption with the model, device and date, and both notes; with none: 0.4.0's sentence. The menu gives each `Set…` action. An integrated card's entry carries its warning.
- [ ] **Step 2:** Run `cargo test -p voxa-app ui::labelling`. Expected: FAIL.
- [ ] **Step 3:** Implement. Capture the strip in each state at both window sizes and read the pictures. Add the button and Stop to the keys-and-mouse list.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): Label this scan, in the Parts tab`.

---

### Task 16: When the graphics device is lost

**Files:** `crates/voxa-render/src/` and `crates/voxa-app/src/ui/` as the finding requires; `.superpowers/sdd/labelling-findings.md`

A driver reset during a run takes Voxa's own device with it. Do not provoke a real reset on the owner's machine; lose the device in a test by destroying it.

- [ ] **Step 1:** Read what `eframe` and `egui-wgpu` do when their device is lost, and what each of Voxa's pipelines does when asked to draw on a destroyed device. Record it.
- [ ] **Step 2: Write failing tests** (returning early where there is no device): after the device is destroyed, drawing a slice, a scene, a plane and the labels each returns without a panic; annotations, saved views and a kept result written before the loss read back whole afterwards.
- [ ] **Step 3:** Implement what the tests need. If Voxa cannot draw again without being restarted, it posts once: "The graphics card stopped answering. Close Voxa and open it again. Nothing you made has been lost."
- [ ] **Step 4:** Run `cargo test --workspace --all-features`. Expected: PASS.
- [ ] **Step 5:** Commit: `fix(app): a lost graphics device stops no work and loses nothing`.

---

### Task 17: Acceptance and release 0.5.0

**Files:** `crates/voxa-app/examples/{write_phantom/,check_labelling.rs}`, `docs/{acceptance-0.5.0.md,architecture.md,models.md}`, `docs/adr/0005-voxa-runs-a-labelling-model.md`, `docs/superpowers/specs/2026-10-05-voxa-design.md`, `README.md`, `CHANGELOG.md`, `crates/voxa-labels/src/lib.rs`, the workspace version

- [ ] **Step 1:** `write_phantom` gains `head CHECK_FOLDER OUT`: the converter's made-up head written as a study, so there is something with a brain's shape to label that is nobody's. `check_labelling EXPORT`: with no window, labels the largest series with a 3D view on every rung offered and prints for each the grid's size, the rung, the seconds, the number of parts and whether the verdict passed, and nothing else.
- [ ] **Step 2:** Run `cargo xtask check` and `cargo xtask coverage` with the variables unset, then `cargo xtask check-models` with them set. Time the made-up 160 cube from the button to the parts on a 16 GB discrete card after its first run: under 5 s for the standard model, under 30 s for the robust model with cortex areas. Label the made-up head (192 x 256 x 256 once padded): it completes on the card, or falls to the next rung and says so.
- [ ] **Step 3:** Acceptance on the made-up head, recorded in `docs/acceptance-0.5.0.md`: labelled from the Parts tab with no Python on the path; the verdict shown; Voxa closed and opened, the kept result offered and shown; the `runtime` folder taken away, and everything of 0.4.0 still working with the button greyed and saying why. What a person must try by hand is listed as not checked. The owner tries their own scans; nothing of what they see is written down.
- [ ] **Step 4:** Decision record 0005 (Voxa runs a labelling model and ships none; it replaces the fifth point of record 0004; the runtime, how it is loaded, telemetry off, the one module with `unsafe`, DirectML frozen and what would replace it; the findings). The README's "Voxa shows labels; it does not make them" and the top of `voxa-labels` say which labels Voxa makes and which it does not, and the README names ONNX Runtime and DirectML with their licences. `docs/architecture.md` gains the two crates and the path of a run. In the design: the crate table of A3, the milestone 4 row of A4 ("provided by the user"), the first risk of A7 (answered). `CHANGELOG.md` with 0.5.0, the workspace version to `0.5.0`, and the three differences from SynthSeg's own program (spec, section 7) in `docs/models.md`.
- [ ] **Step 5:** Commit: `chore: acceptance record and 0.5.0 changelog`, then `git tag v0.5.0`. Do not push, and do not merge into `main`.

---

## Out of scope

Any model but SynthSeg 2.0; the parts outside the brain; a brain-only view; fetching model files or the runtime; CUDA, the WebGPU provider, Windows ML, half precision, tiles; graphics cards on Linux or macOS; joining the robust networks into one file; hand corrections; more than one label set at once; a settings screen.
