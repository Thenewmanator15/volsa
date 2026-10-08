# Architecture

Voxa is a Cargo workspace of small crates that talk only through their public interfaces.

| Crate | Owns | Depends on |
|---|---|---|
| `voxa-core` | Data model (`StudyInfo`, `SeriesInfo`), typed coordinate spaces (`Point<S>`, `Affine<A, B>`), `WindowLevel`, `VoxaError` | nothing in the workspace |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing into studies and series, geometry checks, pixel decoding, tag listing | `voxa-core` |
| `voxa-volume` | Histograms, percentiles, automatic window/level | `voxa-core` |
| `voxa-render` | The GPU slice pipeline and the view transform | `voxa-core` |
| `voxa-app` | The window, interface, application state, frame cache and background workers | all of the above |
| `xtask` | `cargo xtask check` and `cargo xtask coverage` | nothing in the workspace |

Dependencies point downward only: core, then dicom and volume, then render, then app. `voxa-app`
contains no image mathematics; anything that computes on pixels or coordinates lives below it,
where it can be tested without a window.

## Data flow

1. A `SeriesSource` yields named byte streams from a folder, a zip (read in place, nothing is
   extracted to disk) or the folder beside a `DICOMDIR`.
2. `voxa-dicom` parses headers on a worker and emits an index of studies and series. The browser
   fills as results arrive. Entries that are not DICOM are skipped and counted.
3. Selecting a series queues decoding of its slices on worker threads, nearest slices first.
   Decoded frames are cached under a memory budget.
4. `voxa-render` uploads the current frame once as a single-channel floating-point texture. A
   shader applies window/level, so changing contrast never decodes or uploads again.
5. The app draws overlays (orientation letters, scale bar, corner text) with `egui` above the GPU
   viewport.

## State

All application state lives in one `AppState`. It changes only through one `Action` enum handled
by one pure function, `reduce`, which returns the `Effect`s to run (start indexing, request
frames). Workers report back by sending further `Action`s. This makes every interaction testable
without a window: dispatch actions, assert on state.

## Coordinates

DICOM patient coordinates are left-posterior-superior (LPS) millimetres: +x towards the patient's
left, +y towards the back, +z towards the head. Voxel coordinates are (column, row, slice). Screen
coordinates are pixels in a viewport. They are distinct types; see
[adr/0002-coordinates.md](adr/0002-coordinates.md).

## Errors

Nothing a file contains may crash the application; see [adr/0003-errors.md](adr/0003-errors.md).
