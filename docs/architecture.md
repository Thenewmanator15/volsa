# Architecture

Voxa is a Cargo workspace of small crates that talk only through their public interfaces.

| Crate | Owns | Depends on |
|---|---|---|
| `voxa-core` | Data model (`StudyInfo`, `SeriesInfo`, `Volume`), typed coordinate spaces (`Point<S>`, `Affine<A, B>`), `WindowLevel`, `VoxaError` | nothing in the workspace |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing into studies and series, geometry checks, pixel decoding, tag listing | `voxa-core` |
| `voxa-volume` | Assembling a series into a volume; sampling it, lines of sight through it, and reducing it; the planes of a volume through a point; distances, angles and the values in a region; the curve (cubic B-spline) through its values; histograms, percentiles, automatic window/level | `voxa-core` |
| `voxa-labels` | Reading a label volume (single-file NIfTI-1) and the table of names that goes with it; the parts of a label set with their volumes; which part lies at a point, and the first one a line of sight meets; a name's left and right | `voxa-core` |
| `voxa-render` | The GPU slice pipeline and the view transform; the GPU volume pipeline (a 3D texture and a shader that follows a line of sight through it for every pixel), its camera and scene; the GPU plane pipeline, which draws a flat plane through the same texture; labels on the device, drawn over the planes and as solids | `voxa-core`, `voxa-volume`, `voxa-labels` |
| `voxa-app` | The window, interface, application state, frame cache and background workers | all of the above |
| `xtask` | `cargo xtask check` and `cargo xtask coverage` | nothing in the workspace |

Dependencies point downward only: core, then dicom, volume and labels, then render, then app. `voxa-app`
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
   smallest to 1 at the largest, halved first if the device cannot hold it, and hands them
   to the device itself as a 3D texture: the device and its queue can be used from any
   thread, and the window, which has only to take the texture up, does not pause to copy
   bytes. The largest value in each block of voxels, which
   lets a line of sight step over what is empty, is then worked out by the device itself
   from that texture, by drawing into the slices of a second, small one; a worker works it
   out beforehand only for a device that cannot.
3. For every pixel a fragment shader builds the line of sight from the camera, clips it to the
   volume's box in texture space (so a tilted stack needs no resampling), steps past the blocks
   with nothing to draw, and walks the rest: adding up lit, windowed values (Surface, VR) or
   keeping the brightest (MIP). Cut planes shorten the line; the 2D slice is an opaque sheet.
   In VR and MIP a step is half a voxel along the axis the line crosses fastest; in Surface it
   is half the least distance between voxels from every side.
   Surface and VR draw nothing fainter than a tenth of the window, so the noise in the air
   about a patient is neither shown nor allowed to dim what is behind it.
4. The picture is drawn off the screen and shown by `egui` as an image. While the camera
   moves it is drawn as finely as the device keeps up with: the device is timed on the
   pictures it draws in full, by handing it one when it has nothing else in hand and waiting
   for it, and the moving picture is drawn in full, at three quarters or at a half of the
   size and fineness, whichever is the finest expected to fit in 8 ms. The same path draws
   the pictures that are saved; a turntable's pictures are put together into its file on a
   worker.
5. The camera has no perspective and lives in patient space, so nothing is mirrored, a scale bar
   holds across the picture, and the letters at the edges come from the same rule as in 2D.

The plane views and measuring hang on one point, the position:

1. The state holds one position in the patient, inside the scene's volume. Everything that
   moves through the volume moves it: a click in a plane view or on a surface in 3D, the wheel
   over a plane, a grip in the scene, and, while Link is on, scrolling a 2D viewport of the
   same study. Linked viewports in turn go to the slice nearest the position.
2. `voxa-volume` gives the three planes of the volume through the position. They follow the
   volume's own axes, each named by the anatomical plane it is nearest, so the scan's own
   slices are never resampled and a tilted scan is shown tilted.
3. Each plane is drawn by a small fragment shader that reads the scene's own 3D texture: for
   every pixel of the panel, the point of the patient it shows, then the value there, through
   the window. A plane view therefore costs no second copy of the volume. With Smooth on,
   the device turns the volume it holds into the coefficients of the cubic B-spline through
   its values: compute shaders run the spline's filter along every line of voxels, each way
   in turn, in a texture they read and write in place, and the result is copied into a
   second texture as large as the volume, which the same plane shader reads with eight
   blended reads a pixel. Nothing passes through the processor. A device with no compute
   shaders, or with no room for the working texture, leaves it to a worker, which hands the
   coefficients over as before.
4. Annotations (rulers, angles, regions, markers) are points in the patient, in millimetres,
   kept with the series they were made on. What each measures is worked out in `voxa-volume`
   from those points and from the scan's own values, never from the screen. A plane view shows
   the ones that lie in its layer; the 3D scene shows them all, in front of the picture.
5. The list is written to `annotations.json` in the settings folder, filed under a hash of
   the series, and can be saved as a table or as JSON. None of these holds anything that says
   whose scan it was.

Labelled parts come from files made elsewhere and are kept apart from the scan:

1. A worker reads the label file and, if there is one, the table of names beside it.
   `voxa-labels` turns them into a label set: for every voxel of the label volume the part it
   belongs to, the transform that places those voxels in the patient, and the parts with their
   names, colours, volumes and what the table says of how each was made. The workers keep the
   one set in use, as the volume store keeps the one volume.
2. The label volume keeps its own grid and is never resampled onto the scan's; see
   [adr/0004-labels-own-grid.md](adr/0004-labels-own-grid.md). Labels and scan meet in patient
   space only. The state holds what is shown of the labels (which parts, how solid, which is
   selected) and the study they were opened on; they are drawn on that study's series and on
   no other, and labels whose box lies nowhere near the series are refused.
3. The window puts one copy on the graphics device: the part numbers as a 3D texture of
   whole numbers, and a small texture of how each part looks, which is all that is sent again
   when a part is hidden, faded or selected.
4. The plane shader, for each pixel, finds the point of the patient it shows and reads the
   part there from the nearest label voxel: a line where the part changes, and a tint inside
   it when asked. The Parts look is a second scene shader that walks each line of sight
   through the label volume, finds where it enters a shown part, and lights that surface from
   how the part's own voxels slope; cuts shorten the line as they do for the scan. The
   device first finds which blocks of the label volume hold no part at all, and a line of
   sight goes straight to the far side of such a block.
5. Which part is under the pointer is worked out on the processor by `voxa-labels`, from the
   same label set, so naming and selecting a part read nothing back from the device.

## Kinds of series, and volumes of a series

An export holds more than volumes: locating images, single images, pictures in colour, stacks
that are not regular. `SeriesInfo::kind` names what a series is, from what the index found,
and the series list says it in words; only a stack that cannot be shown in 3D and images that
cannot be read are faults. What the files say of how a series was acquired (plane, 2D or 3D,
family of sequence, times, flip angle, field strength) is kept in `ScanFacts`, read from the
first image; images in several planes are given no plane.

A series that fills every position more than once, the same number of times, holds several
volumes. The index splits it, telling the volumes apart by the first of the temporal position,
the diffusion weighting, the echo time, the trigger time and the acquisition number that does
so, and lists each volume as a series of its own whose `VolumeSet` says which volume it is of
how many, and what sets it apart. Everything made for a series works on a volume unchanged;
the list shows one card for the set, and a viewport steps between its volumes. See
[adr/0005-volumes-of-a-series.md](adr/0005-volumes-of-a-series.md).

A picture in colour is decoded with a colour for each pixel beside its brightness. A viewport
draws it as a texture in its own colours; it has no window, and no 3D view.

## What is kept on this machine

In the folder the system gives Voxa: the saved views and annotations of each study,
`preferences.json` (the window's size, the layout, orientation, linking and how labels are
drawn), and `voxa.log`. The log is written by Voxa's own `tracing` subscriber, which takes
only Voxa's crates' events and only the fields named in `log::OWN_WORDS`, so that nothing a
file says can reach it. A picture of the window taken with `VOXA_CAPTURE` reads none of these
and writes none.

## State

All application state lives in one `AppState`. It changes only through one `Action` enum handled
by one pure function, `reduce`, which returns the `Effect`s to run (start indexing, request
frames, build a volume, work out the curve for smooth planes, read labels, read and write saved
views and annotations). Workers report back by sending further `Action`s. This makes every interaction testable
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
(baseline, extended and lossless) and as JPEG 2000 (lossless and lossy), and files whose data set
is deflated. Any other compression is
named on the series card when the scan is indexed, and decoding it returns
`UnsupportedTransferSyntax` with the same name.

A picture stored in colour, 8 bits a sample, as red, green and blue or as brightness and two
colour differences, is decoded to a colour for each pixel, with each pixel's brightness as its
value. Palette colour, deeper colour and the subsampled ways of storing colour are declined in
a sentence that names them.

The decoders for compressed images come from `dicom-pixeldata`. They are other people's code fed
with whatever a file holds, so each call is wrapped: a panic inside a decoder becomes a `Decode`
error for that one slice.

Label files are made the same way for tests, by the `testkit` feature of `voxa-labels`: NIfTI
files of every kind of number labels are stored as, in either byte order, placed either way a
NIfTI file can place its voxels.

Tests build their files in memory with the `testkit` feature of `voxa-dicom`. They cover
uncompressed pixels (unsigned, signed, 8-bit, `MONOCHROME1`, per-frame rescale in a multi-frame
file), pictures in colour, and RLE, for which the test writes the compressed frame by hand. A
JPEG picture is encoded in its test by `jpeg-encoder`, a development dependency. A JPEG 2000
picture is a small made-up file kept with the tests, written by `tools/fixtures/make_ramp_j2k.py`,
and is decoded exactly. A deflated file is deflated in its test. Lossless JPEG and lossy
JPEG 2000 have no test of their own.
