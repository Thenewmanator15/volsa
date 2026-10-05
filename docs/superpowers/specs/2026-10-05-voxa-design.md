# Voxa design

Date: 2026-10-05
Status: draft for review

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
| `voxa-segment` | Atlas regions, label volumes, region statistics, later learned models | `voxa-register` |
| `voxa-render` | GPU slice renderer and volume ray-caster | `voxa-volume`, `wgpu` |
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

Every feature requested or identified in the survey of existing viewers is
listed here against the milestone that delivers it.

| # | Milestone | Delivers |
|---|---|---|
| 1 | Foundation | Workspace, CI, quality gates. Open zip / folder / DICOMDIR. Study and series browser with thumbnails. 2D stack view with zoom, pan, flip, rotate. Window/level by drag, presets and automatic. Orientation labels, scale bar, corner overlay. DICOM tag browser with search. Cine playback. Dark modern theme. |
| 2 | Three-plane and measurement | Volume assembly in patient space. Linked axial / coronal / sagittal views with crosshairs. Cubic B-spline reslicing. Oblique planes. Reference lines and synced scrolling between series. Ruler, angle, elliptical region statistics. Measurement list, CSV / JSON export. Undo and redo. |
| 3 | 3D rendering | GPU ray-cast volume rendering, transfer-function presets and editor, cut planes, maximum-intensity projection, screenshot export. |
| 4 | Image filters | Non-local means denoising for MRI, N4 bias-field correction, as non-destructive derived volumes with progress and cancel. |
| 5 | Time-point comparison | Rigid registration between studies. Synced side-by-side, swipe, fade, checkerboard, colour overlay and difference map. Saved transforms. |
| 6 | Brain regions | Skull-stripping. Affine plus deformable registration of a published labelled atlas. Clickable regions in 3D and 2D, region list, per-region volume and intensity statistics. |
| 7 | Research tools | Segmentation editor (brush, threshold, region growing, interpolation between slices). Learned whole-brain segmentation through an ONNX runtime. DICOM SEG and NIfTI import / export. Scripting and batch processing. |
| 8 | Workstation | Hanging protocols. DICOMweb and classic PACS query / retrieve. DICOM structured-report export. Multi-monitor layouts. Key images. |

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
- **Rust library gaps.** Registration, N4, skull-stripping and segmentation
  have no mature Rust libraries. They are implemented from the papers and
  validated against ITK / ANTs output. This is the largest cost in the project.
- **Input quality limits output.** Thick-slice or partial-coverage scans
  restrict what 3D rendering,
  atlas registration and segmentation can achieve. The interface must say so
  plainly instead of showing confident-looking results.
- **Atlas registration on non-T1 contrast** is less reliable; milestone 6
  begins with a feasibility spike on a real scan and public T2 data.
- **`egui` text and layout** are weaker than a web interface. Custom theming
  and fonts are needed from milestone 1 to avoid a developer-tool look.
- **Scope.** Eight milestones is a multi-month programme. Each is planned and
  shipped separately; later ones may be re-ordered once earlier ones are used.

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
4. Indexing reads headers only and groups instances into studies and series,
   ordered by position along the slice normal with instance number as fallback.
5. Pixel decoding for uncompressed, RLE, JPEG baseline and lossless, and JPEG
   2000 transfer syntaxes; rescale slope / intercept; MONOCHROME1 inversion.
6. Study browser: studies by date, series cards with thumbnail, description,
   slice count and slice thickness.
7. Stack viewport: scroll through slices, zoom, pan, flip, rotate by 90°, fit.
8. Window/level: right-drag, numeric entry, presets, and automatic windowing
   from robust histogram percentiles (default 1st to 99.5th).
9. Overlays: orientation letters derived from image orientation, mm scale bar,
   corner text (study date, series description, slice index and position,
   window/level). Patient name and ID are hidden by default with a toggle.
10. Radiological display convention by default, with a labelled toggle.
11. DICOM tag browser for the current instance with search.
12. Cine playback with adjustable frame rate and loop.
13. Layouts: 1×1, 1×2, 2×2 of independent stack viewports.
14. Theme: dark, custom typography and icon set, keyboard shortcuts with an
    in-app reference.

### B3. Out of scope for milestone 1

Reslicing and three-plane views, measurements, 3D, filters, registration,
regions, segmentation, any network access, editing or anonymising DICOM,
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
