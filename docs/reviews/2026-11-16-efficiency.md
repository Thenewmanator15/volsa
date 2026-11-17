# Efficiency review, 16 November 2026

A read-only review of how Volsa 0.8.0 (`db42f4c`) uses the CPU, threads, memory and the
graphics card. Nothing was changed. Findings are ranked by impact. Sizes: small (under a day),
medium (a few days), large (a week or more).

## What was measured

A build of HEAD in a scratch folder, on a 20-core machine with an RTX 5070 Ti and an Intel
iGPU. The data was made up by `write_phantom ct OUT 600` (512 × 512 × 600, 303 MB). Volsa's
settings files were backed up before the runs and restored after.

| | Welcome screen | Synthetic CT open, idle |
|---|---|---|
| CPU, % of one core | 1.4 | 0.6 |
| Threads | 85 | 84 |
| Private memory | 285 MB | 911 MB |
| GPU dedicated memory | 151 MB | 440 MB |

`measure_big` on the same CT: first slice 55 ms, preview 82 ms, volume 294 ms, on the device
after 193 ms (longest step 13 ms), VR 2.2 ms a picture at 1280 × 800. Finding the headless
device took 1.66 s.

**Already good:** at idle the window does not redraw. Every `request_repaint` has a reason:
cine, uploads, timing, settle. The slice texture is reused when its size stays the same.
Plane bind groups are cached by upload, curve and label number. Volume upload is limited to
32 MiB a frame. The frame cache and the volume share one budget. Labels go to the device once
per `LabelSet`.

## Findings

### 1. 2D viewports copy slices on the CPU even when the volume is already on the GPU (large)

- **Where:** `crates/volsa-app/src/ui/pictures.rs:216` →
  `crates/volsa-app/src/session.rs:90-116` → `crates/volsa-app/src/ui/viewport.rs:323-339`.
- **Cost:** Once the scene's series is held as a volume, the cache loses its slices. Each new
  slice then has to be rebuilt: `HeldVolume::slice_values` makes a fresh `Vec<f32>` (1 MiB at
  512²; 16 MiB for 2048² DX or whole-body planes), that goes into an `Arc<Frame>`, and
  `write_texture` sends it to an R32F texture. That happens on the UI thread on every step
  through the slices. Meanwhile the same values are already on the device as an R16 3D
  texture, and the plane views (`plane_pipeline.rs`, `show`/`bound`) already sample it. This
  goes against the standing rule to reuse what is already on the device.
- **Fix:** When the series' volume is on the device, draw the viewport from the volume
  texture, as an axis-aligned plane: a slice index in a uniform, no upload, no CPU copy. Keep
  the R32F path for series with no volume and for colour. Cine and scrolling then cost only a
  uniform write.
- **Note:** Wait until the work now open in `slice_pipeline.rs` / `slice.wgsl` /
  `slice_labels.rs` has landed. It is the same code.

### 2. Spilled volumes are read from disk on the UI thread (medium)

- **Where:** `Session::value_at` (`session.rs:157-170`) → `HeldVolume::value`
  (`crates/volsa-core/src/held.rs:308-317`) → `read_slab` / `bring_back`
  (`crates/volsa-core/src/held/away.rs:138-176`). Also `cached` / `read_slice` → `read_part`
  (`away.rs:201-222`).
- **Cost:** On a series past the budget, slabs are spilled to disk. Hovering over a slice in a
  spilled slab brings the whole 32 MiB slab back from disk on the UI thread, just to read one
  voxel. `within_budget_later` (`worker/memory.rs:131`) then pushes another slab out. Moving
  the pointer across slabs therefore costs 32 MiB of disk I/O per slab while the window
  waits. Stepping through spilled slices also reads each slice from disk on the UI thread.
  This only affects large CT and whole-body series, which are the ones that matter here.
- **Fix:** In `value`, use `read_part` for one voxel with no bring-back (small). Move slice
  copies for viewports to a worker, so the frame arrives as a report like a decoded slice
  does (medium). Finding 1 removes most of this for series on the device.

### 3. Decoded slices are held as f32: twice the stored size, and a copy of the volume (medium)

- **Where:** `volsa_dicom::Frame { values: Vec<f32> }`, `crates/volsa-app/src/cache.rs:104`.
  The R32F upload is at `slice_pipeline.rs` (HEAD, `texture`).
- **Cost:** For 16-bit data (all CT and almost all MR) each cached slice takes 4 bytes a
  pixel instead of 2. The cache gets whatever the volume leaves of the budget: up to about
  1.7 GiB of a 2 GiB budget when a 300 MiB volume is held. Those slices can be of the same
  series as the volume, so the values are held twice. In the measurement, private memory was
  911 MB against a 300 MiB volume and a 285 MB baseline. Uploading also sends twice the bytes.
- **Fix:**
  - (a) When a series' volume is kept, drop that series from the cache. The volume answers
    for it (small).
  - (b) Keep frames as stored (`u16`/`i16` plus rescale, as `HeldVolume` does). Upload them
    as R16 and rescale in the shader (medium; it touches the slice shader).

### 4. Two pools of one thread per core each, plus header readers (small)

- **Where:** `crates/volsa-app/src/worker.rs:182-183`, `crates/volsa-app/src/worker/pool.rs:10-16`,
  `crates/volsa-dicom/src/index/threads.rs:58`.
- **Cost:** 40 rayon threads on 20 cores before anything is open, and 85 threads in all on
  the welcome screen. Each has 8 MiB of stack reserved; that is address space, not committed
  memory. During a volume build both pools run flat out, so 40 busy threads share 20 cores.
  The decode a viewport asked for then competes with the build it was kept apart from.
- **Fix:** Give `volsa-build` about `cores - 2` threads (at least 1). Give `volsa-worker` a
  small fixed share, for example `max(2, cores / 4)`, so a viewport's decode always has a
  free core. Measure first-slice time during a build before and after.

### 5. Every GPU driver and Vulkan load at start (small; measure first)

- **Where:** `crates/volsa-app/src/device.rs:14-27`. Backends are left at egui-wgpu's
  default.
- **Cost:** On the welcome screen the process had loaded `vulkan-1.dll`, the NVIDIA driver
  (`nvwgf2umx.dll`, 90 MB image) and the Intel driver (`igd12um64xel.dll` and others). Private
  memory was 285 MB before anything was opened. In `measure_big`, finding the headless device
  (`volsa-render/src/gpu/found.rs:103`) took 1.66 s and picked Vulkan.
- **Fix:** On Windows, set `instance_descriptor.backends = DX12` and `power_preference =
  HighPerformance`. Do the same for the headless device. Measure start time and private
  memory before and after. Keep `VOLSA_ADAPTER=software` working.

### 6. Preferences are written to disk on the UI thread on every frame they change (small)

- **Where:** `crates/volsa-app/src/ui/remembered.rs:60-71`, `crates/volsa-app/src/prefs.rs:307-320`.
- **Cost:** `remember` runs every frame. It builds a full `Prefs` (several `String`s) and
  compares it with the saved one. If anything differs, `save` writes the file right there on
  the UI thread. The "pointer held" guard only knows about egui's own pointer. Dragging the
  window's border with the OS changes `window` on every frame, so each frame of the drag
  writes the file.
- **Fix:** Debounce it like the places are (`kept.pace`): write once the prefs have stayed
  the same for about a second, and on exit. Only compare when the screen's own numbers have
  changed.

### 7. Each action clones the list of notices; notices without a key have no limit (small)

- **Where:** `crates/volsa-app/src/session.rs:215`, `crates/volsa-app/src/log.rs:239-241`,
  `crates/volsa-app/src/reducer.rs:251`.
- **Cost:** Every action clones `state.notices` with all its strings. That includes every
  worker report (one per decoded slice) and every `CineTick`. A quadratic `contains` follows.
  Notices without a key are pushed with no cap, so the cost grows over a long session.
- **Fix:** Note `notices.len()` (or keep a counter of notices) before `reduce`, and log only
  those after it. Cap notices without a key, for example at the last 50.

### 8. `looks()` is rebuilt every frame (small)

- **Where:** `crates/volsa-app/src/ui/labels.rs:188`, `crates/volsa-app/src/labels.rs:101-108`.
- **Cost:** Every frame with labels on: a `Vec<PartLook>` with one entry per part (117 for a
  full TotalSegmentator card), a string comparison per part, then a comparison with the last
  one sent. It is cheap, but it runs per frame for nothing.
- **Fix:** Have the reducer increase a `looks_changed` counter on `LabelsState`, and rebuild
  only when it moves.

### 9. Each viewport and plane builds its own pipeline (small)

- **Where:** `crates/volsa-app/src/ui/setup.rs:63` (4 × `SlicePipeline::new`),
  `crates/volsa-app/src/ui/planes.rs:74` (3 × `PlanePipeline::new`).
- **Cost:** Seven shader modules and render pipelines are compiled at start, four of them
  even with a one-viewport layout. Each also has its own uniform buffer and samplers. The
  uniforms are written every frame, changed or not (`viewport.rs:345`, `planes/gpu.rs`).
- **Fix:** Build one pipeline and layout per kind and share it through an `Arc`. Keep a
  uniform buffer and bind group per pane. Skip `write_buffer` when the params haven't
  changed. As with finding 1, wait until the slice work lands.

### 10. Thumbnails: every series is checked every frame (small)

- **Where:** `crates/volsa-app/src/ui/frame.rs:16-43`.
- **Cost:** Each frame loops over every series, clones each `SeriesId` (a `String`) for the
  `windows` lookup, and locks the cache for any series still waiting. That adds up on a
  500-series study.
- **Fix:** Keep a set of series still waiting for a thumbnail. Clear it when the index
  changes, and check only those.

### 11. The CPU copy of the volume stays after it reaches the device (large; later)

- **Where:** `crates/volsa-app/src/volume_store.rs`, `crates/volsa-app/src/ui/scene/gpu.rs:170-198`.
- **Cost:** This is by design: measuring, labelling and the 2D viewports read it. Even so, a
  series under the budget is held in full on both the CPU and the GPU. In the measurement
  that was 300 MiB each.
- **Fix:** Do findings 1 and 2 first. Then a volume on the device could be spilled to disk
  ahead of need. It would come back only for labelling and region statistics, which run on
  workers. This one is worth deciding later, not now.

## Suggested order

Do 3a, 6, 7, 4 and 5 now: they are small and independent. Do 2 (the `value` part) as well.
Do 1, 9 and 3b together once the slice work in progress has landed. Leave 11 for later.

## Done the same day (branch `perf/small-fixes`)

Some findings looked different once the fixes were started:

- **2 (the `value` part):** done. `HeldVolume::value_in_place` reads one number where it
  lies, and the value under the pointer uses it. A slab that has been put away stays away.
- **3a:** already in the code. `keep` in `worker/volume.rs` forgets the series' slices once
  its volume is kept, and `request_frames` decodes none for a held series. Most of the
  911 MB is something else, still to be explained. 3b still stands.
- **4:** `volsa-build` now has `cores - 2` threads, at least 1. `volsa-worker` stays at one
  thread per core: it also runs the susceptibility map's parallel steps and the first burst
  of nearby slices, and both would be slower with fewer threads.
- **5:** the window was not on DirectX 12. It chose **Vulkan** among all the backends. With
  `WGPU_BACKEND` set, on the welcome screen:

  | Backend | Private memory | Drivers loaded |
  |---|---|---|
  | `vulkan` | 200 MB | Vulkan, NVIDIA, a little of Intel |
  | `dx12` | 298 MB | every DirectX 12 driver on the machine, both cards |
  | none set | about 285 MB | both |

  Forcing DX12 would have changed the backend and used more memory. The window now looks on
  Vulkan alone where a card answers there, on Windows and Linux (where it skips OpenGL),
  and on every backend otherwise. The Linux saving is not measured here.
  `VOLSA_ADAPTER=software` and `WGPU_BACKEND` still have the last word.
- **6:** done. Preferences are written once they have stayed the same for a second, and when
  the window closes.
- **7:** done. Notices with no key are kept to the newest 50. That limits the clone every
  action makes.
- **8:** done. The looks are compared part by part as they are made, and a `Vec` is made only
  when they have changed.
- **10:** dropped. The loop only clones for series that still have no thumbnail, so there is
  little to gain. Making it skip work safely would mean following when the window becomes
  ready.

After the change, the release build started from a shell with nothing set: 200 MB private
memory on the welcome screen (was about 285 MB), on Vulkan. Gate green: 2,546 tests.
One oddity, not looked into: windows started from the PowerShell tool with no backend set
stopped with "Failed to create surface for any enabled backend: {}". That happened with the
old build as well. Started from bash, or with `WGPU_BACKEND` set, they opened normally.

## Done on 17 November

- **9:** one slice shader and one plane shader are compiled for the window's device and kept
  with its renderer; every viewport, plane panel and slice drawn from the card shares them
  (eleven pipelines before). Each pane keeps its own uniform buffer and writes it only when
  its numbers change.
- **3b:** `Frame::values` is now `Values`: the 16-bit numbers the file stores with their
  rescale where it stores them, else 32-bit values. The frame cache counts half the bytes for
  such slices, and the viewport sends them to the card as `R16Uint`; the slice shader turns
  them into values. Values read on the processor are the same to the bit. Slices the cache
  takes from a held volume stay 16-bit too (`HeldVolume::read_slice`).
- **11:** once the scene reports its volume on the card whole (not halved), the workers put
  every whole slab of it in the spill file. Volumes under 64 MiB are left in memory. The value
  under the pointer reads one number from disk; measuring and labelling bring back what they
  read. Not yet measured in the window on a large scan: the 2024 study's series are small.
- On the 2024 study's first series, private memory after opening went from 357 MB to 316 MB.
  A capture of the SWAN series in Slices and 3D is pixel-identical apart from the value under
  the pointer, which differs from run to run with either build.
