# Voxa plugins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** People add their own ONNX label-map models to Voxa as checked plugins, run them on a scan in a capped helper process, and see the result framed as "Added by you, not part of Voxa".

**Architecture:** A new crate, `voxa-plugins`, reads and checks a plugin folder (`plugin.json` and an ONNX file read by a small protobuf walker). It prepares a scan by a fixed menu generalised from `voxa-segment`, cuts it into a fixed whole box or fixed blocks, and finishes the network's answer into a label set. Networks run only in `voxa.exe --plugin-helper`, a child process in a Windows job object (in `voxa-models`), spoken to over pipes. `voxa-app` adds the state, workers and a small UI that the redesign's Tools tab will take over.

**Tech Stack:** Rust 2024 (1.99), `ort` `=2.0.0-rc.13` (ONNX Runtime 1.24.4, DirectML), `windows` 0.62.2, serde/serde_json, sha2, egui/eframe 0.36.

**Spec:** `docs/superpowers/specs/2026-10-31-voxa-plugins-design.md` (approved 2026-10-31). Section numbers below (§) are the spec's.

## Global Constraints

- No new crate dependency. `windows` gains features only: `Win32_Foundation`, `Win32_Security`, `Win32_System_JobObjects`, `Win32_System_Threading` (check the exact names in windows 0.62.2's `Cargo.toml` under `~/.cargo/registry/src`).
- `voxa-plugins`: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, and the same `cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))` as `voxa-segment`. `unsafe` only in `voxa-models/src/process.rs` (new) beside `cards.rs`, every block with a `SAFETY` comment.
- The gate: `cargo xtask check` (fmt, clippy pedantic as errors, files ≤ 400 lines, functions ≤ 60 lines, tests, cargo deny, docs). Split files early. Each task ends with `cargo test -p <crate>` and `cargo clippy -p <crate> --all-targets --all-features -- -D warnings` green. The whole gate runs at Tasks 12, 19 and 21.
- `ort`: never `Tensor::to`. DirectML: one run at a time per session, sequential execution, no memory pattern (as `runtime.rs` does).
- Every sentence a person sees ends with a full stop and names no file path. The fixed words of the spec are copied exactly (listed under "Words" below).
- Made-up values only: nothing from the owner's scans in code, tests, docs or commit messages. No MindGrab weights, sums or converter in the repository.
- Downloads: only MindGrab's `model.json` (68 331 bytes) and `model.bin` (584 948 bytes) from `github.com/neuroneural/brainchop`, folder `public/models/mindgrab` (approved by the owner 2026-10-31). Nothing else.
- Python only as `G:\dev\projects\dicom-analysis\.venv-onnx\Scripts\python.exe -I`, never bare `py` or `python`.
- Never push, merge, or touch `main`/`release`. Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- SynthSeg's golden tests (`crates/voxa-segment/tests/golden`, every `voxa-segment` test) pass unchanged at every commit.

### Words (exact, from the spec)

| Key | Text |
|---|---|
| CARD | `Added by you, not part of Voxa. Voxa has not checked what this model does. Not for diagnostic use.` |
| BANNER | `Added by you, not part of Voxa. {name} {version}, made by {made_by}. Source: {source}. Licence: {licence}. {limits, each as written, joined by one space} Not for diagnostic use.` (no limits: one space before `Not`) |
| MODALITY | `This plugin is for {modalities joined by " or "} scans; this scan is {modality}.` / no modality: `This plugin is for {…} scans; this scan does not say what kind it is.` |
| WARN | `This plugin expects {accepted joined by " or "}; this scan's description says {names read joined by " and "}.` |
| STAMP_MISMATCH | `Run after a warning: this plugin expects {…}; this scan's description says {…}.` |
| STAMP_UNSAID | `Contrast not confirmed: the scan's description does not say.` |
| NOTE_MATCHED | `The scan's description says {name read}, as this plugin expects.` |
| NOTE_SPACING | `The voxels of this scan are {widest} mm apart; this plugin states it was made for up to {thickest} mm.` (mm written as `in_mm` in `voxa-segment/src/label.rs`) |
| NOTE_BOX | `The plugin's box of {a} × {b} × {c} mm left out part of the scan, up to {n} mm beyond the box.` |
| NOTE_BLOCKS | `Run in blocks: near the edges between blocks, labels may differ from the model's own program.` |
| CHANGED | `This plugin's files have changed since it was added. Remove it and add it again.` |
| NOT_YET | `This version of Voxa cannot run plugins that give {one number / a value map / an alignment} yet.` |
| UNNAMED | `The plugin gave labels its description does not name.` |
| NOTHING | `The plugin labelled nothing in this scan.` |
| ENDED | `The plugin stopped without finishing. Voxa is unaffected.` |
| FLAT | `This series holds no values that differ, so there is nothing in it for the plugin to see.` |

## Review Focus

1. **A tilted scan** (oblique axes): each axis code must still pick the scan axis nearest the letter and put an off-centre bead back where it was. Test in Task 5.
2. **A scan larger than the whole box along one axis and smaller along another** (cut and surroundings at once): labels land in the right voxels and NOTE_BOX names the overhang. Test in Task 5 and Task 8.
3. **A description listing one file twice, or two names that differ only in case** (one file on Windows): refused before any copy. Test in Task 2.
4. **The helper dying mid-run, or Stop pressed while Voxa waits on its pipe**: no hang, ENDED or "stopped", Voxa running. Test in Task 11 and Task 18.
5. **A flat scan** (all equal, or equal percentiles, or zero standard deviation): FLAT, not a division by zero. Test in Task 6.

---

## File structure

`crates/voxa-plugins/` (new):

| File | Holds |
|---|---|
| `src/lib.rs` | crate docs, lints, re-exports |
| `src/describe.rs`, `src/describe/{fields.rs, input.rs, output.rs, text.rs}` | `plugin.json` format 1 → `Description` |
| `src/folder.rs` | a folder's checks → `Offered`; plain names, reparse points, sizes, sums |
| `src/onnx.rs`, `src/onnx/{wire.rs, graph.rs}` | the ONNX structure walker → `NetworkFacts` |
| `src/fit.rs` | network against description → `Wiring` |
| `src/grid.rs`, `src/grid/sample.rs` | the plugin grid, boxes, linear sampling, placement back |
| `src/scaling.rs`, `src/scaling/stats.rs` | statistics and the four scalings |
| `src/blocks.rs` | block starts and nearest-centre ownership |
| `src/finish.rs`, `src/label_map.rs` | a block's answer → labels; the whole map → `LabelGrid` + `Table` |
| `src/accepts.rs`, `src/accepts/words.rs` | modality and contrast decisions, notes |
| `src/helper.rs`, `src/helper/{frame.rs, serve.rs, client.rs, opener.rs}` | the pipe messages, both ends |
| `src/measure.rs` | `measured.json`, places a run fits |
| `src/run.rs` | one run end to end, over a `HelperStarter` |
| `src/bin/voxa-plugin-helper.rs` | the helper as a binary, for this crate's tests |
| `src/testkit.rs`, `src/testkit/onnx.rs` | (feature `testkit`) plugin folders and ONNX bytes for tests |
| `tests/with_runtime.rs` | ignored tests needing `VOXA_RUNTIME` |
| `tests/with_plugin.rs` | the ignored MindGrab check |

Changed elsewhere: `voxa-models` (`model.rs` names/sums made public, `runtime.rs` free dimensions by name and output choice, `tensor.rs` integer outputs, new `process.rs`, new bin `voxa-test-eater`), `voxa-segment` (`prepare/align.rs` `Turn` public with a direction), `voxa-labels` (`made.rs` `Added`, `words.rs` banner), `voxa-app` (`args.rs`, `main.rs`, `action.rs`, `effect.rs`, `plugins.rs` + `plugins/`, `reducer/plugins.rs`, `worker/plugins.rs` + `worker/plugins/`, `ui/plugins/`, one call in `ui/parts.rs`, captions, `parts_table.rs`), `xtask` (crate lists), `Cargo.toml` (member, dependency), docs.

---

### Task 1: The description, format 1

**Files:**
- Create: `Cargo.toml` member and `voxa-plugins = { path = "crates/voxa-plugins" }`; `crates/voxa-plugins/Cargo.toml` (deps: `voxa-core`, `voxa-labels`, `voxa-models`, `voxa-segment`, `serde`, `serde_json`, `sha2`, `thiserror`, `tracing`; dev: `tempfile`, `voxa-models` with `testkit`; feature `testkit = ["dep:tempfile"]`); `src/lib.rs`; `src/describe.rs`; `src/describe/{fields.rs, input.rs, output.rs, text.rs}`; tests `src/describe/tests.rs`, `src/describe/refusal_tests.rs`
- Modify: `xtask/src/main.rs` (`COVERED_CRATES` gains `voxa-plugins`, array length 7)

**Interfaces:**
- Produces (all `pub`, re-exported at the crate root):
  - `const PLUGIN_FORMAT: u32 = 1;`
  - `fn read_description(text: &[u8]) -> Result<Description, DescribeError>`
  - `struct Description { id, name, version, made_by, source, licence, about: String, limits: Vec<String>, kind: Kind, files: Vec<Listed>, network: String, accepts: Accepts, input: Input, output: Output }`
  - `enum Kind { LabelMap, Number, ValueMap, Alignment }`; `impl Description { fn runnable(&self) -> Result<(), DescribeError> }` (NOT_YET for any kind but `LabelMap`)
  - `struct Listed { name: String, bytes: u64, sha256: String }` (sha256 lower-cased)
  - `struct Accepts { modality: Vec<String>, contrast: Contrasts, body_part: String, thickest_mm: Option<f64> }`; `enum Contrasts { Any, Only(Vec<Contrast>) }`; `enum Contrast { T1, T1In3d, T2, Pd, Flair, T2Star, Diffusion, AdcMap, Swi, Stir, Tof }` with `fn word(self) -> &'static str` giving `T1`, `3D T1`, `T2`, `PD`, `FLAIR`, `T2*`, `Diffusion`, `ADC map`, `SWI`, `STIR`, `Angiography (TOF)`; JSON accepts those words and `TOF`
  - `struct Input { spacing_mm: [f64; 3], axes: [Toward; 3], layout: Layout, fit: Fit, outside: Outside, scaling: Scaling }` (`Toward` from `voxa_segment`, Task 5; until then define it here and move it in Task 5)
  - `enum Layout { ChannelsFirst, ChannelsLast }`; `enum Fit { Whole([usize; 3]), Blocks { size: [usize; 3], step: [usize; 3] } }`; `enum Outside { Lowest, Zero }`
  - `enum Scaling { Percentiles { low: f64, high: f64, over: Over }, ZScore { voxels: Voxels, over: Over }, Fixed { clip: [f64; 2], subtract: f64, divide: f64 }, None }`; `enum Over { Scan, Grid }`; `enum Voxels { All, AboveZero }`
  - `enum Output { Labels(LabelOutput), Number(NumberOutput), ValueMap(ValueOutput), Alignment(AlignOutput) }`
  - `struct LabelOutput { gives: Gives, from: Option<String>, keep_largest_piece: bool, names: Vec<Name> }`; `enum Gives { Scores { channels: Vec<u16> }, Chance { label: u16, threshold: f32 }, Labels }`; `struct Name { id: u16, name: String, side: voxa_labels::Side, group: String, colour: Option<[u8; 3]> }`
  - `struct NumberOutput { name: String, unit: String, decimals: u8, range: Option<[f64; 2]>, from: Option<String> }`; `struct ValueOutput { unit: String, range: [f64; 2], from: Option<String> }`; `struct AlignOutput { gives: Moves, from: Option<String> }`; `enum Moves { Field { unit: FieldUnit }, Matrix }`; `enum FieldUnit { Mm, Voxels }`
  - `enum DescribeError` (`thiserror`, each a sentence): `NotJson` "This folder's plugin.json is not a description Voxa can read.", `Format(u32)` "This plugin is in format {0}, which this version of Voxa cannot read.", `Field { field: String, why: &'static str }` "The description's {field} {why}.", `NotYet(Kind)` NOT_YET
- Rules (§6.1, §8, §9.1): serde with `deny_unknown_fields`; `format` read first as `Voxa`'s `model.rs` reads `Head`; no default for any field of `input` or `output` except `colour`, `from`, `body_part`, `thickest_mm`; text fields refused (not stripped) when longer than their limit in characters or holding a control or format character (copy `FORMAT` and `is_format` from the redesign branch `crates/voxa-app/src/naming/printable.rs` into `text.rs`; one copy is kept when the branches merge); `id` 3–64 of `a-z 0-9 . -`; `axes` one letter of each pair; spacings and steps > 0; `1 ≤ step ≤ size`; percentiles `0 ≤ low < high ≤ 100`; `divide ≠ 0`; `over: grid` only with `Fit::Whole` and `Outside::Lowest`; `channels` non-empty, every non-zero channel label and `Chance.label` present in `names`; names' ids 1–65 535 and unique; at most 8 limits.

- [ ] **Step 1: Write the failing tests** in `src/describe/tests.rs`: `the_example_of_the_spec_reads` (§6.2's JSON with a 64-digit made-up sum; assert `kind == Kind::LabelMap`, `input.axes == [Left, Inferior, Anterior]`, `fit == Fit::Whole([256, 256, 256])`, `output` gives `Scores { channels: vec![0, 1] }`); `each_later_kind_reads_and_is_not_yet_runnable` (three made-up descriptions; `runnable()` gives `NotYet` with the exact NOT_YET words); in `refusal_tests.rs` one test per rule above, each asserting the field named in the error, e.g. `a_description_without_a_scaling_names_input_scaling`, `an_unknown_field_is_refused`, `a_name_with_a_right_to_left_mark_is_refused`, `grid_statistics_need_a_whole_box_and_the_lowest_outside`, `a_channel_label_without_a_name_is_refused`, `format_two_is_refused_whatever_else_it_holds`.
- [ ] **Step 2:** `cargo test -p voxa-plugins` fails to compile (no crate).
- [ ] **Step 3:** Create the crate and implement `read_description` and the types; split so no file passes 400 lines.
- [ ] **Step 4:** `cargo test -p voxa-plugins` and clippy pass.
- [ ] **Step 5: Commit** `feat(plugins): read a plugin's description, format 1`

### Task 2: Checking a plugin folder's files

**Files:**
- Modify: `crates/voxa-models/src/model.rs` (make `is_plain_name` and a file-sum function public), `crates/voxa-models/src/lib.rs`
- Create: `crates/voxa-plugins/src/folder.rs`, `src/folder/tests.rs`, `src/testkit.rs`

**Interfaces:**
- Consumes: `read_description`, `Description`, `Listed` (Task 1).
- Produces:
  - `voxa_models::is_plain_name(name: &str) -> bool`; `voxa_models::file_sum(path: &Path, limit: u64) -> std::io::Result<String>` (today's `sum_of` over a file, lower-case hex)
  - `voxa_plugins::DESCRIPTION: &str = "plugin.json"`
  - `struct Offered { description: Description, sum: String /* SHA-256 of plugin.json */, folder: PathBuf, network: NetworkFacts, wiring: Wiring }` (the last two filled by Tasks 3 and 4; this task leaves them out and adds them there)
  - `fn check_folder(folder: &Path, supported: Supported) -> Result<Offered, PluginError>`
  - `enum PluginError` (sentences): `NotFound` "There is no folder at the place given.", `NoDescription` "That folder holds no plugin.json, or one too long to be a description.", `Describe(DescribeError)`, `NotPlain(String)` "The description names a file outside its folder.", `Twice(String)` "The description lists one file twice.", `Missing(String)` "The plugin is missing a file its description lists.", `NotAFile(String)` "A file the description lists is a link or not a plain file.", `WrongSize(String)` / `WrongSum(String)` "A file is not the one the description lists, so it may be damaged.", `TooLarge` "The plugin's files are larger than Voxa takes: 2 GiB a file, 4 GiB together.", `Network(NetworkError)` (Task 3), `Fit(String)` (Task 4). The `String`s are file names for the log only; `Display` never shows them.
  - testkit (`feature = "testkit"`): `fn plugin_folder(description: &serde_json::Value, files: &[(&str, &[u8])]) -> tempfile::TempDir` writing the files and a `plugin.json` whose `files` list is filled in with true sizes and sums unless the test overrides it.
- Rules (§7.1 steps 1 and 4): `plugin.json` ≤ 1 MiB; names compared for duplicates in lower case; a file is refused if `symlink_metadata` says link, or on Windows its attributes hold `FILE_ATTRIBUTE_REPARSE_POINT` (0x400, via `std::os::windows::fs::MetadataExt`); each ≤ 2 GiB, together ≤ 4 GiB; `network` must be listed and end `.onnx` (any case).

- [ ] **Step 1: Write the failing tests** in `folder/tests.rs`: `a_good_folder_is_offered_with_the_sum_of_its_description`; `a_name_with_a_separator_or_dots_is_refused` (`"../x.onnx"`, `"a\\b.onnx"`, `"c:x"`); `two_names_differing_only_in_case_are_refused` (Review Focus 3); `a_file_one_byte_short_is_the_wrong_size`; `a_file_with_one_byte_changed_has_the_wrong_sum`; `a_link_is_refused` (`#[cfg(windows)]` symlink made with `std::os::windows::fs::symlink_file`; skip with a message when the system refuses to make one); `files_over_the_limits_are_refused` (limits taken as parameters of an internal function so the test uses small numbers); `no_error_sentence_names_a_file` (every variant's `to_string()` lacks the name used).
- [ ] **Step 2:** Run `cargo test -p voxa-plugins folder` and see them fail.
- [ ] **Step 3:** Implement `check_folder` (steps 1–4 of §7.1) and the two `voxa-models` functions (keep `voxa-models`' own tests green).
- [ ] **Step 4:** Tests and clippy pass for both crates.
- [ ] **Step 5: Commit** `feat(plugins): check a plugin folder's files against its description`

### Task 3: Reading the network's structure

**Files:**
- Create: `crates/voxa-plugins/src/onnx.rs`, `src/onnx/wire.rs`, `src/onnx/graph.rs`, `src/onnx/tests.rs`, `src/onnx/limit_tests.rs`, `src/testkit/onnx.rs`
- Modify: `crates/voxa-models/src/lib.rs` (add `Supported`)

**Interfaces:**
- Produces:
  - `voxa_models::Supported { ir_version: i64, opset: i64 }` and `pub const SUPPORTED: Supported` (the highest IR version and default-domain opset ONNX Runtime 1.24 runs, from its documentation's compatibility table; write the source in the doc comment)
  - `fn read_network(path: &Path, listed: &[Listed], supported: Supported) -> Result<NetworkFacts, NetworkError>`
  - `struct NetworkFacts { inputs: Vec<Port>, outputs: Vec<Port> }` (graph inputs that are not initializers, in order); `struct Port { name: String, element: Element, dims: Vec<Dim> }`; `enum Element { F32, U8, I32, I64, Other(i32) }`; `enum Dim { Fixed(u64), Named(String), Unknown }`
  - `enum NetworkError` (sentences): `NotANetwork` "The network file is damaged or is not an ONNX network.", `Domain(String)` "The network uses operators outside standard ONNX, which Voxa does not run.", `Functions` "The network holds functions of its own, which Voxa does not run.", `Outside` "The network keeps data in a file its description does not list.", `TooNew` "The network needs a newer ONNX Runtime than the one Voxa uses.", `TooDeep` "The network is nested too deeply or is too large for Voxa to check."
  - testkit: `struct NetSpec { ir_version: i64, opsets: Vec<(String, i64)>, nodes: Vec<NodeSpec>, inputs: Vec<PortSpec>, outputs: Vec<PortSpec>, initializers: Vec<InitSpec>, functions: usize }` and `fn network_bytes(spec: &NetSpec) -> Vec<u8>`; `fn conv_network(input: &PortSpec, out_channels: usize, weights: &[f32], bias: &[f32]) -> Vec<u8>` (one 1×1×1 `Conv`, opset 17, IR 8, for Task 12's real runs).
- Field numbers the walker reads (from `onnx.proto`): ModelProto `ir_version` 1, `opset_import` 8 (OperatorSetIdProto `domain` 1, `version` 2), `graph` 7, `functions` 25; GraphProto `node` 1, `initializer` 5, `input` 11, `output` 12, `sparse_initializer` 15; NodeProto `attribute` 5, `domain` 7; AttributeProto `t` 5, `g` 6, `tensors` 10, `graphs` 11, `sparse_tensor` 22, `sparse_tensors` 23; TensorProto `name` 8, `external_data` 13 (StringStringEntryProto `key` 1, `value` 2), `data_location` 14 (1 = external); SparseTensorProto `values` 1, `indices` 2; ValueInfoProto `name` 1, `type` 2; TypeProto `tensor_type` 1; TypeProto.Tensor `elem_type` 1, `shape` 2; TensorShapeProto `dim` 1; Dimension `dim_value` 1, `dim_param` 2. Element types: 1 F32, 2 U8, 6 I32, 7 I64.
- Rules (§7.3): domains `""` and `"ai.onnx"` only, at every depth; opset version ≤ `supported.opset`; `ir_version` ≤ `supported.ir_version`; `functions` present → `Functions`; external `location` must be a plain listed name and `offset + length` within its listed size; subgraph depth ≤ 16, nodes ≤ 100 000; every length checked against what is left of its message before it is read; bytes fields not needed are skipped with `Seek`, never read into memory (a `BufReader<File>`, so a 2 GiB file is walked in little memory).

- [ ] **Step 1: Write the failing tests** (`onnx/tests.rs`): `a_conv_network_gives_its_ports` (`conv_network` → one input `F32` `[Fixed(1), Fixed(1), Named("x"), Named("y"), Named("z")]`, one output); `a_node_in_a_custom_domain_is_refused`; `a_custom_node_inside_an_if_branch_is_refused`; `a_com_microsoft_opset_is_refused`; `local_functions_are_refused`; `external_data_in_a_listed_file_is_taken`; `external_data_named_with_dots_or_unlisted_or_past_the_end_is_refused`; `an_opset_newer_than_supported_is_refused`. `limit_tests.rs`: `subgraphs_seventeen_deep_are_refused`; `a_length_past_the_end_is_not_a_network`; `a_file_cut_short_anywhere_is_refused_without_a_panic` (proptest over cut points of `conv_network`'s bytes); `random_bytes_never_panic` (proptest).
- [ ] **Step 2:** Run `cargo test -p voxa-plugins onnx`; fail.
- [ ] **Step 3:** Implement the walker (`wire.rs`: varint, tags, length-delimited with remaining-length checks; `graph.rs`: the messages above), the testkit writer, and wire `read_network` into `check_folder` as §7.1 step 5 (`PluginError::Network`).
- [ ] **Step 4:** Tests and clippy pass.
- [ ] **Step 5: Commit** `feat(plugins): read a network's structure and refuse what is not standard ONNX`

### Task 4: The network against its description

**Files:**
- Create: `crates/voxa-plugins/src/fit.rs`, `src/fit/tests.rs`

**Interfaces:**
- Consumes: `Description`, `NetworkFacts`, `Port`, `Dim`, `Element`.
- Produces: `fn fit(description: &Description, facts: &NetworkFacts) -> Result<Wiring, PluginError>`; `struct Wiring { inputs: Vec<String>, output: String, free: Vec<(String, usize)> }`. `Offered` gains `network: NetworkFacts` and `wiring: Wiring`; `check_folder` calls `fit` as §7.1 step 6.
- Rules: label map: one image input (alignment: two, order as the network gives them); input element `F32`; rank 5; layout puts the single channel where `Layout` says; the three spatial dims equal to the box (`Whole`) or block size, a `Named` dim taking that size into `free`, `Unknown` refused; output from `output.from`, required when there are several outputs and must exist; output element and rank by `Gives`: `Scores` F32 rank 5 with channel count `channels.len()`, `Chance` F32 rank 5 with one channel, `Labels` U8/I32/I64 of rank 4 (`[1, d0, d1, d2]`) or rank 5 with one channel. `PluginError::Fit(sentence)` with sentences such as "The network's input is not the size its description gives." and "The network gives more than one answer and the description does not say which to read."

- [ ] **Step 1: Write the failing tests:** `a_whole_box_network_with_named_sides_is_wired_with_them`; `a_channels_last_network_is_read_so`; `a_fixed_side_that_differs_is_refused`; `several_outputs_need_from_and_from_must_exist`; `a_scores_output_with_the_wrong_channel_count_is_refused`; `a_labels_output_of_sixty_four_bit_integers_is_taken`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): check the network's inputs and outputs against the description`

### Task 5: The plugin grid and sampling

**Files:**
- Modify: `crates/voxa-segment/src/prepare/align.rs`, `src/prepare.rs`, `src/lib.rs` (export `Turn`, `Toward`)
- Create: `crates/voxa-plugins/src/grid.rs`, `src/grid/sample.rs`, `src/grid/tests.rs`, `src/grid/bead_tests.rs`

**Interfaces:**
- Produces:
  - `voxa_segment::Toward { Left, Right, Anterior, Posterior, Superior, Inferior }` (move from Task 1); `voxa_segment::Turn { pub order: [usize; 3], pub turned_round: [bool; 3] }` with `pub fn towards(volume: &Volume, steps: [f64; 3], towards: [Toward; 3]) -> Turn`; today's `Turn::of(volume, steps)` becomes `towards(volume, steps, [Right, Anterior, Superior])` and every SynthSeg test stays unchanged. Voxa's patient space is left-posterior-superior: a letter is a patient axis and a sign, and "turned round" means the step along that scan axis points against the letter.
  - `struct Grid { turn: Turn, size: [usize; 3], first: [f64; 3], step: [f64; 3], source_dims: [u32; 3] }` in the turned order; `fn Grid::of(volume: &Volume, input: &Input) -> Grid` with `steps` = source spacing / plugin spacing per source axis for `towards`;
  - `struct Region { from: [isize; 3], size: [usize; 3] }` (grid voxels; `from` may be below 0); `fn Grid::whole_box(&self, size: [usize; 3]) -> Region` centred: `from[a] = floor((m[a] − n[a]) / 2)`.
  - `fn sample(volume: &Volume, grid: &Grid, region: Region, outside: f32) -> Result<Vec<f32>, voxa_models::RunError>` (`[d0, d1, d2]`, d2 fastest; memory asked with `try_reserve_exact`, `NoRoom` when refused).
  - `fn label_grid(grid: &Grid, region: Region, ids: &[u16], volume: &Volume) -> voxa_labels::LabelGrid` (the region clipped to the grid, values turned back to the scan's own axes, columns fastest, `placement` from the scan's).
  - `fn overhang_mm(grid: &Grid, region: Region, input: &Input) -> Option<f64>` (largest distance in mm the scan's grid reaches past the region along any axis; `None` when it does not).
- Geometry (decided here; the spec leaves it to the plan): along turned axis `a` taken from scan axis `o = order[a]` with `n` voxels of spacing `s`, plugin spacing `t`: `m = max(1, round(n·s / t))`; grid voxel `k` lies at scan index `p(k) = c + (k − (m − 1)/2)·(t / s)`, `c = (n − 1)/2`, read as `(n − 1) − p(k)` where `turned_round[a]`. Sampling is linear along each scan axis (trilinear in scan index), positions in `[−0.5, n − 0.5]` clamped to `[0, n − 1]`, any position outside on any axis gives `outside`. No pre-blur (unlike SynthSeg's `resample`, which plugins do not use).

- [ ] **Step 1: Write the failing tests:** in `voxa-segment` `a_turn_towards_ras_is_synthsegs_turn` (for the made-up volumes of its `prepare/tests.rs`); in `grid/tests.rs` `a_one_mm_scan_at_one_mm_samples_its_own_values`, `halving_the_spacing_doubles_the_sides_and_interpolates`, `a_box_larger_than_the_grid_is_surrounded_with_outside`, `a_box_smaller_along_one_axis_and_larger_along_another_cuts_and_surrounds` (Review Focus 2, with `overhang_mm` asserted), `sampling_a_region_asks_only_for_its_own_voxels` (a 2 000-slice made-up volume, 64³ region: result length 64³). In `bead_tests.rs` `every_axis_code_puts_the_bead_back_where_it_was` (for all 48 codes: a bead off centre in a made-up volume, sample the whole grid as ids `1` where value > 0.5, `label_grid` back, the bead's voxel in the label grid's placement lies within half a voxel of the bead's patient point; and the test fails when `turned_round` is flipped for one axis) and `a_tilted_scan_keeps_the_bead` (Review Focus 1: the same with the scan's axes rotated 20° about two axes).
- [ ] **Step 2:** Run both crates' tests; the new ones fail.
- [ ] **Step 3:** Implement; keep SynthSeg's path calling the same code it did.
- [ ] **Step 4:** All `voxa-segment` tests (goldens included) and the new ones pass.
- [ ] **Step 5: Commit** `feat(plugins): the plugin's grid, sampled from the scan and mapped back`

### Task 6: Brightness

**Files:**
- Create: `crates/voxa-plugins/src/scaling.rs`, `src/scaling/stats.rs`, `src/scaling/tests.rs`

**Interfaces:**
- Consumes: `Grid`, `Region`, `sample` (Task 5); `Scaling`, `Over`, `Voxels`, `Outside` (Task 1).
- Produces:
  - `struct Statistics { lowest: f32, highest: f32, count: u64, mean: f64, sd: f64 }` plus a private histogram; `fn gather(volume: &Volume, grid: &Grid, over: Option<Region>, voxels: Voxels) -> Result<Statistics, RunError>` (`None` = the whole grid, §8.3 `scan`; `Some(box)` = §8.3 `grid`, surroundings counted at the scan's lowest value); one slab of grid (one index of axis 0) sampled at a time; two passes for the histogram.
  - `fn percentile(stats: &Statistics, percent: f64) -> f64`: 65 536 equal bins from `lowest` to `highest`, linear within the bin.
  - `struct Scale` and `fn Scale::new(scaling: &Scaling, stats: Option<&Statistics>) -> Result<Scale, ScaleError>`; `fn Scale::apply(&self, values: &mut [f32])`; `ScaleError::Flat` with the words FLAT.
  - `fn outside_value(outside: Outside, stats: &Statistics) -> f32` (`Lowest` → `stats.lowest`; `Zero` → `f32::NAN`, which `apply` turns into 0 after scaling).
- Definitions (§8.3): percentiles: `(x − lo)/(hi − lo)` held in `[0, 1]`, `lo == hi` → Flat; z-score: `(x − mean)/sd` over the voxels named (`AboveZero`: value > 0), `sd == 0` or no voxel counted → Flat; fixed: clip to `[a, b]` then `(x − m)/s`; none: unchanged.

- [ ] **Step 1: Write the failing tests:** `percentiles_of_a_ramp_are_within_a_bin_of_numpys` (values 0..=999 999 as a made-up volume; 2nd and 98th percentiles within `(highest − lowest)/65 536` of NumPy-linear figures worked by hand: 19 999.98 and 979 999.02); `z_score_above_zero_ignores_the_air`; `fixed_clips_then_subtracts_and_divides` (hand figures); `none_leaves_values_as_stored`; `over_the_grid_counts_the_surroundings_at_the_lowest_value`; `a_flat_scan_is_refused_in_words` (Review Focus 5: constant volume with percentiles, z-score; and a volume equal outside two voxels with 2nd/98th → FLAT words exactly); `zero_outside_becomes_zero_after_scaling`; `gathering_a_long_scan_holds_one_slab` (assert the private high-water mark of asked-for values ≤ one slab's count).
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): the four brightness scalings, from statistics gathered a slab at a time`

### Task 7: Blocks

**Files:**
- Create: `crates/voxa-plugins/src/blocks.rs`, `src/blocks/tests.rs`

**Interfaces:**
- Produces: `struct BlockPlan { grid: [usize; 3], size: [usize; 3], step: [usize; 3] }`; `fn BlockPlan::starts(&self) -> Vec<[usize; 3]>` (per axis: `0, step, 2·step, …` while `start + size < m`, then `m − size` unless already there; `[0]` when `m ≤ size`; the product of the three lists, axis 0 slowest); `fn BlockPlan::owned(&self, start: [usize; 3]) -> Region` (per axis, block `i` owns from the midpoint with block `i − 1`'s centre, inclusive, to the midpoint with block `i + 1`'s, exclusive, a midpoint falling on a voxel going to the earlier block; clipped to the grid); `fn BlockPlan::count(&self) -> usize`.

- [ ] **Step 1: Write the failing tests:** `blocks_cover_the_grid_and_the_last_lies_against_the_end`; `a_grid_smaller_than_a_block_is_one_block`; `every_voxel_is_owned_by_exactly_one_block` (proptest over grid 1..300, size 1..64, step 1..=size per axis); `a_voxel_is_owned_by_the_block_whose_centre_is_nearest` (brute force on small cases); `ties_go_to_the_earlier_block`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): blocks over the grid, each voxel owned by the nearest centre`

### Task 8: Finishing an answer into labels

**Files:**
- Modify: `crates/voxa-models/src/tensor.rs` (`Numbers::I32(Vec<i32>)`, `Numbers::I64(Vec<i64>)`), and every exhaustive `match` on `Numbers` in `voxa-models` and `voxa-segment` (treat the new kinds as `WRONG_KIND` there)
- Create: `crates/voxa-plugins/src/finish.rs`, `src/label_map.rs`, `src/finish/tests.rs`, `src/label_map/tests.rs`

**Interfaces:**
- Consumes: `Tensor`, `Numbers`; `LabelOutput`, `Gives`, `Layout`; `Grid`, `Region`, `label_grid`, `overhang_mm`; `BlockPlan`; `voxa_segment::largest_piece`.
- Produces:
  - `fn labels_of(answer: Tensor, layout: Layout, block: [usize; 3], output: &LabelOutput) -> Result<Vec<u16>, FinishError>`: scores → index of the largest (first on ties) mapped through `channels`; chance → `label` where `> threshold`; labels → as given, each 0 or a named id. Shape, kind and finiteness checked as `voxa-segment/src/find.rs` does (`floats`, `all_finite`).
  - `enum FinishError` (sentences): `WrongAnswer` "The plugin gave an answer of the wrong size or kind.", `NotFinite` "The plugin gave an answer with a number that is not finite.", `Unnamed` UNNAMED, `Nothing` NOTHING, `NoRoom` (→ `RunError::NoRoom`).
  - `struct LabelMap` with `fn LabelMap::new(region: Region) -> Result<LabelMap, FinishError>`, `fn put(&mut self, block_start: [usize; 3], block_size: [usize; 3], owned: Region, labels: &[u16])` (writes only owned voxels inside the map), `fn finish(self, output: &LabelOutput, title: &str, grid: &Grid, volume: &Volume) -> Result<(LabelGrid, Table), FinishError>`: largest piece of all labelled voxels when asked, `Nothing` when no voxel is labelled, `label_grid` back, a `Table` of `names` (title = plugin name, each `Entry` with `trust: Trust::Model`, colour from `Name.colour` or `colour_for(id)`, `verdict: None`, `made: None` — Task 15 fills `made`).

- [ ] **Step 1: Write the failing tests:** finish: `scores_take_the_largest_and_the_first_on_ties`; `channels_last_scores_are_read_so`; `a_chance_above_the_threshold_is_the_label`; `sixty_four_bit_labels_are_taken_and_an_unnamed_one_is_refused`; `a_nan_or_a_wrong_shape_stops_with_a_sentence`. label map: `blocks_put_together_give_each_voxel_its_owners_label` (a made-up "network" answering each block with its block number: every voxel's id is the owning block's); `the_largest_piece_is_kept_when_asked`; `nothing_labelled_says_so`; `a_whole_box_result_lands_on_the_scan_as_sampled` (Review Focus 2: cut along one axis, surrounded along another; a labelled bead lands on the bead's patient point).
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** All three crates' tests pass (SynthSeg unchanged).
- [ ] **Step 5: Commit** `feat(plugins): finish a network's answer into a label set`

### Task 9: The runtime, opened for any network

**Files:**
- Modify: `crates/voxa-models/src/runtime.rs` (split into `runtime.rs` + `runtime/open.rs` if it passes 400 lines), `src/network/tests.rs`

**Interfaces:**
- Produces: `fn Runtime::open_named(&self, file: &Path, device: Device, free: &[(String, usize)], output: Option<&str>, stop: &Stop) -> Result<Box<dyn Network>, RunError>` (each free dimension fixed by its name with `with_dimension_override`; with `output` set, only that output is fetched with `OutputSelector`; integer outputs read into `Numbers::I32`/`I64`; everything else as `open`); `Runtime::open` keeps its signature and calls `open_named` with `x`, `y`, `z` and `None`. `fetched` counts 4- and 8-byte integer outputs.

- [ ] **Step 1: Write the failing tests:** `bytes_of_counts_eight_byte_integers` (no runtime); `free_dimensions_are_named_by_the_caller` (no runtime: the list of overrides built for a session, through a small pure function `overrides(free) -> Vec<(String, i64)>` that refuses a size too large for `i64`).
- [ ] **Step 2:** Run `cargo test -p voxa-models`; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass; every existing `voxa-models` test still passes.
- [ ] **Step 5: Commit** `feat(models): open a network with its free sides named by the caller, and read integer answers`

### Task 10: The helper's process and its job object

**Files:**
- Create: `crates/voxa-models/src/process.rs` (`#[allow(unsafe_code)] mod process;` in `lib.rs` beside `cards`), `src/process/tests.rs`, `src/bin/voxa-test-eater.rs` (`[[bin]] name = "voxa-test-eater"`, `required-features = ["testkit"]`)
- Modify: `Cargo.toml` (`windows` features), `crates/voxa-models/Cargo.toml`, `crates/voxa-models/src/lib.rs` (doc comment: two modules may use `unsafe`)

**Interfaces:**
- Produces: `struct Helper`; `fn start_helper(program: &Path, args: &[&str], cap: Option<u64>) -> std::io::Result<Helper>` (piped stdin and stdout, stderr to null, no window: `CREATE_NO_WINDOW`; on Windows a job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` = 1, `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`, and with `cap` `JOB_OBJECT_LIMIT_PROCESS_MEMORY`; the process is put in the job before this returns, and the helper does nothing until its first message, so nothing runs outside the job); `fn Helper::take_pipes(&mut self) -> Option<(ChildStdin, ChildStdout)>`; `fn Helper::stop(&mut self)` (ends it at once: `TerminateJobObject`; elsewhere `kill`); `fn Helper::wait(&mut self) -> std::io::Result<ExitStatus>`; `fn Helper::peak_memory(&self) -> Option<u64>` (the job's `PeakProcessMemoryUsed`; `None` elsewhere); `Drop` stops it.
- `voxa-test-eater`: reads one line from stdin with a number of MiB, asks for that much with `try_reserve_exact` and touches every page, prints `held` or `refused`, then waits for stdin to close.

- [ ] **Step 1: Write the failing tests** (`#[cfg(windows)]`, using `env!("CARGO_BIN_EXE_voxa-test-eater")`): `a_helper_under_its_cap_holds_what_it_asks` (cap 256 MiB, ask 64 → `held`, `peak_memory` ≥ 64 MiB and ≤ 256 MiB); `a_helper_past_its_cap_is_refused` (ask 512 → `refused`); `stop_ends_a_helper_at_once` (stop before the line is sent; `wait` returns within 2 s); `dropping_the_helper_ends_its_process`.
- [ ] **Step 2:** Run `cargo test -p voxa-models --features testkit process`; fail.
- [ ] **Step 3:** Implement with the `windows` crate; each `unsafe` block with a `SAFETY` comment.
- [ ] **Step 4:** Pass; clippy pedantic clean.
- [ ] **Step 5: Commit** `feat(models): start a helper in a job object that caps its memory and ends it with Voxa`

### Task 11: The messages between Voxa and its helper

**Files:**
- Create: `crates/voxa-plugins/src/helper.rs`, `src/helper/{frame.rs, serve.rs, client.rs, opener.rs}`, `src/helper/tests.rs`, `src/helper/frame_tests.rs`

**Interfaces:**
- Consumes: `Network`, `RunError`, `Stop`, `Device`, `file_sum` (voxa-models); `labels_of` (Task 8); `Gives`, `Layout`, `LabelOutput`.
- Produces:
  - Frames (§10.2): `[kind: u8][length: u64 little-endian][payload]`, length ≤ 1 GiB else refused; control payloads are JSON (serde), a block's payload is its `f32`s little-endian, labels' payload `u16`s little-endian.
  - `enum Where { Processor, Card { id: CardId, name: String } }`; `struct OpenJob { runtime: PathBuf, network: PathBuf, sha256: String, device: Where, free: Vec<(String, usize)>, output: Option<String>, layout: Layout, block: [usize; 3], finish: LabelOutput }` (serde; `CardId` gains `Serialize`/`Deserialize` if it lacks them)
  - `enum Request { Open(OpenJob), Block(Vec<f32>), Finish }`; `enum Reply { Opened, Labels(Vec<u16>), Done { card_bytes: Option<u64> }, Failed { kind: FailKind, words: String } }`; `enum FailKind { NoRoom, DeviceLost, Changed, Failed }`
  - `trait Opener { fn load(&self, runtime: &Path) -> Result<(), RuntimeError>; fn open(&self, job: &OpenJob, stop: &Stop) -> Result<Box<dyn Network>, RunError>; fn card_in_use(&self, place: &Where) -> Option<u64>; }`
  - `fn serve(input: &mut impl Read, output: &mut impl Write, opener: &dyn Opener) -> Result<(), ServeError>`: waits for `Open`; checks the network's sum (`Failed{Changed}` on a mismatch, words CHANGED); loads, opens; answers each `Block` with its labels (`labels_of`); on `Finish` reports `Done` with `card_in_use` read before the network is dropped; ends when the input closes.
  - `struct Client<R: Read, W: Write>`; `fn Client::new(read: R, write: W) -> Self`; `fn open(&mut self, job: &OpenJob) -> Result<(), RunError>`; `fn block(&mut self, values: Vec<f32>) -> Result<Vec<u16>, RunError>`; `fn finish(&mut self) -> Result<Option<u64>, RunError>`. An input that ends before an answer is `RunError::Failed("ended")`, which callers say as ENDED; `Changed` maps to `RunError::Failed("changed")`, said as CHANGED.

- [ ] **Step 1: Write the failing tests** (`helper/tests.rs`, with `std::os::unix`-free in-memory pipes: two `std::sync::mpsc`-backed `Read`/`Write` adapters or `std::io::pipe` on Rust 1.99, serving on a thread with a stand-in `Opener` that hands out `voxa_models::testkit::Canned`): `a_job_is_opened_and_each_block_answered_with_labels`; `a_changed_network_file_is_refused_before_it_is_opened`; `a_helper_that_ends_mid_block_is_a_failure_not_a_hang` (Review Focus 4: the serving thread drops its pipe after `Opened`; `block` returns `Failed` within 2 s); `no_room_on_the_card_comes_back_as_no_room`. `frame_tests.rs`: `a_frame_round_trips`, `a_length_over_one_gibibyte_is_refused_before_anything_is_read`, `a_frame_cut_short_is_an_error_not_a_panic`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): the messages between Voxa and its plugin helper`

### Task 12: The helper as a program, and the first real runs

**Files:**
- Create: `crates/voxa-plugins/src/helper/runtime_opener.rs` (`RuntimeOpener`, using `voxa_models::Runtime::load` and `open_named`, telemetry off first as `Runtime::load` does), `src/bin/voxa-plugin-helper.rs`, `tests/with_runtime.rs`
- Modify: `crates/voxa-app/src/args.rs` (`Command::PluginHelper` for exactly `--plugin-helper`, not listed in `--help`), `crates/voxa-app/src/main.rs` (dispatched before anything else; no window, no message box), `crates/voxa-app/Cargo.toml`, `xtask/src/main.rs` (`WITH_MODELS` gains `voxa-plugins`)

**Interfaces:**
- Produces: `fn serve_stdio() -> std::process::ExitCode` (in `voxa_plugins::helper`): locks stdin/stdout, serves with `RuntimeOpener`, exit code 0 when the input closed after `Finish`, 1 otherwise.

- [ ] **Step 1: Write the failing tests:** `crates/voxa-app/src/args.rs` tests `plugin_helper_is_its_own_command` and `the_help_does_not_mention_the_helper`. In `tests/with_runtime.rs` (all `#[ignore]`, each needing `VOXA_RUNTIME`; a missing variable fails with a sentence, as `voxa-segment/tests/with_models.rs` does): `a_conv_network_labels_a_whole_box_on_the_processor` (`conv_network` weights making channel 1 = value − 0.5 and channel 0 = 0, a made-up volume with a bright bead; the labels equal `value > 0.5` exactly); `the_same_network_in_blocks_gives_the_same_labels`; `each_discrete_card_gives_the_processors_labels` (passes saying so when there is none); `a_cap_below_the_network_s_need_is_refused_not_obeyed` (cap 64 MiB on a 128³ block: `Failed` with `NoRoom`, Voxa's test process fine).
- [ ] **Step 2:** Run `cargo test -p voxa-app args` and `cargo test -p voxa-plugins --test with_runtime -- --ignored` with `VOXA_RUNTIME` set to `G:\dev\projects\voxa\.superpowers\runtime`; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass, then `cargo xtask check` green.
- [ ] **Step 5: Commit** `feat(plugins): voxa.exe --plugin-helper serves one plugin run in its own process`

### Task 13: Memory, measured once, and where a run fits

**Files:**
- Create: `crates/voxa-plugins/src/measure.rs`, `src/measure/tests.rs`

**Interfaces:**
- Consumes: `Helper`, `start_helper`, `peak_memory` (Task 10); `Client`, `OpenJob` (Task 11); `voxa_models::{Room, RunsOn, Card, CardKind, Device}`.
- Produces:
  - `const MEASURED: &str = "measured.json";` `struct Measured { runtime_sum: String, processor: Option<u64>, cards: Vec<CardFigures> }`; `struct CardFigures { name: String, card_bytes: u64, ordinary_bytes: u64 }`; `fn read_measured(folder: &Path) -> Option<Measured>`; `fn write_measured(folder: &Path, measured: &Measured) -> std::io::Result<()>` (written to a temporary name, then renamed).
  - `trait HelperStarter: Send + Sync { fn start(&self, cap: Option<u64>) -> std::io::Result<Box<dyn HelperPipes>>; }` with `trait HelperPipes { fn client(&mut self) -> Client<Box<dyn Read + Send>, Box<dyn Write + Send>>; fn stop(&mut self); fn peak_memory(&self) -> Option<u64>; }`; `struct ProcessStarter { program: PathBuf }` using `start_helper(program, &["--plugin-helper"], cap)`.
  - `fn measure(starter: &dyn HelperStarter, job: OpenJob) -> Result<(u64, Option<u64>), RunError>` (one made-up block: a smooth pattern `sin` of the index, not zeros; returns the job's peak ordinary memory and the card bytes).
  - `struct Fits { device: Device, cap: u64 }`; `fn places(measured: &Measured, own_bytes: u64, room: &Room, runs_on: RunsOn) -> Vec<Fits>` (§10.3: a card fits when `card_bytes × 1.25 ≤ budget` and `card_bytes ≤ unheld` where those were read; then for any place `ordinary × 1.25 + own_bytes ≤ room.free_memory`, and `cap = ordinary × 1.25`; order as `rungs`: discrete cards then the processor for `Automatic`; a card not yet measured is listed with `cap = 0`, which the caller takes as "measure first").
  - `fn own_bytes(block: [usize; 3], grid_voxels: u64) -> u64` (one block of `f32`, the `u16` label map, and 1 MiB for statistics).

- [ ] **Step 1: Write the failing tests:** `measured_figures_round_trip`; `a_card_fits_only_with_a_quarter_to_spare_within_its_budget_and_what_is_free`; `the_processor_fits_when_the_system_can_promise_the_cap_and_voxas_own_part`; `automatic_tries_discrete_cards_then_the_processor`; `a_card_not_measured_is_offered_to_be_measured_first`; `measuring_reports_the_jobs_peak` (a stand-in starter giving fixed figures).
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): measure a plugin's memory once and find where a run fits`

### Task 14: Modality and contrast

**Files:**
- Create: `crates/voxa-plugins/src/accepts.rs`, `src/accepts/words.rs`, `src/accepts/tests.rs`

**Interfaces:**
- Produces: `enum Decision { Refuse(String), Warn { warning: String, stamp: String }, Run { note: Option<String> } }`; `fn decide(modality: &str, description: &str, accepts: &Accepts) -> Decision`; `fn contrasts_named(description: &str) -> Vec<Contrast>`; `fn spacing_note(spacing_mm: [f64; 3], thickest_mm: Option<f64>) -> Option<String>`.
- `words.rs` is a copy of the redesign's `WEIGHTINGS`, `JOINED` and conflict rule (`crates/voxa-app/src/naming/words.rs` and `weightings_in`/`contrast_of` in `naming.rs` on `claude/ui-redesign-081656`), mapping its plain names to `Contrast`. A comment says one copy is kept when the branches merge.
- Rules (§5.3): modality compared upper-cased and trimmed; not listed or empty → `Refuse(MODALITY)`. `Any` → `Run { note: None }`. No name read → `Run { note: Some(STAMP_UNSAID) }`. Any name read accepted (`T1In3d` counting as `T1`) → `Run { note: Some(NOTE_MATCHED with the first accepted name's word) }`. Else `Warn { warning: WARN, stamp: STAMP_MISMATCH }`.

- [ ] **Step 1: Write the failing tests:** `a_ct_scan_is_refused_by_an_mr_plugin_in_words`; `a_scan_with_no_modality_is_refused`; `t2_flair_matches_a_flair_plugin_and_a_t2_plugin`; `mprage_matches_a_t1_plugin`; `a_t2_scan_warns_a_t1_plugin_with_the_exact_words`; `diffusion_warns_a_t1_plugin`; `t1_and_t2_together_are_not_said`; `no_contrast_word_is_not_said`; `any_contrast_runs_without_a_note`; `a_thick_scan_gets_the_spacing_note`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): refuse the wrong modality, warn on a contrast the description contradicts`

### Task 15: "Added by you" on a label set

**Files:**
- Modify: `crates/voxa-labels/src/made.rs` (`Added`, read and written in the `voxa` object as `added_by_you`), `src/words.rs` (`added_in_words`), `src/lib.rs`, the table tests
- Create: `crates/voxa-labels/src/made/added_tests.rs`

**Interfaces:**
- Produces: `struct Added { id, name, version, made_by, source, licence: String, limits: Vec<String>, sum: String }`; `Made` gains `pub added: Option<Added>` (default `None`; every `Made { .. }` literal in the workspace gains `added: None`); `fn added_in_words(added: &Added) -> String` (BANNER exactly); text cut to the description's limits on write and read; a `voxa` object without `added_by_you` reads as `None`; one of the wrong kind reads as `None`.

- [ ] **Step 1: Write the failing tests:** `added_by_you_round_trips_through_a_written_table`; `a_table_without_it_is_not_added`; `a_damaged_added_by_you_never_makes_a_table_unreadable`; `the_banner_is_word_for_word` (made-up plugin: `"Added by you, not part of Voxa. Example skull strip 1.0, made by An example lab. Source: https://example.org/skull-strip. Licence: MIT. Trained on adult heads only. Not for diagnostic use."`).
- [ ] **Step 2:** Run `cargo test -p voxa-labels`; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Workspace builds; all tests pass.
- [ ] **Step 5: Commit** `feat(labels): a label set can say a plugin added by the person made it`

### Task 16: One run, end to end

**Files:**
- Create: `crates/voxa-plugins/src/run.rs`, `src/run/tests.rs`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `struct Installed { folder: PathBuf, description: Description, sum: String, wiring: Wiring, measured: Option<Measured> }` and `fn installed(folder: &Path, supported: Supported) -> Result<Installed, PluginError>` (re-reads `plugin.json`, requires its sum to begin with the folder's name, runs `check_folder`).
  - `enum Stage { Preparing, Running { block: usize, of: usize }, Finishing }`
  - `struct Ran { grid: LabelGrid, table: Table, notes: Vec<String> }`
  - `fn run_plugin(plugin: &Installed, volume: &Volume, place: Fits, runtime: &Path, starter: &dyn HelperStarter, stop: &Stop, said: &mut dyn FnMut(Stage)) -> Result<Ran, RunError>`: grid, statistics, scale, box or blocks; for each block: sample, scale, send, put; then `finish`; notes in the order of §11.1 (NOTE_SPACING from `spacing_note`, NOTE_BOX when `overhang_mm` is `Some`, NOTE_BLOCKS for blocks); the caller puts the contrast stamp or note first. The helper is started with `cap = place.cap`, and stopped when `stop` is set (a thread watches `stop` every 50 ms, as `runtime.rs` does).

- [ ] **Step 1: Write the failing tests** (stand-in starter serving in-process with a stand-in opener that thresholds the block at 0.5): `a_whole_box_run_gives_labels_and_the_box_note_when_the_scan_reaches_past_it`; `a_block_run_gives_the_same_labels_and_the_blocks_note`; `stages_are_said_in_order_with_the_block_count`; `stop_ends_the_run_as_stopped` (Review Focus 4: stop set during block 2 of 8; `Err(RunError::Stopped)` within 2 s); `a_plugin_whose_folder_name_is_not_its_sum_is_not_installed`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(plugins): run a plugin on a volume, from its grid to its labels`

### Task 17: The app's plugins: folder, state and actions

**Files:**
- Create: `crates/voxa-app/src/plugins.rs` (state, `PluginAction`, `ToolEntry`), `src/plugins/store.rs` (the folder: list, add, replace, remove), `src/plugins/tests.rs`, `src/plugins/store_tests.rs`, `src/reducer/plugins.rs`, `src/reducer/plugins_tests.rs`
- Modify: `src/action.rs` (`Action::Plugins(PluginAction)`), `src/effect.rs` (`Effect::Plugins(PluginEffect)`), `src/state.rs` (`AppState.plugins: Plugins`), `src/reducer.rs`, `src/settings.rs` (`plugins_folder()` = settings folder + `plugins`)

**Interfaces:**
- Produces:
  - `store.rs`: `fn list(folder: &Path, supported: Supported) -> Vec<Result<Installed, (String, PluginError)>>`; `fn add(offered: &Offered, folder: &Path, supported: Supported) -> Result<Installed, PluginError>` (copy the listed files and `plugin.json` into `.adding-<16 hex of the sum>`, then rename to the sum's first 16 digits; on any failure remove the temporary folder; then `installed()` on the copy); `fn remove(plugin: &Installed) -> std::io::Result<()>`; `fn same_id<'a>(installed: &'a [Installed], offered: &Offered) -> Option<&'a Installed>`.
  - `plugins.rs`: `struct Plugins { entries: Vec<PluginEntry>, offered: Option<OfferCard>, warning: Option<Warning>, removing: Option<String>, adding: Option<String>, running: Option<Running>, runs: u64 }`; `struct PluginEntry { id, name, version, made_by, source, licence, about: String, limits: Vec<String>, checked: Result<(), String>, accepts: String, returns: String, runs_on: String, kept: bool }`; `struct OfferCard { entry: PluginEntry, settings: Vec<String> /* §5.1 step 3, one line each */, replaces: Option<String> /* the old version */ }`; `struct Warning { id: String, warning: String, stamp: String }`; `struct Running { id: String, run: u64, stage: Stage, series: SeriesId }`.
  - `enum PluginAction { Look, Looked(Vec<PluginEntry>), Offer(PathBuf), Offered(Result<OfferCard, String>), Add, Cancel, Added(Result<PluginEntry, String>), Measuring(String), Remove(String), ConfirmRemove, Removed(String), Run(String), ConfirmRun, Stage { run: u64, stage: Stage }, Failed { run: u64, why: String }, Stop, ShowKept(String), DeleteKept(String) }`; `enum PluginEffect { Look { folder: PathBuf }, Offer(PathBuf), Add, Remove(String), Run { run: u64, id: String, series: SeriesId, stamp: Option<String>, runs_on: RunsOn }, Stop, FindKept { series: SeriesId }, ShowKept { id: String, series: SeriesId }, DeleteKept { id: String, series: SeriesId } }`
  - `struct ToolEntry` = `PluginEntry` plus `state: String` and the action ids `plugin.<id>.run|stop|show|delete|remove` and `plugins.add`; `fn tool_entries(state: &AppState) -> Vec<ToolEntry>`.
  - `fn settings_lines(input: &Input, output: &LabelOutput) -> Vec<String>` (plain words: "Resampled to 1 × 1 × 1 mm.", "Turned to left, inferior, anterior.", "Fitted to 256 × 256 × 256 voxels." / "Cut into blocks of 128 × 112 × 112 voxels, 64 apart.", "Brightness: the 2nd to the 98th percentile of the fitted box." etc.; "Answer: the highest score of 2 channels.", "Only the largest connected piece is kept.").
- Reducer rules: `Run` decides with `accepts::decide` from the scene's `SeriesInfo` (modality, description): `Refuse` posts the sentence as a notice; `Warn` sets `warning`; `Run` emits the effect with the note as `stamp`; `ConfirmRun` emits it with the warning's stamp. A plugin run and a labelling run are one turn: starting either stops the other (`Effect::StopLabelling` / `PluginEffect::Stop`) (§5.3 step 4). Stages and failures of an older run are ignored (`runs`), as labelling's are.

- [ ] **Step 1: Write the failing tests:** store: `adding_copies_only_the_listed_files_into_a_folder_named_by_the_sum`; `a_failed_copy_leaves_no_folder_behind`; `a_plugin_with_the_same_id_is_found_for_replacing`; `removing_deletes_the_copy`. reducer: `a_ct_scan_posts_the_refusal_and_runs_nothing`; `a_mismatch_warns_and_runs_only_on_confirmation_with_the_stamp`; `a_plugin_run_stops_a_labelling_run_and_the_other_way`; `an_old_runs_stage_is_ignored`; `remove_asks_before_it_removes`; `tool_entries_carry_every_field_the_tools_tab_draws`; `settings_lines_say_the_examples_words`.
- [ ] **Step 2:** Run `cargo test -p voxa-app plugins`; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(app): plugins in the state: the folder, the entries and their actions`

### Task 18: The app's workers: add, measure, run, keep

**Files:**
- Create: `crates/voxa-app/src/worker/plugins.rs`, `src/worker/plugins/{add.rs, run.rs}`, `src/worker/plugins_tests.rs`, `src/worker/plugins_run_tests.rs`
- Modify: `src/worker.rs` (dispatch `Effect::Plugins`), `src/worker/labelling.rs` (the turn shared: plugin runs take the same `turn` as labelling runs), `src/kept_labels.rs` (a key per series and plugin: `"{series key}-{16 hex of the plugin sum}"`)

**Interfaces:**
- Consumes: `store`, `run_plugin`, `places`, `measure`, `ProcessStarter`, the labelling `Labeller` for `Room` (`room_of`) and the runtime folder found by labelling's look (beside the program, then the chosen folder).
- Produces: `Offer` → `check_folder` with `SUPPORTED` → `Offered(OfferCard)`; `Add` → `store::add`, then `measure` on the processor (a failure removes the copy and says why) and on each discrete card (a failure there keeps the plugin, processor only), `write_measured`, `Added(PluginEntry)`; `Run` → the volume the store keeps for the series (as labelling's `Job` takes it), `places`, measuring first a place not yet measured, `run_plugin` on the first place that works (no room or a lost card: the next place, as `climb` does), then a `Made { by: "Voxa {version}", model: name and version, model_sum: plugin sum, device, when, notes: stamp then run notes, added: Some(Added) }`, a label set, `Action::LabelsRead` with `NamesFrom::Model`, and the files kept under the plugin key. A helper that ends early says ENDED; a changed file says CHANGED; the runtime's own words go to the log with paths taken out (`logged_words`).
- Large scans: a comment at the point the volume is taken says that on merging `claude/big-scans-c8aec7` this takes the volume through its labelling call (`slices` range).

- [ ] **Step 1: Write the failing tests** (stand-in `Labeller` and a stand-in `HelperStarter` from `voxa-plugins` testkit): `offering_a_good_folder_reports_its_card`; `adding_measures_and_reports_where_it_runs`; `a_plugin_whose_processor_measurement_fails_is_not_kept`; `a_run_reports_labels_made_by_the_plugin_with_added_by_you_and_the_stamp`; `a_run_is_kept_for_its_series_and_plugin_and_found_again`; `a_helper_that_ends_early_says_so_and_voxa_goes_on` (Review Focus 4); `a_run_falls_to_the_processor_when_the_card_has_no_room`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5: Commit** `feat(app): add, measure and run plugins on the workers, and keep their results`

### Task 19: What the person sees

**Files:**
- Create: `crates/voxa-app/src/ui/plugins.rs`, `src/ui/plugins/{entry.rs, card.rs, warning.rs}`, `src/ui/plugins/tests.rs`
- Modify: `src/ui/parts.rs` (one call drawing the plugin section under the labelling strip), `src/ui/parts/labelling/made.rs` (the BANNER first when `made.added` is `Some`), `src/ui/planes/parts.rs` (`labels_note`: "Labels: added by you, not part of Voxa"), `src/ui/scene/captions.rs` (the same for solids), `src/parts_table.rs` (CSV: the BANNER and each note as lines before the table), `src/ui/capture.rs` (`VOXA_CAPTURE_SHOW` gains `plugin-card` and `plugin-warning`, drawn from made-up state)

**Interfaces:**
- Produces: `fn plugin_section(ui: &mut egui::Ui, entries: &[ToolEntry]) -> Option<Action>` (heading "Added by you", `Add a plugin…`, one row per entry: name and version, made by, source and licence as plain text, checked or the sentence, accepts, returns, runs on, `Run` or `Stop` with the stage, `Show`, `Delete`, `Remove`); `fn plugin_card(ctx: &egui::Context, card: &OfferCard) -> Option<Action>` (a window: CARD in bold first, then the plugin's words, then `settings_lines`, then `Add` / `Cancel`; with `replaces`: "This replaces version {old} of this plugin."); `fn contrast_warning(ctx: &egui::Context, warning: &Warning) -> Option<Action>` (WARN, then `Run anyway` / `Cancel`). Text from a description is drawn with `egui::RichText` as plain labels, never as links. The section is the redesign's to move: it draws nothing of its own outside these three functions.

- [ ] **Step 1: Write the failing tests** (egui's headless `Context::run_ui`, as other `ui` tests do): `the_card_says_voxas_words_before_the_plugins`; `a_source_is_text_not_a_link`; `the_warning_runs_only_on_run_anyway`; `the_planes_and_scene_say_added_by_you`; `the_csv_starts_with_the_banner_and_the_stamp`.
- [ ] **Step 2:** Run; fail.
- [ ] **Step 3:** Implement; then capture `VOXA_CAPTURE=…png VOXA_CAPTURE_SHOW=plugin-card` and `plugin-warning` at 900×600 and 640×480 into the scratchpad and look at both pictures (no overlap, words whole).
- [ ] **Step 4:** Pass; `cargo xtask check` green.
- [ ] **Step 5: Commit** `feat(app): the plugin entries, the confirmation card and the contrast warning`

### Task 20: Documents

**Files:**
- Create: `docs/adr/0007-plugins.md` (plugins; the helper process and job object; `process.rs` the second `unsafe` module of `voxa-models`; record 0006's "only one runtime per program" holds per process), `docs/plugins.md` (for plugin makers: the folder, every `plugin.json` field of §6 and §12 with the later kinds marked "not run by this version yet", the menu's exact meanings from §8 including the histogram's precision, the checks of §7, that Voxa recommends no plugin and links to none), `docs/quality/requirements.md` and `docs/quality/risk-register.md` (each with the preamble the large-scans branch uses and only a "PL" section: the spec's §16 and §17 tables with "Shown by" / "Checked by" filled with the test paths from Tasks 1–19)
- Modify: `SECURITY.md` (section "Plugins", S-PL-01 to S-PL-06 from §18), `docs/intended-use.md` (plugins are added by the person, not part of Voxa, not for diagnostic use), `docs/models.md` (a pointer to `plugins.md`), `CHANGELOG.md` (Unreleased: Added, Security), `docs/adr/0006-voxa-runs-a-labelling-model.md` (a line pointing to 0007), `docs/architecture.md` (the new crate and the helper)
- Shared notes (outside the repository): `ui-plugins-for-the-redesign.md` in the side-session notes folder: `ToolEntry`, the action ids, the three UI functions and the one call in `ui/parts.rs` to move, the captions changed, the contrast word list copied into `voxa-plugins/src/accepts/words.rs`.

- [ ] **Step 1:** Write the documents; every requirement row has at least one test path, and a row a test does not yet show says "Not yet met" as the large-scans file does.
- [ ] **Step 2:** `cargo xtask check` green (the docs build is part of it).
- [ ] **Step 3: Commit** `docs: plugins: the decision record, a guide for plugin makers, security, requirements and risks`

### Task 21: MindGrab, checked on this computer

**Files (outside the repository unless said):**
- Download (approved): `model.json`, `model.bin` from `public/models/mindgrab/` on the default branch of `github.com/neuroneural/brainchop` (check the branch name first) into a new empty folder `G:\dev\projects\dicom-analysis\models\mindgrab-source\` (git-ignored); check their sizes are 68 331 and 584 948 bytes before use.
- Create: `G:\dev\projects\dicom-analysis\models\mindgrab-convert\convert.py` (run with `G:\dev\projects\dicom-analysis\.venv-onnx\Scripts\python.exe -I convert.py SOURCE OUT`, paths as arguments, never run from inside the download folder): reads `model.json`'s `weightsManifest` and `model.bin` with NumPy, builds with `onnx.helper` the MeshNet the paper describes (each layer `Conv` 3×3×3 with the layer's dilation and padding, `InstanceNormalization` for group norm with groups = channels, GELU written as `0.5·x·(1 + Erf(x / √2))`, a final 1×1×1 `Conv` to 2 channels), opset 17, input `[1, 1, 256, 256, 256]` channels-first, and writes `mindgrab.onnx` and a `plugin.json` (§6, axes and scaling as Brainchop's conform and quantile step: 1 mm, `LIA` unless Brainchop's source shows otherwise, `Whole [256, 256, 256]`, `outside: lowest`, percentiles 2 and 98 `over: grid`; limits naming the eight-bit conform step not copied, and the weights' licence as not stated) into `G:\dev\projects\dicom-analysis\models\mindgrab-plugin\`. Check the ONNX against the browser model's layer list (`model.json` topology) before trusting it.
- Create (in the repository): `crates/voxa-plugins/tests/with_plugin.rs`: `#[ignore] a_plugin_named_by_voxa_test_plugin_masks_the_made_up_head` (needs `VOXA_TEST_PLUGIN` (the plugin folder), `VOXA_MODELS` (its `check/volume.f32` and `case.json` give the made-up head) and `VOXA_RUNTIME`): adds the plugin to a temporary folder, measures it, runs it on the processor and on each discrete card; the mask's Dice with the made-up head's brain (SynthSeg's labels from `check/standard.u16`, every label but 0 and 24) is at least 0.85 and it covers at least 0.90 of it. Prints both figures.
- Create: `docs/acceptance-plugins.md`: what was checked, on what (the made-up head only), the two figures, run times and measured memory, and that nothing derived from the weights is in the repository.

- [ ] **Step 1:** Download the two files and check their sizes.
- [ ] **Step 2:** Write and run the converter; read the browser model's topology and confirm layer by layer.
- [ ] **Step 3:** Add the plugin through Voxa itself (`cargo run --release -p voxa-app`, Tools → Add a plugin…), look at the card in a capture, and run it on the made-up head saved as a series by `write_phantom`'s means or on the test above.
- [ ] **Step 4:** Run `cargo test -p voxa-plugins --test with_plugin -- --ignored` with the three variables set; it passes, or the figures and the reason are reported to the owner rather than the mark lowered.
- [ ] **Step 5:** Measure §15's budgets in a release build and write them into the acceptance record: checking a plugin of 100 MB (a testkit network with a 100 MB initializer) within 2 s; a run's start-up (helper started, runtime loaded) within 1.5 s beyond opening the network; preparing MindGrab's 256 × 256 × 256 box from the made-up head within 2 s on the processor. A budget missed is reported to the owner with the figure, not hidden.
- [ ] **Step 6:** `cargo xtask check` green. **Commit** `test(plugins): a real plugin, checked on a made-up head` (the test and the acceptance record only).
