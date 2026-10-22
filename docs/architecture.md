# Architecture

Voxa is a Cargo workspace of small crates that talk only through their public interfaces.

| Crate | Owns | Depends on |
|---|---|---|
| `voxa-core` | Data model (`StudyInfo`, `SeriesInfo`, `Volume`), typed coordinate spaces (`Point<S>`, `Affine<A, B>`), `WindowLevel`, `VoxaError` | nothing in the workspace |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing into studies and series, geometry checks, pixel decoding, tag listing | `voxa-core` |
| `voxa-volume` | Assembling a series into a volume; sampling it, lines of sight through it, and reducing it; the planes of a volume through a point; distances, angles and the values in a region; the curve (cubic B-spline) through its values; histograms, percentiles, automatic window/level | `voxa-core` |
| `voxa-labels` | Reading a label volume (single-file NIfTI-1) and the table of names that goes with it, and writing both, with who made a label set; the parts of a label set with their volumes; which part lies at a point, and the first one a line of sight meets; a name's left and right | `voxa-core` |
| `voxa-models` | The networks: finding a model folder and checking every file against its list; running a network through ONNX Runtime, loaded from a folder, on the processor or a graphics card, behind the `Network` trait; the graphics cards, known by identity, and what the system lets a program use on each; which devices a run fits on and in what order to try them. No image mathematics. Its `cards.rs` is the one module outside `voxa-render` that uses `unsafe` | nothing in the workspace |
| `voxa-segment` | Everything SynthSeg does around its networks: preparing a volume, the steps between the networks and the mirrored run, tidying the answer into a label grid, the table of names with the model's verdict, and the notes a reader should know | `voxa-core`, `voxa-labels`, `voxa-models` |
| `voxa-render` | The GPU slice pipeline and the view transform; the GPU volume pipeline (a 3D texture and a shader that follows a line of sight through it for every pixel), its camera and scene; the GPU plane pipeline, which draws a flat plane through the same texture; labels on the device, drawn over the planes and as solids | `voxa-core`, `voxa-volume`, `voxa-labels` |
| `voxa-app` | The window, interface, application state, frame cache and background workers | all of the above |
| `xtask` | `cargo xtask check`, `cargo xtask coverage` and `cargo xtask check-models` | nothing in the workspace |

Dependencies point downward only: core, then dicom, volume, labels and models, then segment and
render, then app. `voxa-app` contains no image mathematics; anything that computes on pixels or
coordinates lives below it, where it can be tested without a window.

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
5. The list is written to `annotations.json` in Voxa's local folder, filed under a hash of
   the series, and can be saved as a table or as JSON. None of these holds anything that says
   whose scan it was.

Labelled parts come from files made elsewhere, or from a run that Voxa makes itself (the next
section), and are kept apart from the scan:

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

## Labelling a scan: the path of a run

Voxa labels a brain scan with SynthSeg 2.0's networks, run through ONNX Runtime. It ships no
network, and the zip a release offers carries ONNX Runtime but never DirectML or the weights;
see [models.md](models.md) for what a person provides and
[adr/0006-voxa-runs-a-labelling-model.md](adr/0006-voxa-runs-a-labelling-model.md) for why it is
built as it is. Nothing else in Voxa depends on any of it.

1. **Looking.** When Voxa starts, and when the person names another folder, a worker looks
   at whether this computer can label a scan: it reads the model folder and checks every
   network file against the folder's list (`voxa_models::read_model`), loads the runtime from
   its folder with telemetry switched off (`Runtime::load`), and lists the graphics cards. The
   answer is a `Ready` in the state: yes, or a sentence that says what is missing. The runtime
   is loaded once in a program's life, so a runtime folder changed later takes effect at the
   next start.
2. **The button.** **Label this scan** in the Parts tab sends `LabelScan`. The reducer starts
   a run only if the computer is ready, no run is on, no label file is being read and the
   scene holds a built volume. It picks the model (Automatic takes the robust one when any
   slice spacing is over 3 mm) and sends `Effect::LabelScan` with the series, what is wanted,
   where the person wants it to go, and how much of its card the window holds itself.
3. **Where it fits.** The worker takes the volume kept for the series and, once any run before
   it is over (one run goes at a time), works out how many voxels the networks would be given,
   whole and cut to the head (`voxa_segment::sizes`). It reads what the computer has just
   now (each card by its identity with what the system lets Voxa use on it, and the ordinary
   memory the system can still promise) and asks `voxa_models::rungs` for the places that fit,
   best first: the whole scan on each discrete card, then the processor, then the scan cut
   to the head on each, where cutting makes it smaller. A run that fits nowhere ends with a
   sentence that says what it needs and what the computer has. The run goes on a thread of
   its own, not on the workers' pool, and waits there for its turn; one that is stopped while
   it waits gives up at once and lets its volume go.
4. **The networks.** For each place in turn `voxa_segment::label` prepares the volume (resamples
   to 1 mm if need be, turns its axes to the order the networks were trained on, scales
   brightness between its 0.5th and 99.5th percentile, pads each side), finds the structures,
   and tidies them. Finding is SynthSeg's: the standard network twice, once mirrored, and the
   two answers averaged; or the three robust networks in a row, each given the class at every
   voxel that the one before found; then the cortex network if the areas are wanted; then the
   scores network, given the classes in a 224 cube. Each network is opened on the place's
   device when it is needed and let go before the next. The scores network may go on the
   processor when the card cannot hold it. A card that has no room, or that stops answering, gives
   way to the next place, and the result says so when the place that worked is the processor
   or another card. A card is found by its identity each time
   a network is opened, since the system's list changes its order while a program runs.
5. **The result.** The labels are on the 1 mm grid of the preparing step, turned back to the
   scan's own axes, with their own place in the patient: they are never resampled onto the
   scan, as [adr/0004-labels-own-grid.md](adr/0004-labels-own-grid.md) requires. A table names
   every part, from the model folder's lists, and carries the model's verdict from its scores
   (failed below 0.65), who made the labels (the model and the SHA-256 of its list, the
   device, the day) and the notes. The worker builds the label set and reports it with
   `LabelsRead`, as it reports a label file, so from here on the labels are drawn, listed
   and counted by the code described above. A part's volume is its voxels counted. The
   worker holds a run's set aside until the state takes the report in and asks for it
   (`TakeLabels`): only then is it the set drawn, so what is drawn is always what is listed,
   even when **Stop** is pressed between the report and the state's taking it.
6. **Keeping.** After the report, the worker writes a gzipped label file and its table, for
   every result so that **Save as** can offer them, and, once the state has taken the result
   in, keeps them under the local folder if the person has not switched keeping off. A kept
   result is found again when the same series is shown, and the Parts tab offers to show it,
   make it again or delete it.
7. **Stopping.** A run says what it says with its own number. It ends when the person presses
   **Stop**, when the scene moves to another series, when another export is opened, or when the
   store no longer holds the volume it began with; what it reports after that is dropped. A
   network on a card finishes what it was handed before it answers; on the processor it stops
   between two steps.

The runtime sits behind a trait, `Network`, so the arithmetic around the networks is tested with
stand-ins and no runtime, and another runtime could replace this one. The window draws with
one graphics device and ONNX Runtime uses its own; the window tells a run how much of its card
it holds, so that the run leaves room. If the window's device is lost, `eframe` cannot be given
another: `voxa-app/src/ui/lost.rs` watches for it from the start, the work in hand goes on, and
the person is told once, in the window and in a message box, to start Voxa again. Annotations,
saved views and kept results are written by workers and do not depend on the card.

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

In two folders the system gives Voxa. The settings folder (`config_dir`, on Windows in the
roaming profile, which a managed network may copy to its servers) holds only
`preferences.json` (the window's size, the interface's size, the shares of the window's width
the series list and the column take, the layout, orientation, linking and how labels are
drawn). The local folder (`data_local_dir`, which stays on the machine) holds the rest: the
saved views and annotations of each study, `choices.json` (what was chosen for labelling: the
model, the cortex areas, where a run goes by the card's name, whether results are kept, and
the two folders of the model and the runtime, which are its only paths), a `labels` folder
holding the result of labelling kept for each series, as a label file and a table named by a
hash of the series, and `voxa.log`. Voxa 0.5.0 and earlier kept all but the log in the
settings folder; as a newer Voxa starts, `moved.rs` moves what is there into the local folder,
each file whole and never over a different file of the same name (a result's two files go
together or not at all), and logs in a sentence what it moved and what it left, by Voxa's own
file names and a count of result files, whose names are hashes. The log is written by Voxa's
own `tracing` subscriber, which takes
only Voxa's crates' events and only the fields named in `log::OWN_WORDS`, so that nothing a
file says can reach it. Labelling adds no subscriber of its own: its notices are logged by
kind and key only, and the runtime's own words for a failed network, with every path taken
out, are logged as a warning by the worker. A picture of the window taken with
`VOXA_CAPTURE` reads none of these and writes none.

## State

All application state lives in one `AppState`. It changes only through one `Action` enum handled
by one pure function, `reduce`, which returns the `Effect`s to run (start indexing, request
frames, build a volume, work out the curve for smooth planes, read labels, look for the
labelling files, run the labelling networks, stop them, keep and find a result, read and write
saved views and annotations). Workers report back by sending further `Action`s. This makes every interaction testable
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
error for that one slice. A decoder makes room for a whole frame from what the header says, and
a request for memory that fails stops the program outright, which no wrapping catches. So
before any decoder is called, a file is refused in a sentence when it says one of its images
takes more than 256 MiB once decoded (larger than any MRI image: a slice of 1024 by 1024 values
of 16 bits takes 2 MiB), or when its pixels are stored uncompressed and it holds fewer bytes of
them than its header says (`decode/declared.rs`).

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

Labelling is tested in layers, none of which needs a runtime or a model folder. Each of
SynthSeg's preparing and tidying steps is held to arrays that SynthSeg's own Python made from
small made-up volumes (`tools/golden` makes them; they are kept under
`crates/voxa-segment/tests/golden`). The steps between the networks, the mirrored run and the
224 cube are run with stand-in networks (the `testkit` feature of `voxa-models`), the model
folder check with folders made in the test, the sums that say where a run fits with made-up
cards, and the reducer and the Parts tab by actions and state with no window. A label file is
written and read back. So `cargo xtask check` needs neither on the machine. It is meant to
pass on Windows and Linux; for 0.5.0 it has been run on Windows only, and has not been built
or run on Linux. The tests that run the real networks on the converter's made-up head are marked
ignored and run by `cargo xtask check-models`; `runtime.rs` and `cards.rs`, which cannot run
without a runtime or a card, are left out of the coverage count.
