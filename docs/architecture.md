# Architecture

Voxa is a Cargo workspace of small crates that talk only through their public interfaces.

| Crate | Owns | Depends on |
|---|---|---|
| `voxa-core` | Data model (`StudyInfo`, `SeriesInfo`, `Volume`), typed coordinate spaces (`Point<S>`, `Affine<A, B>`), `WindowLevel`, `VoxaError` | nothing in the workspace |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing into studies and series, geometry checks, pixel decoding, tag listing | `voxa-core` |
| `voxa-volume` | Assembling a series into a volume; sampling it, lines of sight through it, and reducing it; histograms, percentiles, automatic window/level | `voxa-core` |
| `voxa-render` | The GPU slice pipeline and the view transform; the GPU volume pipeline (a 3D texture and a shader that follows a line of sight through it for every pixel), its camera and scene | `voxa-core`, `voxa-volume` |
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

The 3D scene follows the series of the active viewport:

1. A worker decodes every slice of the series, taking those already in the frame cache, and
   `voxa-volume` stacks them into a `Volume`: the values, and the transform that places each
   voxel in the patient. The one volume in use is kept in a store shared with the window.
   A series is decoded once and held once: while its volume is being built the viewport's own
   request decodes only the slice on show, and once the volume is kept the series' frames
   leave the cache and the 2D viewport reads its slices out of the volume. A volume that gives
   way to another, or a build that fails, leaves its slices in the frame cache.
2. The same worker makes the volume ready for the graphics device: 16-bit values from 0 at the
   smallest to 1 at the largest, halved first if the device cannot hold it, with the largest
   value in each block of voxels. The window only hands the result over as two 3D textures.
3. For every pixel a fragment shader builds the line of sight from the camera, clips it to the
   volume's box in texture space (so a tilted stack needs no resampling), steps past the blocks
   with nothing to draw, and walks the rest: adding up lit, windowed values (Surface, VR) or
   keeping the brightest (MIP). Cut planes shorten the line; the 2D slice is an opaque sheet.
   In VR and MIP a step is half a voxel along the axis the line crosses fastest; in Surface it
   is half the least distance between voxels from every side.
4. The picture is drawn off the screen, at half size and half quality while the camera moves,
   and shown by `egui` as an image. The same path draws the pictures that are saved; a
   turntable's pictures are put together into its file on a worker.
5. The camera has no perspective and lives in patient space, so nothing is mirrored, a scale bar
   holds across the picture, and the letters at the edges come from the same rule as in 2D.

## State

All application state lives in one `AppState`. It changes only through one `Action` enum handled
by one pure function, `reduce`, which returns the `Effect`s to run (start indexing, request
frames, build a volume, read and write saved views). Workers report back by sending further `Action`s. This makes every interaction testable
without a window: dispatch actions, assert on state.

## Coordinates

DICOM patient coordinates are left-posterior-superior (LPS) millimetres: +x towards the patient's
left, +y towards the back, +z towards the head. Voxel coordinates are (column, row, slice). Screen
coordinates are pixels in a viewport. They are distinct types; see
[adr/0002-coordinates.md](adr/0002-coordinates.md).

## Errors

Nothing a file contains may crash the application; see [adr/0003-errors.md](adr/0003-errors.md).

## Decoding and what the tests cover

`decode_slice` reads images stored uncompressed, with run-length encoding (RLE), as JPEG
(baseline, extended and lossless) and as JPEG 2000 (lossless and lossy). Any other compression is
named on the series card when the scan is indexed, and decoding it returns
`UnsupportedTransferSyntax` with the same name. Colour images are declined.

The decoders for compressed images come from `dicom-pixeldata`. They are other people's code fed
with whatever a file holds, so each call is wrapped: a panic inside a decoder becomes a `Decode`
error for that one slice.

Tests build their files in memory with the `testkit` feature of `voxa-dicom`. They cover
uncompressed pixels (unsigned, signed, 8-bit, `MONOCHROME1`, per-frame rescale in a multi-frame
file) and RLE, for which the test writes the compressed frame by hand. No JPEG or JPEG 2000
encoder is among the development dependencies, so those two are not covered by an automated test;
they are checked by hand against real exports in the acceptance test at the end of milestone 1.
