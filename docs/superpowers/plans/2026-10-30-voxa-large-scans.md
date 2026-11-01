# Very large scans: implementation plan

> Volsa was earlier called Voxa. This record keeps the name it was written with.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A 2,000-slice CT opens with its first slice at once, a 3D preview within a second and full detail after, while Voxa holds each series once at 16 bits and keeps decoded values under a memory budget, spilling past it to a file of its own.

**Architecture:** A new `HeldVolume` in `voxa-core` keeps a series' values as stored (16-bit integers with one rescale, or 32-bit floats) in slabs of whole slices; a `Grid` trait lets `voxa-volume`'s sampling, rays and regions read it or the old `Volume`. Workers decode straight into it; the window alone hands slabs to the card, a little each frame, as `R16Unorm` (signed values moved up by 32,768) in up to four slab textures. A small f32 preview is drawn while the full volume is decoded. Past the budget, finished slabs go to a spill file and come back on demand.

**Tech Stack:** Rust 1.99, egui/eframe 0.36, wgpu 30 (`TEXTURE_FORMAT_16BIT_NORM`), dicom-rs 0.10 (`dicom-pixeldata` `ConvertOptions` / `ModalityLutOption::None`), rayon. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-30-voxa-large-scans-design.md` (requirement numbers LS-nn, risks R-LS-nn and budgets Bn refer to it).

## Global Constraints

- The disclaimer verbatim: "Voxa is not a medical device and is not for diagnostic use."
- No `unsafe` outside `voxa-render` and `voxa-models/src/cards.rs`; `voxa-app` has `#![forbid(unsafe_code)]`. No new third-party crate; `cargo deny` unchanged.
- `cargo xtask check` passes before every commit: rustfmt, clippy pedantic as errors, files under 400 lines, functions under 60, every test, docs. `crates/voxa-app/src/worker/volume.rs` is at 399 lines: new worker code goes in new files.
- Nothing from a real export in a test, a document or a commit message; made-up studies only (`write_phantom`, the test kit). Never run bare `py`/`python`.
- Nothing is pushed, and nothing merged into `main` or `release`, without the owner.
- Do not touch `crates/voxa-segment` or `crates/voxa-models` (the plugin session's area), except reading `voxa_models::{cards, budget, unheld, free_memory, FreeMemory, CardKind}` and one additive function in `voxa-models/src/cards.rs` (Task 11), told to the plugin session in the shared notes folder.
- In `crates/voxa-app/src/ui/` change only the files the spec's section 10 names (`start.rs`, `mod.rs`, `pictures.rs`, `scene.rs`, `scene/gpu.rs`, `scene/captions.rs`, `planes.rs`, `planes/curve.rs`, `capture.rs`, `capture/measured.rs`), and only as each task says; the redesign session restructures the rest.
- Wording on screen, verbatim from the spec: "Preview: loading full detail ({done} of {of} slices)"; "Not enough memory to show this series in 3D: it needs {n} GB, and Windows can promise Voxa {m} GB. Close other programs and open it again."; "Smooth planes need more memory than this series leaves."; and 0.6.0's "Shown at {share} resolution to fit this graphics device." stays.
- Values: a value read from a `HeldVolume` holding 16-bit integers is `(slope as f64 * x as f64 + intercept as f64) as f32`, where `x` is the stored sample masked to Bits Stored and, if signed, sign-extended from bit Bits Stored − 1 (8-bit data: masked to 8 bits). This is what `dicom-pixeldata`'s default conversion gives, to the bit.
- Constants (*the spec's calls*): slab ≤ 32 MiB of whole slices; preview for series of more than 256 slices, every 8th slice, a quarter across and down; one texture ≤ 512 MiB; at most four slab textures; 64 MiB handed to the card a frame; budget 2 GiB or a quarter of physical memory, whichever is less, frame cache floor 64 MiB; memory-check margin 512 MiB.
- Comments and documents follow the project's plain-words style (see `docs/architecture.md`).

## Review Focus

1. A classic series whose files carry different rescale slopes (per-file rescale is normal): the build starts again holding 32 bits and every value still equals 0.6.0's. Test in Task 5.
2. The person switches series while a volume is being handed to the card: the old upload stops, its textures are destroyed, and slabs of two series are never mixed. Test in Task 6.
3. Series lengths at the edges of the slab plan (2 slices; exactly one texture; one slice more; a last slab of one shared slice): no seam, no missing slice, orientation kept. Test in Task 7.
4. The preview of one series still on screen after the person has moved to another, or after its build failed: never drawn for the wrong series. Test in Task 8.
5. The spill file cannot be made or written (folder not writable, disk full): the volume stays in memory over the budget, a notice says so, nothing crashes. Test in Task 12.

---

## Phase 1: held once

### Task 1: Reading a picture back a row at a time

**Files:**
- Modify: `crates/voxa-render/src/gpu.rs:238-264`
- Test: `crates/voxa-render/src/gpu.rs` (unit test module) or `crates/voxa-render/src/gpu/tests.rs`

**Interfaces:**
- Produces: `fn unpadded(bytes: &[u8], row_bytes: usize, padded_row: usize, rows: usize) -> Vec<u8>` (private), used by `read_back`.

- [ ] **Step 1: failing test** `rows_are_copied_without_their_padding`: 3 rows of 5 bytes padded to 8 (`[0,1,2,3,4,9,9,9, 10,..14,9,9,9, 20,..24,9,9,9]`) give the 15 bytes `0..4, 10..14, 20..24`; a padded buffer shorter than `rows * padded_row` gives only the whole rows it holds.
- [ ] **Step 2:** `cargo test -p voxa-render unpadded`; expected: does not compile.
- [ ] **Step 3:** implement with `Vec::with_capacity(rows * row_bytes)` and one `extend_from_slice` a row; `read_back` calls it.
- [ ] **Step 4:** `cargo test -p voxa-render`; expected: pass (the existing picture tests prove the copy is unchanged).
- [ ] **Step 5:** `cargo xtask check`; commit `perf(render): a picture is read back a row at a time (LS-05)`.

### Task 2: Preset windows only for slices shown

**Files:**
- Modify: `crates/voxa-app/src/worker/volume.rs:238-248` (`announce`), `crates/voxa-app/src/worker.rs:210-260` (`request_frames`)
- Test: `crates/voxa-app/src/worker/volume_tests.rs`, `crates/voxa-app/src/session/tests.rs`

**Interfaces:**
- Consumes: today's `Action::FrameReady { series, slice, windows }`, `PRESETS`.
- Produces: `request_frames` for a series whose volume is held reports `FrameReady` for the slice `around` (windows worked out from the volume's slice), instead of returning at once.

- [ ] **Step 1: failing tests.** `volume_slices_report_no_windows_while_building`: building a 30-slice made-up series reports no `FrameReady` for slices other than the one the viewport asked for. `a_held_series_reports_windows_for_the_slice_shown`: after `VolumeReady`, `RequestFrames { around: 7 }` yields one `FrameReady { slice: 7 }` whose windows equal `PRESETS.map(|p| p.window(&slice_7_values))`. In `session/tests.rs`, `scrolling_a_held_series_keeps_the_automatic_window`: scrolling to slice 20 of a held series settles `wl_pending` to false with the Auto preset's window for slice 20.
- [ ] **Step 2:** `cargo test -p voxa-app windows`; expected: the first fails (it reports every slice) and the others fail.
- [ ] **Step 3:** `announce` reports nothing; `request_frames`, when the store holds the series, spawns one task that reads slice `around` from the held volume, works out the three windows and reports `FrameReady`.
- [ ] **Step 4:** `cargo test -p voxa-app`; expected: pass.
- [ ] **Step 5:** gate; commit `perf(app): preset windows are worked out only for slices shown (LS-06)`.

### Task 3: Decoding values as stored

**Files:**
- Create: `crates/voxa-core/src/rescale.rs`, `crates/voxa-dicom/src/decode/stored.rs`, `crates/voxa-dicom/src/decode/stored_tests.rs`
- Modify: `crates/voxa-core/src/lib.rs`, `crates/voxa-dicom/src/decode.rs`, `crates/voxa-dicom/src/lib.rs`, `crates/voxa-dicom/src/testkit.rs` and `testkit/build.rs` (`bits_stored`)

**Interfaces:**
- Produces in `voxa-core`: `#[derive(Debug, Clone, Copy, PartialEq)] pub struct Rescale { pub slope: f64, pub intercept: f64 }` with `pub fn value(self, stored: i32) -> f32` = `(self.slope * f64::from(stored) + self.intercept) as f32`.
- Produces in `voxa-dicom`: `pub struct StoredFrame { pub rows: u32, pub cols: u32, pub samples: StoredSamples }`; `pub enum StoredSamples { Unsigned(Vec<u16>, Rescale), Signed(Vec<i16>, Rescale), Float(Vec<f32>) }`; `impl ImageFile { pub fn decode_stored(&self, frame: Option<u32>) -> Result<StoredFrame, VoxaError> }`. `Float` (the frame as `decode` gives it) when the photometric interpretation is not `MONOCHROME2`, Bits Allocated is not 8 or 16, or the slope is not finite and above zero. Otherwise the samples from `to_vec_frame_with_options` with `ModalityLutOption::None`, masked and sign-extended as the Global Constraints say, with the frame's own `rescale()?[0]`.
- Produces in the test kit: `SliceOptions::bits_stored: u16` (default 16), written as Bits Stored with High Bit one less.

- [ ] **Step 1: failing tests** in `stored_tests.rs`, each comparing `decode_stored` value by value, bit for bit (`f32::to_bits`), with `decode`: unsigned 16-bit with slope 1, intercept −1024 (`stored_ct_values_read_as_their_hounsfield_numbers`); signed 16-bit with negative samples; slope 2.5, intercept 0.25; Bits Stored 12 with garbage in the top four bits (`bits_above_bits_stored_are_ignored`); signed with Bits Stored 12 (`a_signed_twelve_bit_sample_is_sign_extended`); 8-bit unsigned and 8-bit signed (written with the existing `altered()` helper); an enhanced multi-frame file with a different rescale per frame (each frame has its own); `MONOCHROME1` and a zero slope give `Float`. Plus `Rescale::value(-1024)` with slope 1, intercept 0 is `-1024.0`.
- [ ] **Step 2:** `cargo test -p voxa-dicom stored`; expected: does not compile.
- [ ] **Step 3:** implement `Rescale`, `bits_stored` in the test kit, and `decode_stored` (inside `guarded`, after the same checks `decode` makes).
- [ ] **Step 4:** `cargo test -p voxa-dicom -p voxa-core`; expected: pass.
- [ ] **Step 5:** gate; commit `feat(dicom): a frame can be decoded to its values as stored, with its rescale (LS-02)`.

### Task 4: The held volume, and a grid both kinds of volume share

**Files:**
- Create: `crates/voxa-core/src/held.rs`, `crates/voxa-core/src/held/tests.rs`, `crates/voxa-core/src/grid.rs`
- Modify: `crates/voxa-core/src/lib.rs`, `crates/voxa-core/src/volume.rs` (implements `Grid`), `crates/voxa-volume/src/{sample,ray,measure,plane}.rs`, `crates/voxa-render/src/pick.rs`

**Interfaces:**
- Consumes: `Rescale` (Task 3).
- Produces in `voxa-core`:
  - `pub trait Grid: Sync { fn dims(&self) -> [u32; 3]; fn voxel_to_patient(&self) -> Affine<Voxel, Patient>; fn patient_to_voxel(&self) -> Affine<Patient, Voxel>; fn value(&self, col: u32, row: u32, slice: u32) -> Option<f32>; fn corners(&self) -> [Point<Patient>; 8]; fn spacing_mm(&self) -> [f64; 3]; }` with `corners` and `spacing_mm` given by default exactly as `Volume`'s are today (`Volume`'s inherent methods call them).
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Stored { Unsigned16, Signed16, Float32 }`.
  - `pub enum SliceIn<'a> { Unsigned(&'a [u16]), Signed(&'a [i16]), Float(&'a [f32]) }` and `pub enum SlabRaw<'a>` of the same three shapes.
  - `#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] pub enum HeldError { WrongKind, WrongSize, NoSuchSlice }` (sentences for the log only).
  - `pub struct HeldVolume` with `pub fn new(dims: [u32; 3], voxel_to_patient: Affine<Voxel, Patient>, stored: Stored, rescale: Rescale, slab_bytes: usize) -> Option<Self>` (`None` when a dimension is 0, the transform cannot be undone, or a slab could not hold one slice); `stored()`, `rescale()`, `slab_count() -> usize`, `slab_slices(slab: usize) -> Range<u32>`, `put_slice(&self, slice: u32, values: SliceIn<'_>) -> Result<(), HeldError>`, `has_slice(&self, slice: u32) -> bool`, `is_complete(&self) -> bool`, `slice_values(&self, slice: u32) -> Option<Vec<f32>>`, `with_slab<T>(&self, slab: usize, read: impl FnOnce(SlabRaw<'_>) -> T) -> Option<T>`, `to_volume(&self, slices: Range<u32>) -> Option<Volume>` (its transform moved to the first slice of the range), `bytes_held(&self) -> usize`; `impl Grid`.
  - Slabs are `RwLock<Option<Box<[T]>>>`, allocated at their first slice, zeroed; `put_slice` takes `&self` so that rayon tasks write different slices at once. A slab holds the most whole slices that fit `slab_bytes`, at least one.
- Produces in `voxa-volume`: `sample`, `enter_and_leave`, `first_hit`, `ellipse_stats`, `reslice` take `&(impl Grid + ?Sized)` instead of `&Volume`. In `voxa-render`: `pick(volume: &(impl Grid + ?Sized), ..)`.

- [ ] **Step 1: failing tests** in `held/tests.rs`: `a_value_is_its_stored_number_rescaled` (2×2×3, unsigned, slope 1, intercept −1024: stored 1024 reads 0.0); `signed_values_read_below_zero`; `float_values_read_as_given`; `slices_land_in_their_slabs` (dims 4×4×10, `slab_bytes` = 3 slices: 4 slabs of 3,3,3,1; `slab_slices(3) == 9..10`); `a_slice_of_the_wrong_size_or_kind_is_refused`; `a_volume_is_complete_once_every_slice_is_put` (also when slices are put from 8 rayon tasks at once); `the_values_of_a_range_of_slices_make_a_volume_placed_at_its_first_slice` (`to_volume(3..6)` equals the old `Volume` of those slices, with origin at slice 3's centre); `corners_and_spacing_are_those_of_a_volume_with_the_same_placement`. In `voxa-volume`, the existing tests of `sample`, `first_hit` and `ellipse_stats` run once on a `Volume` and once on a `HeldVolume` of the same values and give the same answers.
- [ ] **Step 2:** `cargo test -p voxa-core held`; expected: does not compile.
- [ ] **Step 3:** implement `Grid`, `HeldVolume`; make the five functions and `pick` generic.
- [ ] **Step 4:** `cargo test -p voxa-core -p voxa-volume -p voxa-render`; expected: pass.
- [ ] **Step 5:** gate; commit `feat(core): a held volume keeps a series' values as stored, in slabs (LS-01, LS-02)`.

### Task 5: Building the held volume, and the application reading it

**Files:**
- Create: `crates/voxa-app/src/worker/held.rs` (the build), `crates/voxa-app/src/worker/held_tests.rs`
- Modify: `crates/voxa-app/src/worker/volume.rs` (calls `held.rs`; `stacked`, `decode_all`, `slices_of`, `keep_frames` go), `crates/voxa-app/src/volume_store.rs` (`Arc<HeldVolume>`), `crates/voxa-app/src/session.rs` (`cached`, `volume`), `crates/voxa-app/src/worker/labelling.rs` (`Job.volume`, `volume_gone`), `crates/voxa-app/src/ui/{pictures.rs,scene.rs,scene/gpu.rs,planes.rs,planes/curve.rs,capture.rs,capture/measured.rs}` (type of the volume only), `crates/voxa-render/src/prepared.rs` and `smoothed.rs` (made from a `HeldVolume`), the examples and `crates/voxa-app/tests/phantom_head.rs`
- Test: `held_tests.rs`, `volume_tests.rs`, `session/tests.rs`, `worker/labelling_*tests.rs`

**Interfaces:**
- Consumes: `ImageFile::decode_stored` (Task 3), `HeldVolume`, `Grid` (Task 4).
- Produces: `VolumeStore::get(&self, &SeriesId) -> Option<Arc<HeldVolume>>`, `put(SeriesId, Arc<HeldVolume>) -> Option<(SeriesId, Arc<HeldVolume>)>`, `put_smoothed(&SeriesId, of: &Arc<HeldVolume>, Smoothed) -> bool`; `Session::volume(&self, &SeriesId) -> Option<Arc<HeldVolume>>`; `Prepared::of(held: &HeldVolume, max_side: u32, max_bytes: u64)` and `without_blocks(..)` (same signatures otherwise; this task keeps the 0.6.0 card path, which Task 6 replaces); `Smoothed::of(volume: &Volume, ..)` unchanged, given `held.to_volume(0..slices)` by the worker.
- In `held.rs`: `fn build_held(&self, listed: &IndexedSeries, build: &Build) -> Result<HeldVolume, String>`: the first slice decoded decides `Stored` and `Rescale` (`Unsigned`/`Signed` samples → 16 bits; `Float` → `Float32`); a later slice with another kind or rescale makes the build start again with `Float32`, decoding every slice again with `decode`; slices are decoded on the pool, each `put_slice` straight into place (no list of frames, no stacking); progress as before; a slice that fails ends the build with today's sentence ("Slice {n} could not be decoded, so a 3D view cannot be built: {why}"); a build that is overtaken stops and holds nothing. Slab size `SLAB_BYTES = 32 << 20`.
- `Session::cached` reads `held.slice_values(slice)` into a `Frame`. `keep_slices` (a volume giving way) puts its slices into the frame cache from `slice_values`. Labelling takes `Arc<HeldVolume>` in its job and makes `held.to_volume(0..slices)` on its own thread when the run starts.

- [ ] **Step 1: failing tests** in `held_tests.rs`: `a_ct_series_is_held_at_sixteen_bits_and_reads_its_hounsfield_numbers` (30 slices 4×4 of made-up CT, intercept −1024: `stored() == Unsigned16`, every value equals `decode`'s); `a_series_whose_files_differ_in_rescale_is_held_in_thirty_two_bits_with_the_same_values` (Review Focus 1: slices 0-14 slope 1, 15-29 slope 2); `a_monochrome1_series_is_held_in_thirty_two_bits`; `a_slice_that_cannot_be_decoded_fails_the_build_in_the_old_words`; `an_overtaken_build_holds_nothing`; `no_frame_of_a_built_series_stays_in_the_cache`. In `session/tests.rs` the existing `a_series_shown_in_3d_is_held_once_...` holds with `HeldVolume`, and `cached` of a held series equals the decoded slice. Labelling stand-in tests pass with the held job.
- [ ] **Step 2:** `cargo test -p voxa-app held`; expected: does not compile.
- [ ] **Step 3:** implement `held.rs`; switch the store, session, labelling and the listed `ui/` files to `Arc<HeldVolume>` (types only; `pointed` and `measured_on` call the now-generic `pick` and `ellipse_stats`); make `Prepared` from a `HeldVolume` by slab.
- [ ] **Step 4:** `cargo test --workspace`; run `cargo run --release -p voxa-app --example measure_big -- target/big/ct` with the sampler; expected: tests pass; the CT's peak private memory is below 0.6.0's 4,707 MiB (the old card path still makes its 16-bit copy here).
- [ ] **Step 5:** gate; commit `feat(app): a series is decoded straight into a volume held at 16 bits (LS-01, LS-03)`.

### Task 6: The card holds stored values, handed over by the window alone

**Files:**
- Create: `crates/voxa-render/src/upload.rs` (the plan of what goes to the card and the paced hand-over), `crates/voxa-render/src/upload/tests.rs`, `crates/voxa-app/src/ui/scene/upload.rs` only if `scene/gpu.rs` would pass 400 lines, `tools/measure/` (the sampler and scripts, made general: `sample.ps1`, `repeat.ps1`, `summarise.ps1`, `cold.ps1`, `warp_race.ps1`, with a `README.md`)
- Modify: `crates/voxa-render/src/{volume_pipeline.rs,volume_texture.rs,offscreen_scene.rs,gpu.rs,lib.rs,on_device/blocks.rs}`; delete `prepared.rs` (its range helpers move to `upload.rs`); `crates/voxa-app/src/worker/volume.rs` (`give_device`, `DeviceDoes::device`, `hand_over` go; `set_volume_side` becomes `set_card_room`), `crates/voxa-app/src/volume_store.rs` (`ready` becomes `for_card: Option<Arc<HeldVolume>>`, a reduced copy when the card cannot hold the series), `crates/voxa-app/src/ui/{mod.rs,start.rs,pictures.rs,scene.rs,scene/gpu.rs}`, the examples, benches
- Test: `upload/tests.rs`, `offscreen_scene/device_tests.rs`, `offscreen_scene/lost_tests.rs`, `worker/device_tests.rs`

**Interfaces:**
- Consumes: `HeldVolume::with_slab`, `slab_slices` (Task 4).
- Produces in `voxa-render`:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct CardRoom { pub longest_side: u32, pub most_bytes: Option<u64>, pub norm16: bool }`; `CardRoom::of(device: &wgpu::Device, most_bytes: Option<u64>) -> Self`; `CardRoom::halvings(&self, dims: [u32; 3]) -> u32` (0 when the series fits one texture of ≤ 512 MiB and ≤ `most_bytes`; Task 7 widens it to four slabs).
  - `VolumePipeline::start_upload(&mut self, device: &wgpu::Device, held: Arc<HeldVolume>, room: CardRoom) -> Result<(), RenderError>` (makes the texture under an out-of-memory scope: `R16Unorm` when `room.norm16` and the volume is 16-bit, else `R16Float`; drops any upload or volume before it); `VolumePipeline::continue_upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, at_most_bytes: u64) -> UploadState`; `#[derive(Debug, Clone, PartialEq)] pub enum UploadState { Idle, Going { done: u64, of: u64 }, Done, Failed(RenderError) }`. Each step writes whole slices from the slab in hand: unsigned 16-bit as they are; signed with `x ^ 0x8000` into a reused buffer of one slab; 32-bit through `as_half` as 0.6.0 did. `Placed.smallest`/`span` are `intercept`/`65535 × slope` (unsigned), `intercept − 32768 × slope`/`65535 × slope` (signed), or the volume's range (32-bit), so `shader_params` is unchanged. When the last slab is written the blocks of largest values are made by `BlocksMaker` or, on a device without it, on the processor from each slab as it is written.
  - `HeadlessScene::upload_held(&mut self, held: &Arc<HeldVolume>) -> Result<u32, RenderError>` (runs the whole upload; returns the halvings).
  - `pub const UPLOAD_BYTES_A_FRAME: u64 = 64 << 20`.
- Produces in `voxa-app`: `Workers::set_card_room(&self, room: CardRoom)`; the worker, after building, stores `for_card` = `voxa_volume::halved_held` applied `room.halvings(dims)` times when that is above 0; `voxa-volume` gains `pub fn halved_held(held: &HeldVolume) -> HeldVolume` (block means, rounded; same `Stored` and `Rescale`). The window (`ui/scene/gpu.rs`) calls `start_upload` when the store has a new volume and `continue_upload(UPLOAD_BYTES_A_FRAME)` each frame, asks for another frame while `Going`, and dispatches `SceneUploaded { halved }` at `Done`. The window's device (`ui/start.rs`) asks for `adapter.limits()`'s `max_texture_dimension_3d` and `max_buffer_size` and for `TEXTURE_FORMAT_16BIT_NORM` when the adapter has it; `VOXA_ADAPTER=software` makes it choose the adapter whose `device_type` is `Cpu` (for the WARP check; *my call*). The headless device asks for the same feature. The window works out `most_bytes` when it starts and before each new volume: the card found by name as labelling finds it (`worker/labelling/room.rs` `card_named`, made `pub(crate)`), `voxa_models::unheld(id)` plus what Voxa's current volume holds on the card, less 256 MiB; `None` when no card is found (*my call*), and passes it with `Workers::set_card_room`.

- [ ] **Step 1: failing tests.** `upload/tests.rs`: `card_room_halves_until_one_texture_holds_the_series` (512×512×2000 with side 2048 and no byte limit: 1 halving; with 16,384: still 1, for the 512 MiB texture cap; Task 7 makes it 0); `signed_values_are_moved_up_by_half_the_range` (the bytes written for −1024, 0, 3071 are 31744, 32768, 35839); `smallest_and_span_turn_a_texel_back_into_its_value` for unsigned and signed. Device tests (skip without an adapter, as today): `a_held_ct_draws_as_the_old_volume_did` (a 64×64×48 made-up CT with negative values drawn in VR and MIP after `upload_held` matches the picture of the same values uploaded through `upload(&Volume)` to within 1 grey level a channel); `paced_hand_over_finishes_in_steps` (`continue_upload(1 MiB)` on a 4 MiB volume returns `Going` three times, then `Done`); `a_new_upload_drops_the_one_in_hand` (Review Focus 2: start A, step once, start B, finish: the picture is B's, and A's texture is destroyed); in `lost_tests.rs` `an_upload_to_a_lost_device_fails_without_stopping`. `worker/device_tests.rs`: the workers have no device (the field is gone; the test that a worker hands over is replaced by one that the store holds `for_card` only when the card cannot hold the series).
- [ ] **Step 2:** `cargo test -p voxa-render upload`; expected: does not compile.
- [ ] **Step 3:** implement `upload.rs`, the pipeline changes, `halved_held`, the worker and window changes; remove `Prepared`, `hand_over`, `give_device`; update the examples (`measure_big` uses `upload_held` and prints `halved`), benches and `check_*` examples.
- [ ] **Step 4:** `cargo test --workspace`; then copy the scratch measuring scripts into `tools/measure/` with paths as parameters and run `tools/measure/warp_race.ps1` with `VOXA_ADAPTER=software` 20 times on the made-up head; expected: tests pass; 20 of 20 runs with no graphics error (B12).
- [ ] **Step 5:** gate; commit `feat(render): the card holds values as stored, handed over by the window a slab a frame (LS-04, LS-08, LS-09)`. Then run `tools/measure/repeat.ps1 -Prefix p1`, start `docs/measurements/2026-10-30-large-scans.md` with the 0.6.0 tables and the phase-1 numbers against B5-B9, B11, B12, and commit it (`docs: phase 1 measured`).

## Phase 2: full detail and speed

### Task 7: Slab textures past the card's longest side

**Files:**
- Modify: `crates/voxa-render/src/{upload.rs,volume_texture.rs,volume_pipeline.rs,volume_params.rs,plane_pipeline.rs,volume.wgsl,plane.wgsl,on_device/blocks.rs,blocks.wgsl}`, `crates/voxa-app/src/ui/planes/curve.rs`, `crates/voxa-app/src/ui/scene/captions.rs`
- Test: `crates/voxa-render/src/upload/tests.rs`, `crates/voxa-render/src/offscreen_scene/slab_tests.rs` (new)

**Interfaces:**
- Produces: `pub(crate) struct SlabPlan { pub step: u32, pub depths: Vec<u32> }`; `SlabPlan::for_volume(dims: [u32; 3], room: &CardRoom) -> Option<SlabPlan>` (`None` when more than four slabs would be needed). One slab of `dims[2]` slices when the series fits one texture; otherwise `step` is the largest multiple of 8 with `step + 1` slices within both the longest side and 512 MiB, slab `k` holds slices `k*step ..= min((k+1)*step, dims[2]-1)`. `CardRoom::halvings` is 0 whenever a plan exists within `most_bytes`; otherwise it halves (the slices only, while they are closer than the pixels; then every side) until one does. `Placed` holds up to four value textures; `volume.wgsl` and `plane.wgsl` gain bindings for values 1-3 (unused ones bound to slab 0) and a uniform `slabs: vec4<f32>` = (slices, step, count, 0) and `depths: vec4<f32>`; one WGSL function `value_at(at: vec3<f32>) -> f32` in each picks the slab from the voxel coordinate along the slices and samples it with `textureSampleLevel`. `BlocksMaker::make` runs its first pass once per slab into the z-range of the one blocks texture. The curve is made on the device only when the plan has one slab; otherwise `curve_on_device` returns `None` and the planes say "Smooth planes need more memory than this series leaves."
- [ ] **Step 1: failing tests.** `upload/tests.rs`: `a_series_that_fits_is_one_slab`; `the_study_ct_is_two_slabs_sharing_a_slice` (512²×2000, side 2048: step 1016, depths 1017 and 984); `a_long_thin_series_needs_four` and `five_would_be_too_many`; Review Focus 3 edges: 2 slices; exactly `step + 1`; `step + 2` (last slab two slices); every slab but the last a multiple of 8 long. `slab_tests.rs` (device): a made-up 32×32×40 volume with an off-centre bright bead in slices 18-22, drawn with a forced `step` of 16 (three slabs) and as one texture: VR, MIP and the three planes through the bead match to within 1 grey level a channel (`no_seam_where_slabs_meet`, `the_bead_is_where_it_was`); the blocks of largest values equal the one-texture ones.
- [ ] **Step 2:** `cargo test -p voxa-render slab`; expected: fails.
- [ ] **Step 3:** implement the plan, the textures, the shader function and uniforms, the blocks per slab, the curve rule and its sentence (the planes caption, in `captions.rs`).
- [ ] **Step 4:** `cargo test --workspace`; `measure_big target/big/ct` on the RTX and on the Intel card; expected: `[512, 512, 2000] on the device, halved 0 times` on both.
- [ ] **Step 5:** gate; commit `feat(render): a series longer than the card's longest side is held in slabs, in full detail (LS-07, LS-10)`.

### Task 8: The preview first

**Files:**
- Create: `crates/voxa-volume/src/preview.rs`, `crates/voxa-app/src/worker/preview.rs`
- Modify: `crates/voxa-volume/src/lib.rs`, `crates/voxa-app/src/{action.rs,scene.rs,reducer/scene.rs,volume_store.rs,worker/held.rs}`, `crates/voxa-app/src/ui/{pictures.rs,scene.rs,scene/captions.rs}`, `crates/voxa-app/examples/measure_big.rs`
- Test: `crates/voxa-volume/src/preview.rs` tests, `crates/voxa-app/src/reducer/scene_tests.rs` (or the reducer's existing test file), `crates/voxa-app/src/worker/held_tests.rs`

**Interfaces:**
- Produces in `voxa-volume`: `pub fn preview(voxel_to_patient: Affine<Voxel, Patient>, dims: [u32; 3], slices: &[(u32, Vec<f32>)], across: u32) -> Option<Volume>`: each given slice reduced to `ceil(cols/across) × ceil(rows/across)` by block means, stacked in the order given, placed so that preview voxel `(i, j, k)` lies at the centre of full voxels `across*i .. across*i+across-1`, `across*j ..`, and at full slice `slices[k].0` (slices evenly spaced).
- Produces in `voxa-app`: `pub const PREVIEW_SLICES: usize = 256`, `PREVIEW_STRIDE: u32 = 8`, `PREVIEW_ACROSS: u32 = 4`; `Action::PreviewReady { series: SeriesId, corners: [Point<Patient>; 8] }`; `Scene::preview: bool` (set by `PreviewReady` while the scene is `Building` for that series; cleared by `start`, `VolumeReady`, `VolumeFailed`); `VolumeStore::put_preview(SeriesId, Volume)` / `take_preview(&SeriesId) -> Option<Volume>`, emptied by `put` and `clear`. The build decodes every 8th slice first (for a series of more than 256 slices), makes the preview, stores it and reports `PreviewReady`, then decodes every slice slab by slab. The window uploads the preview with `VolumePipeline::upload` and draws it while `Building` and `preview`; the scene's caption is "Preview: loading full detail ({done} of {of} slices)"; `pointed` (picking) and the planes do nothing while the status is not `Ready`, as today. `measure_big` prints a `preview` step.
- Also produces the redesign's hook (spec section 10): `Action::SceneWanted(bool)` and `Scene::wanted: bool` (default `true`). While it is `false`, `start` emits no `BuildVolume` and the scene stays `Empty`; turning it `true` starts the build for the active viewport's series. Nothing in the window sends `false` yet.
- [ ] **Step 1: failing tests.** `preview.rs`: `a_ramp_is_reduced_to_its_block_means`; `the_preview_lies_where_the_series_lies` (corners within one full voxel of the full volume's). Reducer: `a_preview_is_drawn_only_while_its_series_builds` (Review Focus 4: `PreviewReady` for another series, or after `VolumeFailed`, sets nothing; `VolumeReady` clears it); `nothing_is_picked_or_measured_on_a_preview`. `held_tests.rs`: `a_long_series_reports_a_preview_before_its_volume` (300 slices: `PreviewReady` comes before `VolumeReady`; a 200-slice series reports none); `the_preview_slices_are_decoded_twice_and_the_volume_is_still_exact`. `session/tests.rs`: `the_slice_on_screen_is_decoded_before_the_preview` (LS-12: the viewport's `FrameReady` comes before `PreviewReady`); `no_volume_is_built_while_the_scene_is_not_wanted` (then `SceneWanted(true)` starts it).
- [ ] **Step 2:** `cargo test -p voxa-volume preview` and `-p voxa-app preview`; expected: fail.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** `cargo test --workspace`; a picture-and-quit run on the CT with `VOXA_CAPTURE_SHOW=vr` taken during building (add `preview` to `VOXA_CAPTURE_SHOW` only if capture cannot otherwise wait for it; *my call*) shows the caption.
- [ ] **Step 5:** gate; commit `feat(app): a long series is drawn as a preview while it is decoded (LS-11)`.

### Task 9: Indexing on several threads

**Files:**
- Modify: `crates/voxa-dicom/src/source.rs` (two provided methods), `source/folder.rs`, `source/zipped.rs`, `crates/voxa-dicom/src/index.rs:64-90`
- Test: `crates/voxa-dicom/src/index/reading_tests.rs`, `crates/voxa-app/benches/decode.rs` (an index bench of 2,000 files)

**Interfaces:**
- Produces: `fn names(&mut self) -> Option<Vec<String>>` (the names `visit_files` would visit, in its order; default `None`) and `fn visit_these(&mut self, names: &[String], visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>) -> Result<(), VoxaError>` (default: an error) on `SeriesSource`, implemented by folders and zips. `index` uses them when both `names` and `another_reader` give something: the names in contiguous runs, one run per thread with a reader of its own, each header read as today; then every header is added in name order on the calling thread, with progress as today. Otherwise it visits as today.
- [ ] **Step 1: failing tests.** `an_index_read_on_several_threads_is_the_same_index` (a made-up folder of three series, 300 files, and the same as a zip, stored and compressed: equal to the index from `MemorySource`, which reads in turn); `a_file_that_cannot_be_read_stops_the_index_as_before` (the same error, and no file after it in name order is added).
- [ ] **Step 2:** `cargo test -p voxa-dicom several_threads`; expected: fails to compile.
- [ ] **Step 3:** implement; the redesign's documents change sits where `found.add` meets a file with no image, which this task moves but does not change.
- [ ] **Step 4:** `cargo test --workspace`; `cargo bench -p voxa-app --bench decode -- index`; expected: pass; the 2,000-file index is quicker than before (record both).
- [ ] **Step 5:** gate; commit `perf(dicom): headers are read on several threads (LS-13)`.

### Task 10: Where a stored zip's extra time goes

**Files:**
- Modify (as the measurement shows): `crates/voxa-dicom/src/source/zipped.rs`, `source/archive_file.rs`, `source.rs` (`read_limited`)
- Test: `crates/voxa-dicom/src/source/zipped/tests.rs`

- [ ] **Step 1:** time reading every entry of `target/big/ct-stored.zip` and of `target/big/ct` with a throwaway timing in `benches/decode.rs` (`read_every_entry`), and profile the zip's read: entry lookup, small reads through `ArchiveFile`'s read-ahead, `read_limited`'s growth, the CRC check.
- [ ] **Step 2:** write the failing test for the change the profile points to (for example `a_stored_entry_is_read_in_one_read_of_its_size`, counting reads through a counting reader), keeping the CRC check.
- [ ] **Step 3:** implement it; if nothing cheap is found, record the profile in the commit message and leave the code.
- [ ] **Step 4:** `measure_big` on `ct`, `ct-stored.zip`, `ct.zip` three times each; expected: B10 (stored ≤ 1.2 ×, compressed ≤ 1.5 × the folder's time to full detail), or the reason it is not met recorded.
- [ ] **Step 5:** gate; commit `perf(dicom): <what was found>` (LS-17). Then run `tools/measure/repeat.ps1 -Prefix p2` and record the phase-2 numbers against B1-B4, B10.

## Phase 3: the budget

### Task 11: One memory budget

**Files:**
- Create: `crates/voxa-app/src/budget.rs`, `crates/voxa-app/src/budget/tests.rs`
- Modify: `crates/voxa-app/src/{prefs.rs,session.rs,cache.rs,lib.rs}`, `crates/voxa-app/src/ui/mod.rs` (passes the budget to `Session::new`), `crates/voxa-models/src/cards.rs` (`total_memory`, additive), examples and test helpers that call `Session::new`

**Interfaces:**
- Produces: `pub struct Budget { limit: usize }`; `Budget::default_for(physical_bytes: u64) -> usize` = min(2 GiB, physical / 4); `pub const CACHE_FLOOR: usize = 64 << 20`; `Prefs::memory_mib: Option<u32>` (absent: the default; `sound()` keeps it between 256 and 65,536); `Session::new(to_reports, reports, budget_bytes: usize, settings)` (the frame cache's budget becomes `budget_bytes − held volume bytes`, never below `CACHE_FLOOR`, re-set whenever a volume is kept or given way); `Shared::over_budget(&self) -> usize` (bytes past the budget: held slabs in memory + frame cache + on an integrated card the card's copy as the window reports it with `Workers::card_holds(bytes)`).
- [ ] **Step 1: failing tests.** `the_default_budget_is_two_gigabytes_or_a_quarter_of_memory`; `a_preferences_file_without_a_budget_takes_the_default` and `a_budget_out_of_range_is_held_within_it`; `the_frame_cache_gets_what_the_volume_leaves_and_never_less_than_its_floor`; `on_an_integrated_card_the_cards_copy_counts`.
- [ ] **Step 2:** `cargo test -p voxa-app budget`; expected: fails.
- [ ] **Step 3:** implement. The computer's memory comes from one additive function, `pub fn total_memory() -> Option<u64>` in `voxa-models/src/cards.rs` beside `free_memory` (the same `GlobalMemoryStatusEx` call, `ullTotalPhys`; `/proc/meminfo` `MemTotal` on Linux; `None` elsewhere), since `voxa-app` may not call the system itself; with `None` the default is 2 GiB. Tell the plugin session in the shared notes folder.
- [ ] **Step 4:** `cargo test --workspace`.
- [ ] **Step 5:** gate; commit `feat(app): one memory budget for decoded values (LS-14)`.

### Task 12: The spill file

**Files:**
- Create: `crates/voxa-core/src/held/away.rs` (the trait and the slab states), `crates/voxa-app/src/spill.rs`, `crates/voxa-app/src/spill/tests.rs`
- Modify: `crates/voxa-core/src/held.rs`, `crates/voxa-app/src/{lib.rs,worker/held.rs,ui/mod.rs}` (leftovers deleted at start, with `moved.rs`'s other start-up work), `crates/voxa-app/src/state.rs` (one notice key)

**Interfaces:**
- Produces in `voxa-core`: `pub trait SlabStore: Send + Sync { fn put_away(&self, slab: usize, bytes: &[u8]) -> std::io::Result<()>; fn bring_back(&self, slab: usize, into: &mut [u8]) -> std::io::Result<()>; }`; `HeldVolume::keep_away_in(&self, store: Arc<dyn SlabStore>)`; `HeldVolume::put_away_least_used(&self) -> std::io::Result<usize>` (the bytes freed; only complete slabs; least lately read first); every reader brings a slab back on demand and counts it as read. A slab that cannot be brought back reads as missing (`None`), and the failure is logged by kind only.
- Produces in `voxa-app`: `pub struct SpillFile` implementing `SlabStore`: a file `spill-{random}.voxa` in `<local folder>/cache/`, opened with `share_mode(0)` and `create_new`, slabs at `slab × slab_bytes`, deleted on `Drop`; `pub fn clear_leftovers(local: &Path) -> usize` deletes every `spill-*.voxa` there. After each slab is finished, and after a slab is brought back, the worker calls `put_away_least_used` until `over_budget()` is 0. If the file cannot be made or written, the volume stays in memory and one notice says: "Voxa could not make room on disk, so this series uses more memory than the amount set." (*my call*, Review Focus 5). Nothing is spilled while a series fits.
- [ ] **Step 1: failing tests.** `spill/tests.rs`: `a_slab_put_away_comes_back_the_same`; `the_file_is_deleted_when_its_volume_goes`; `leftovers_are_deleted_at_start`; `the_file_cannot_be_opened_by_another_program_while_voxa_has_it` (a second open fails on Windows); `the_file_holds_no_name_or_tag` (its bytes are exactly the slabs). Worker: `a_series_within_the_budget_never_spills`; `a_series_past_the_budget_spills_and_reads_the_same_values` (budget of 2 slabs for a 6-slab series; every value equal; `bytes_held` ≤ budget + one slab); `when_the_spill_file_cannot_be_made_the_volume_stays_and_a_notice_says_so` (Review Focus 5: local folder pointed at a file, not a folder).
- [ ] **Step 2:** `cargo test -p voxa-app spill`; expected: fails.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** `cargo test --workspace`; `measure_big` on the CT with `VOXA_BUDGET_MIB=512` (*my call*: an environment override for measuring, read in `Session` setup beside the preference); expected: B13.
- [ ] **Step 5:** gate; commit `feat(app): past the budget, finished slabs wait in a file of Voxa's own (LS-14, LS-15)`.

### Task 13: Memory checked before building; whole-volume work within the budget

**Files:**
- Modify: `crates/voxa-app/src/worker/held.rs`, `crates/voxa-app/src/worker/volume.rs` (`work_out_curve`), `crates/voxa-app/src/worker/labelling.rs`, `crates/voxa-app/src/worker.rs` (a free-memory source that tests can replace)
- Test: `held_tests.rs`, `volume_tests.rs`, `labelling_tests.rs`

**Interfaces:**
- Produces: `Shared::free_memory: Box<dyn Fn() -> FreeMemory + Send + Sync>` (`voxa_models::free_memory` by default). Before building: `needed = min(values_bytes, budget) + slab_bytes + 512 MiB`; when `FreeMemory::Promised(m)` (or `Physical(m)`) is below it, `VolumeFailed` with "Not enough memory to show this series in 3D: it needs {n} GB, and Windows can promise Voxa {m} GB. Close other programs and open it again." (n, m to one decimal). `work_out_curve` (the processor's curve) is refused for a series whose 32-bit copy would pass the budget, with `Action::Post` of "Smooth planes need more memory than this series leaves." Labelling's job carries `slices: Range<u32>` (all of them unless a later model asks for fewer) and makes `to_volume(slices)` (LS-18).
- [ ] **Step 1: failing tests.** `too_little_memory_refuses_the_build_in_a_sentence` (stand-in free memory of 100 MiB); `a_series_past_the_budget_is_not_refused_for_its_size` (budget 64 MiB, free 2 GiB, a 300 MiB series builds); `the_processors_curve_is_refused_past_the_budget`; `labelling_is_given_the_range_of_slices_it_asks_for`.
- [ ] **Step 2:** `cargo test -p voxa-app memory`; expected: fails.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** `cargo test --workspace`.
- [ ] **Step 5:** gate; commit `feat(app): memory is checked before a volume is built (LS-16, LS-18, LS-19)`.

### Task 14: The documents and the measurements

**Files:**
- Create or modify: `docs/quality/requirements.md` and `docs/quality/risk-register.md` (a section "LS: very large scans" in the redesign's format; create the files with the redesign's preamble if they are not on this branch), `docs/quality/usability.md` (only if present on this branch; else the notes go in the measurement record), `docs/architecture.md` (data flow steps 3 and 3D steps 1-2, the spill file in "What is kept on this machine"), `SECURITY.md` (the spill file), `CHANGELOG.md` (`[Unreleased]`), `docs/measurements/2026-10-30-large-scans.md` (before and after, every budget met or not)

- [ ] **Step 1:** fill "Shown by" for LS-01 to LS-20 with each test's path and name; add R-LS-01 to R-LS-08 with "Checked by" pointing at the tests.
- [ ] **Step 2:** run `tools/measure/repeat.ps1 -Prefix after -Runs 1,2,3` (headless, both cards, and `voxa.exe`), `warp_race.ps1` 20 times, `cold.ps1` on the CT folder and zips; summarise with `summarise.ps1`.
- [ ] **Step 3:** write the measurement record: the section 3 tables before and after, each budget B1-B13 met or not with the figure, what was left.
- [ ] **Step 4:** `cargo xtask check`; expected: green, notices unchanged.
- [ ] **Step 5:** commit `docs: very large scans, requirements, risks and measurements`.
