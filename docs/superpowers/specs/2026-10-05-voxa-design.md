# Voxa design

Date: 2026-10-05, revised 2026-10-07
Status: draft for review

Revision of 2026-10-07: the 3D view becomes the main view and gains direct
interaction (A8); the roadmap is re-ordered and extended with model-based brain
labelling and the inner ear, both proven in the earlier prototypes
(A4); the methods, constraints and risks are updated to match (A5, A7).

Voxa is an open-source medical image viewer written in Rust. It aims to combine
what today is split across separate tools: one-step opening of a patient's own
scan export, a clinical-style reading interface, comparison between time
points, selectable 3D brain regions, and research-grade quantitative tools,
behind a modern interface in a single native executable.

**Voxa is not a medical device and is not for diagnostic use.** This statement
appears in the README, the About screen and the main window.

This document has two parts. Part A describes the whole product and the order
it is built in. Part B specifies milestone 1 in enough detail to plan and
build. Each later milestone gets its own spec before work on it starts.

---

## Part A: the whole product

### A1. Who it is for

| Audience | What they need |
|---|---|
| A person viewing their own scans | Drop a zip, folder or disc export and see it; compare studies over time; simple measuring |
| Researchers | Reproducible measurements, registration, segmentation, per-region statistics, export, scripting |
| Clinicians reading studies | Layout presets, synced series, archive connectivity, structured reports, multiple monitors |
| Developers and reviewers | A well-engineered, maintainable Rust codebase that is easy to extend |

The first user is the project owner, on Windows, with their own scan exports as the
first real data.

### A2. Decisions already made

- Pure Rust; `egui` for the interface, `wgpu` for rendering, `dicom-rs` for DICOM.
- Windows first; Linux in continuous integration from the start; macOS later.
- Dual MIT / Apache-2.0 licence.
- Local-only repository until the owner chooses to publish.
- No network access and no telemetry until the archive-connectivity milestone,
  and then only to servers the user configures.
- Patient data never enters the repository. Automated tests use synthetic
  volumes and public sample datasets.
- Name: Voxa. Free on crates.io as of 2026-10-05; other unrelated projects on
  GitHub share the name, so the repository may need a qualifier when published.

### A3. Architecture

A Cargo workspace. Crates talk only through their public interfaces, and the
interface crate contains no image mathematics.

| Crate | Responsibility | Depends on |
|---|---|---|
| `voxa-core` | Shared data model: study, series, volume, typed coordinate spaces, errors | none |
| `voxa-dicom` | Open zip / folder / DICOMDIR, index headers, decode pixels, build geometry | `voxa-core`, `dicom-rs` |
| `voxa-volume` | Interpolation, reslicing maths, histograms, automatic window/level | `voxa-core` |
| `voxa-filters` | Denoising, bias correction, later skull-stripping | `voxa-volume` |
| `voxa-register` | Rigid, affine and deformable registration | `voxa-volume` |
| `voxa-models` | ONNX runtime wrapper, model download and checksum verification, quality verdicts | `voxa-volume` |
| `voxa-segment` | Brain extraction and labelling, inner-ear outline and parts, label volumes, structure statistics | `voxa-models`, `voxa-register` |
| `voxa-render` | GPU slice renderer, volume ray-caster, label rendering, picking, cut planes | `voxa-volume`, `wgpu` |
| `voxa-app` | Interface, tools, application state, settings | all of the above, `egui` |

Extension points are small traits with a registry, so a new item is a new file
and no existing file changes: `SeriesSource` (where data comes from), `Filter`,
`Tool` (mouse interaction and its annotations), `Layout`, `Exporter`.

Three coordinate spaces are distinct types: `Patient` (mm, DICOM LPS), `Voxel`
(indices into a volume) and `Screen` (pixels in a viewport). Converting between
them requires an explicit transform, so mixing them is a compile error.

Application state changes only through an explicit `Action` enum handled in one
place. This makes behaviour traceable and gives undo and redo.

`egui`, `wgpu` and `dicom-rs` are each wrapped behind a thin internal interface
so that an upstream API change touches one module.

### A4. Feature inventory and milestones

Every feature requested, built in the earlier prototypes, or
identified in the survey of existing viewers is listed against the milestone
that delivers it.

| # | Milestone | Delivers |
|---|---|---|
| 1 | Foundation | Workspace, CI, quality gates. Open zip / folder / DICOMDIR, classic and enhanced DICOM. Study and series browser with thumbnails. Automatic choice of the most useful series on opening, with the reason each other series was passed over. 2D stack view with zoom, pan, flip, rotate. Window/level by drag, presets and automatic. Orientation labels, scale bar, corner overlay. DICOM tag browser with search. Cine playback. Dark modern theme. |
| 2 | Interactive 3D | The main view. Volume assembly in patient space. GPU ray-cast volume rendering with presets and maximum-intensity projection. Orbit, pan, zoom, saved camera views. A movable cut plane at any angle. The scan's slice planes drawn inside the 3D scene. Snapshot and rotating-video export. Window layout: one large 3D viewport with supporting panels beside it. |
| 3 | Plane views and measurement | Linked axial / coronal / sagittal panels beside the 3D view, with crosshairs shared with the 3D scene. Cubic B-spline reslicing, oblique planes, reference lines, synced scrolling between series. Ruler, angle and region statistics in the plane views; point-to-point distance and labelled markers in 3D. Measurement list, CSV / JSON export. Undo and redo. |
| 4 | Models and brain labelling | An ONNX runtime inside the app, with models downloaded on first use and checksum-verified. Brain extraction. Whole-brain labelling into about thirty structures on any contrast. A quality verdict in plain words on every automatic result. Label volumes rendered as coloured solids. The structure list: hover for a name, click to select, show / hide / isolate, opacity per structure, volume per structure. Selection linked between 3D, plane views and the list. |
| 5 | Inner ear | Locate both inner ears on a fluid-bright scan. Outline each labyrinth. Split it into cochlea, vestibule and the three semicircular canals. Solid and scan-signal views, per-part volumes, left against right. The same structure list and interactions as milestone 4. |
| 6 | Time-point comparison | Rigid registration between studies, with a local refinement around a chosen structure. Synced side-by-side 3D and plane views, overlay in two colours, swipe, fade, checkerboard, difference map. Saved transforms. Structure volumes compared across time. |
| 7 | Image enhancement | N4 bias-field correction, denoising, and slice sharpening across thick slices, each as a non-destructive derived volume with progress and cancel, and each stating what it can and cannot recover. |
| 8 | Research tools | Click-to-outline in 3D with a permissively licensed model. Segmentation editor (brush, threshold, region growing, interpolation between slices). DICOM SEG and NIfTI import / export. Scripting and batch processing. A summary report of volumes and quality verdicts. |
| 9 | Workstation | Hanging protocols. DICOMweb and classic PACS query / retrieve. DICOM structured-report export. Multi-monitor layouts. Key images. |

Each milestone ends with a tagged release that is usable on its own.

### A5. Methods from the literature

| Feature | Method | Reference |
|---|---|---|
| Reslicing | Cubic B-spline interpolation with prefiltering | Thévenaz, Blu and Unser, IEEE TMI 2000 |
| Fast GPU cubic sampling | Eight trilinear taps | Sigg and Hadwiger, GPU Gems 2, 2005 |
| Denoising | Non-local means adapted to Rician noise | Manjón et al., JMRI 2010 |
| Bias correction | N4ITK | Tustison et al., IEEE TMI 2010 |
| Rigid registration metric | Mutual information with Parzen windows | Mattes et al., IEEE TMI 2003 |
| Deformable registration | Symmetric diffeomorphic normalisation, or B-spline free-form deformation | Avants et al., MedIA 2008; Rueckert et al., IEEE TMI 1999 |
| Skull-stripping | Deformable surface model first; learned model later | Smith, HBM 2002; Hoopes et al., NeuroImage 2022 |
| Contrast-agnostic segmentation | SynthSeg | Billot et al., MedIA 2023 |
| Volume rendering | GPU ray-casting with early termination and empty-space skipping | Krüger and Westermann, IEEE Vis 2003 |
| Brain extraction | HD-BET | Isensee et al., Human Brain Mapping 2019 |
| Brain labelling quality score | SynthSeg 2.0 automated QC, fail below 0.65 | Billot et al., PNAS 2023 |
| Inner-ear outline | IE-Vnet | Ahmadi et al., Frontiers in Neurology 2022 |
| Mask interpolation between slices | Shape-based interpolation of signed distance maps | Raya and Udupa, IEEE TMI 1990 |
| Slice sharpening | SMORE self-supervised super-resolution | Zhao et al., IEEE TMI 2021 |

Brain labelling uses SynthSeg (already listed) as the primary method; registering
a labelled atlas is kept only as a fallback. The labyrinth is split into its five
parts by the geometric method written in the prototypes (thick parts by
morphological opening and watershed, canals by plane fitting), since no published
model with usable weights does it.

Each method lives in its own module that cites the paper, states which
equations it implements, and has a test reproducing a known result. Where a
reference implementation exists (ITK, ANTs), results are compared numerically
on public data. Atlas and model licences are checked before either is bundled.

### A6. Engineering practice

- Test-first. No mathematics or rendering code lands without a test.
- CI on Windows and Linux: `cargo fmt --check`, `clippy -D warnings`, tests,
  `cargo deny` (licences and advisories), documentation build. A failing check
  blocks merging.
- Library crates never panic: typed errors with `thiserror`; `unwrap` and
  `expect` are denied by lint outside tests.
- `#![forbid(unsafe_code)]` in every crate except `voxa-render`, where each use
  carries a safety comment.
- Soft size limits by lint: about 400 lines per file, 60 per function.
- Every public item documented. Architecture overview, contributor guide, and
  "how to add a filter / tool / source" walkthroughs in `docs/`.
- Architecture decision records in `docs/adr/` for each significant choice.
- Pinned toolchain (`rust-toolchain.toml`), committed `Cargo.lock`, stated
  minimum supported Rust version checked in CI.
- Semantic versioning, conventional commits, changelog, small pull requests.
- Weekly automated dependency update pull requests gated by the full suite.
- Golden-image tests for rendering; numerical regression tests for filters and
  registration; coverage reported, with a floor of 80% on non-GPU crates.
- Benchmarks (`criterion`) for decode, reslice, filters and registration.
- Structured logging (`tracing`) that never records patient identifiers.
- Settings and saved annotations carry a format version with migrations.
- One command (`cargo xtask check`) runs exactly what CI runs.

### A7. Known constraints and risks

- **Not certifiable as built.** Clinical-style features do not make it a
  diagnostic tool; the disclaimer stays until someone funds certification.
- **Models must run inside a Rust app.** SynthSeg (Keras), IE-Vnet and HD-BET
  (PyTorch) have to be converted to ONNX and reproduce the Python results on
  the same inputs before milestone 4 or 5 can ship. Milestone 4 starts with a
  feasibility spike on exactly this. If a model will not convert, the fallback
  is a small bundled helper process, which costs the single-executable goal.
- **Licences decide what can ship.** SynthSeg, IE-Vnet and SMORE are Apache 2.0
  and can be used. The nnInteractive weights (CC BY-NC-SA) and the BM4D package
  (non-commercial) cannot be bundled in an MIT / Apache application: click-to-
  outline needs a permissively licensed model, and denoising is implemented
  from the non-local-means paper. Every model's weights licence is checked
  before it is added.
- **Automatic results can look right and be wrong.** In the prototypes a poor
  scan produced labels as polished as a good one. Every automatic result
  carries a quality verdict in plain words, and a failed verdict is shown on
  the result itself, not in a log.
- **Rust library gaps.** Registration, N4, and the labyrinth split have no
  mature Rust libraries. They are implemented from the papers and the
  prototypes and validated against the Python outputs. This is the largest
  cost in the project.
- **Input quality limits output.** Thick-slice or partial-coverage scans
  restrict what 3D rendering, labelling and comparison can achieve. In the
  prototypes, denoising did nothing and sharpening gained about a quarter in
  edge strength on a scan limited by slice thickness and metal artefact. The
  interface says so plainly instead of showing confident-looking results.
- **A 3D-first interface needs a capable GPU path.** Ray-casting, label
  rendering and picking run every frame. Milestone 2 sets a frame-rate target
  on integrated graphics and a reduced-quality mode while the camera moves.
- **`egui` text and layout** are weaker than a web interface. Custom theming
  and fonts are needed from milestone 1 to avoid a developer-tool look.
- **Scope.** Nine milestones is a multi-month programme. Each is planned and
  shipped separately; later ones may be re-ordered once earlier ones are used.

### A8. The 3D view

The 3D viewport is the main view, not a fourth panel. From milestone 2 the
window is one large 3D viewport with supporting panels in a column beside it
(the plane views from milestone 3, the structure list from milestone 4). Any
panel can be enlarged to fill the window and restored.

Everything in the scene is an object the user can act on directly:

- **Camera.** Drag to orbit, middle-drag or Shift-drag to pan, scroll to zoom
  towards the cursor, double-click a surface to centre on it. Named views
  (front, back, left, right, top, bottom) and user-saved views.
- **Hover and select.** Hovering a labelled structure shows its name beside the
  cursor. Clicking selects it: it is outlined in 3D, tinted in the plane views,
  and highlighted in the structure list. Clicking empty space clears the
  selection.
- **Show, hide, fade.** Each structure has a visibility toggle and an opacity
  slider in the list. "Isolate" shows one structure alone; "Show all" restores.
  Opacity presets peel the outer layers to reveal what is inside.
- **Cut plane.** A plane that can be dragged along its normal and rotated by a
  handle. One side of it is removed. The cut face shows the scan signal or the
  label colours, chosen by the user. Up to three cut planes.
- **Slice planes in the scene.** The axial, coronal and sagittal slices
  currently shown in the plane views are drawn in place inside the 3D scene and
  move as the user scrolls. Dragging one in 3D scrolls its plane view.
- **Measure and annotate.** Click two points on any surface for a straight-line
  distance in millimetres. Place labelled markers. Both are listed with the
  plane-view measurements, exported with them, and undoable.

Picking is done by rendering structure identifiers to an off-screen target, so
hover and click cost one texture read and stay exact at any zoom. While the
camera moves the renderer may drop resolution, and it returns to full quality
when the camera rests.

---

## Part B: milestone 1 specification (Foundation)

### B1. Goal

A user can drop a scan export onto Voxa, browse its studies and series, and
view any series as a 2D stack with correct orientation, contrast controls,
overlays, tag inspection and cine playback. The repository has every quality
gate in A6 running from the first commit.

### B2. In scope

1. Workspace skeleton: `voxa-core`, `voxa-dicom`, `voxa-volume` (histogram and
   automatic window/level only), `voxa-render` (2D slice pipeline only),
   `voxa-app`, and an `xtask` crate.
2. CI, lints, `cargo deny`, toolchain pin, licences, README with disclaimer,
   contributor guide, first ADRs (stack, coordinate conventions, error policy).
3. Sources: a folder, a `.zip`, or a `DICOMDIR`, by drag-and-drop, file dialog
   or command-line argument. Zips are read in place without extracting to disk.
   Classic DICOM (one file per slice) and enhanced DICOM (a series in one
   multi-frame file) are both read.
4. Indexing reads headers only and groups instances into studies and series,
   ordered by position along the slice normal with instance number as fallback.
5. Pixel decoding for uncompressed, RLE, JPEG baseline and lossless, and JPEG
   2000 transfer syntaxes; rescale slope / intercept; MONOCHROME1 inversion.
6. Study browser: studies by date, series cards with thumbnail, description,
   slice count and slice thickness. On opening, the most useful series (the
   one covering the most anatomy with the thinnest slices, among those without
   problems) is selected automatically; each other card says why it was not.
7. Stack viewport: scroll through slices, zoom, pan, flip, rotate by 90°, fit.
8. Window/level: right-drag, numeric entry, presets, and automatic windowing
   from robust histogram percentiles (default 1st to 99.5th).
9. Overlays: orientation letters derived from image orientation, mm scale bar,
   corner text (study date, series description, slice index and position,
   window/level). Patient name and ID are hidden by default with a toggle.
10. Radiological display convention by default, with a labelled toggle.
11. DICOM tag browser for the current instance with search.
12. Cine playback with adjustable frame rate and loop.
13. Layouts: 1×1, 1×2, 2×2 of independent stack viewports. The window is
    built as one main area with a side column, so that the 3D viewport of
    milestone 2 takes the main area without rearranging the interface.
14. Theme: dark, custom typography and icon set, keyboard shortcuts with an
    in-app reference.

### B3. Out of scope for milestone 1

3D rendering, reslicing and three-plane views, measurements, filters,
registration, model-based labelling, the inner ear, segmentation, any network access, editing or anonymising DICOM,
colour and multi-frame ultrasound cine, macOS packaging.

### B4. Data flow

1. A `SeriesSource` yields named byte streams.
2. `voxa-dicom` parses headers on a worker pool and emits an index of
   `Study { series: Vec<SeriesInfo> }`; the browser updates as results arrive.
3. Selecting a series queues decoding of its instances on worker threads.
   Decoded frames are cached under a memory budget, nearest slices first.
4. `voxa-render` uploads the current frame as a single-channel texture; a
   shader applies rescale, window/level and inversion, so contrast changes
   never re-decode or re-upload.
5. The app composes overlays in `egui` above the GPU viewport.

### B5. Errors

- An unreadable or non-DICOM file is skipped and counted; the browser shows
  "n files skipped" with a details list.
- An unsupported transfer syntax marks the series as unavailable with the
  syntax name; other series are unaffected.
- A series with inconsistent image sizes or missing geometry opens as a plain
  stack with orientation labels and the mm scale bar disabled, and says why.
- Decoding failure of one slice shows a placeholder for that slice only.
- No input can crash the application; fuzz tests cover the header parser path.

### B6. Testing

- `voxa-core`, `voxa-volume`: unit and property tests (coordinate round trips,
  histogram percentiles against a reference).
- `voxa-dicom`: synthetic DICOM files generated in tests for each supported
  transfer syntax; public sample sets for integration; fuzzing of indexing.
- `voxa-render`: golden-image tests through a headless `wgpu` adapter, with a
  small tolerance and software-rasteriser fallback in CI.
- `voxa-app`: state tests that dispatch `Action`s and assert on state, with no
  window; a smoke test that opens a synthetic study end to end.
- Manual acceptance on a real export, locally: every study and series
  listed correctly, every slice displays, left/right labels verified against
  the vendor viewer.

### B7. Done when

- All items in B2 work on Windows 11 and pass CI on Windows and Linux.
- A real export's zip opens by drag-and-drop in under two seconds to the browser,
  and scrolling a 64-slice series holds 60 frames per second.
- `cargo xtask check` is clean, coverage floor met, docs build without warnings.
- Tagged `v0.1.0` with a changelog entry and a Windows build artefact.
