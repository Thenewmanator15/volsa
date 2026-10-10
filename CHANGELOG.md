# Changelog

All notable changes to Voxa are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- The curve that Smooth draws the planes from is worked out by the graphics device itself,
  from the volume it already holds, in compute shaders that run on any Vulkan, DirectX 12 or
  Metal device. Nothing is worked out on the processor and nothing is handed across, so
  Smooth takes effect at once. For 512 x 512 voxels in 300 slices: 1.15 s on one processor
  thread before; 0.34 s on integrated graphics and 23 ms on a discrete card now. For 64
  slices: 210 ms before; 44 ms and 2.3 ms now. On a device with no compute shaders, or with
  no room to work the curve out, a worker does it as before.
- The largest value in each block of a volume, which lets the 3D scene step over empty
  space, is worked out by the graphics device from the values it has just been given. For
  512 x 512 x 300 the worker's part of making a volume ready drops from 142 ms to 35 ms; the
  device adds 45 ms of its own on integrated graphics and 2.5 ms on a discrete card, away
  from the interface's thread. The values themselves are still turned into 16-bit numbers on
  the processor: doing that on the device would mean handing it twice the bytes from the
  interface's thread, which costs more than it saves.
- The renderer with no window, which the tests, the benchmarks and the check programs use,
  takes its backend and its adapter from `WGPU_BACKEND` and `WGPU_POWER_PREF` when they are
  set. The work above was tested on Vulkan and on DirectX 12, on both graphics devices of
  the machine it was written on, and for the first time timed on the discrete one.

- While the 3D scene moves it is drawn as finely as the graphics device keeps up with, not
  always at half size. The device is timed on the pictures it draws in full, and the moving
  picture is drawn in full, at three quarters or at a half, whichever is the finest expected
  to take no more than 8 ms. On a discrete card the moving picture is now the still one:
  measured in the window, it keeps all of the still picture's detail, where the half-size
  picture kept about two thirds of it in MIP. A device that is slow, or has not been timed
  yet, draws the moving scene at a half as before. A sharper way of enlarging the half-size
  picture was tried first and left out: it made the picture look crisper without bringing it
  any closer to the full one.

### Fixed

- Two tests of the worker that reads label files waited for a quiet moment where they
  should have waited for what they were about, and failed when the machine was slow.

## [0.4.0] - 2026-10-10

The fourth milestone: labelled parts, from files. Voxa shows labels that another program made
for a scan; it does not make them, and it runs no labelling model. Voxa is not a medical device
and is not for diagnostic use.

### Added

- Opening a file of labels on the series shown in 3D: from the Parts tab ("Open labels…"), by
  dropping the file on the window, or with `--labels FILE` on the command line. A label file is
  a single-file NIfTI-1 volume (`.nii` or `.nii.gz`) of whole numbers, one number for each part.
  It is read on a worker thread, and a file that is not that is refused with a sentence that
  says why. Labels are put on the 3D view of a series: a label file opened before there is one
  is not read, and a notice says to open the scan first.
- Names for the parts, from a table beside the label file: the table a labelling pipeline
  writes (names with a group, a colour, a number for each side, how each was made, and the
  model's own check), a plain list of numbers and names, or a 3D Slicer colour table (`.ctbl`).
  Another table can be chosen with "Names…". With no table the parts are called by their
  numbers, and a number the table does not name is listed as not named, never left out.
- A Parts tab beside the slices and the planes: the parts by group, a name with a left and a
  right on one row, each with its colour, its volume and a switch for each side. A search box,
  a switch for a whole group, "Show all", and for the part selected a slider for how solid it
  is drawn and "Show only this".
- The list says what the labels are and how far to trust them: that another program made them,
  where their names came from, which parts their table marks as approximate and why, and, when
  the table carries the labelling model's own check, whether it passed. A failed check is said
  in the warning colour above the list.
- Labels over the plane views: an outline round each part, or the outline and a tint of the
  part's colour, or nothing. The pointer resting on a part is told its name and side, a click
  selects it, and the part selected is outlined in white. Each panel that draws labels says
  "Labels: made elsewhere".
- A fourth look in the 3D scene, Parts: the labelled parts as lit solids, each in its own
  colour, drawn from the label volume itself. Parts can be hidden or faded to see what is
  behind them, cuts open the solids as they open the scan, the pointer is told the name of the
  part it rests on, a click selects it, and the ruler and marker measure on the solids. The
  scene says that the solids are labels made elsewhere.
- Left against right: a selected part that has two sides says the volume of each and how much
  the left differs from the right, as a percentage of their mean, and that Voxa cannot tell how
  much of such a difference is noise in the labelling.
- "Save as CSV…": every part, hidden or not, with its group, name, side, volume in millilitres,
  count of voxels and how it was made. The table holds nothing of the scan, and a name that a
  spreadsheet would run as a formula is written as text.
- Labels keep their own grid. They are never resampled onto the scan: what is drawn and what is
  counted are the labelling program's own voxels, and a part's volume is its voxels times the
  size of one. See [docs/adr/0004-labels-own-grid.md](docs/adr/0004-labels-own-grid.md).
- The made-up study that `write_phantom` writes now comes with made-up labels of its ball, in a
  folder `labels` beside the series: a shell in two halves, a core and a bead, on a grid of
  their own, with one part marked approximate and a check that fails.
- `cargo run -p voxa-app --example check_labels` checks a label file against a whole export
  without a window and prints nothing from either.
- A new crate, `voxa-labels`, reads label volumes and their tables and answers which part lies
  at a point. `cargo xtask coverage` holds it to the same 80% as the other crates below the
  window.

### Changed

- The column beside the large part of the window is at least 260 points wide where the window
  allows, so that its three tabs fit. In the smallest window the large part is 40 points
  narrower than before.
- The file that writes the made-up study moved from `examples/write_phantom.rs` to
  `examples/write_phantom/`; it is run as before.

### Known limits

- Voxa does not make labels. It runs no labelling model, and none comes with it.
- One set of labels at a time. Two files, such as the two inner ears, are opened one after the
  other, not together.
- A label file does not say which scan it was made from. Labels that lie nowhere near the
  series are refused; labels made from another scan of the same head, in the same place in the
  scanner, are not told apart from the right ones.
- Labels are bound to the study they were opened on and are forgotten when Voxa closes. They
  are not drawn on the 2D stack viewports.
- Choosing the Parts look does not move the camera. Labels that reach beyond the series shown,
  such as a whole head over a thin slab, run off the picture until it is zoomed out; in the
  plane views they are drawn only where the series is.
- The solids are the label volume's voxels, lit: on a smooth shape they show faint bands, like
  contour lines, where the voxels step. The slice shown in 2D is not drawn among them.
- A selected part seen flat-on through a cut is not marked: the pale rim that marks it follows
  its surface, and a cut face has none.
- A label file named with `--labels` waits for a series with a 3D view to be shown. While the
  series on show has none, the labels are not opened and nothing says that they are waiting.
- A part that is faded but still shown takes the pointer: resting on, or clicking, a part seen
  through it names and selects the faded one in front. Hide the part in front instead.
- No FreeSurfer, NRRD or DICOM SEG files, no meshes, no correcting labels by hand, and no
  comparing the labels of two studies.
- Opening the made-up study's folder reports two files skipped: its labels and their table,
  which are not part of the scan.

## [0.3.0] - 2026-10-10

The third milestone: plane views and measurement, on top of a pass for speed. Voxa is not a
medical device and is not for diagnostic use.

### Added

- Three plane views of the series in the 3D scene: axial, coronal and sagittal, through one
  point, the position. They are in the column beside the scene under a "Planes" tab, or, with
  Enlarge, share the large part of the window with the scene, two by two. Each is drawn on the
  graphics device from the scene's own volume, so it costs no second copy and moving through
  its layers decodes nothing.
- The planes follow the scan's own axes and are named by the anatomical plane each is nearest,
  so the scan's own slices are shown exactly as taken and a tilted scan gives tilted planes,
  lettered truly. Axial is seen from the feet and coronal from the front, as radiology shows
  them. Colours are those of 3D Slicer: axial red, sagittal yellow, coronal green.
- In a plane view: a click or drag moves the position, the wheel moves through layers, Ctrl
  with the wheel zooms about the pointer, the middle button moves the picture, the right
  button changes the window, a double-click fits. Each panel has a letter at each edge, a
  scale bar, the lines of the other two planes, and a note where its picture runs across
  slices far apart and is interpolated between them.
- In the 3D scene the three planes are outlined in their colours, with the position where they
  cross; a click on a surface moves the position there, and each plane has a grip to drag it.
- Measuring, in the plane views and in 3D: a ruler, an angle, the scan values inside an
  ellipse (mean, spread, least, greatest, area), and named markers. Everything is worked out
  from millimetres in the patient and the scan's own values, never from the screen, and a
  ruler says how exactly it can be known: one voxel along its own direction.
- A Measure window (the M key): the tools, undo and redo (Ctrl+Z, Ctrl+Y), the list of what
  has been measured with a name for each that can be typed over, and saving the list as a
  table (CSV) or as JSON. Esc puts the tool in hand down.
- Measurements are kept between sessions, in a small file in the settings folder, filed under
  a hash of the series. That file and the saved lists hold coordinates, numbers and the names
  typed, and nothing that says whose scan it was.
- Link, on from the start: 2D viewports showing a series of the same study go to the slice
  nearest the position, and scrolling one takes the position with it. The position is marked
  on each by a small cross. Linked views take it that the patient did not move between series.
- Smooth, off from the start: the planes drawn from a curve through the scan's values (a
  cubic B-spline) in place of a blend of the nearest. The curve passes through every measured
  value; it shows no more detail, and beside a sharp edge it overshoots a little.
- `cargo run -p voxa-app --example check_planes` checks the plane views of a whole export
  without a window and prints nothing from its files.

### Changed

- A 3D scene that the person has turned or zoomed keeps what it shows when its part of the
  window changes size. Before, it kept its scale, so a scene moved into a small part of the
  window showed only a corner of itself.
- In a narrow 3D scene the captions go beneath the letter at the top, across the whole width.

- Showing a large series in 3D no longer holds the window up while the volume is made ready
  for the graphics device: that is now done on a worker thread, in parallel, and the window
  only hands the result over. For 512 x 512 voxels in 300 slices on integrated graphics the
  window's part went from about 730 ms to about 65 ms.
- MIP draws faster with the same picture: a line of sight stops once it is as white as the
  window shows, and the empty blocks at its ends are stepped over. 512 x 512 x 64 in a
  1280 x 800 picture on integrated graphics: 85 ms to 51 ms still, 15.6 ms to 9.9 ms moving.

- A series with no 3D view has its slices in the large part of the window, with the reason in
  the column beside them. Before, the large part stayed black with the reason written in it,
  while the pictures were small at the side. Each reason is now said once, and what is done
  about it ("It is shown as a plain stack.") once after them.
- VR leaves out what is fainter than a tenth of the window, as Surface does. A scan's air is
  never quite empty, and in VR its faint noise dimmed everything seen through it: in a test,
  40 mm of haze at 8% of the window took about an eighth of the light from what lay
  behind. It also
  kept every block of the air from being stepped over. Pictures are a little clearer and draw
  faster, by how much depending on how noisy the air is; tissue fainter than a tenth of the
  window is no longer drawn in VR. From there tissue becomes more solid up to the top of the
  window, as before.
- VR and MIP follow each line of sight in steps of half a voxel along whichever axis of the
  volume the line crosses fastest. Before, a step was half the least distance between voxels
  from every side, so a view along slices that are far apart took many more steps than there
  were voxels to see. A made-up head of 512 x 512 voxels of 0.5 mm in 64 slices 3 mm apart,
  seen from above in a 1280 x 800 picture on integrated graphics: VR 33 ms to 14 ms, MIP 30 ms
  to 13 ms. Seen at a slant about a tenth is saved, and across the slices nothing. The
  surface look keeps its short step and draws exactly as before.
- A series opened with the 3D scene in view is decoded once. Before, its slices could be
  decoded twice, once for the 2D viewport and once for the volume. They are now decoded for
  the volume, and the viewport reads them out of it; only the slice on show is also decoded
  straight away for the viewport, so that it is seen at once.
- A series shown in 3D is held in memory once, as its volume, and no longer a second time as
  separate slices. When another series takes the 3D scene, the slices of the one before are
  kept for its viewport, as far as the slice cache has room, so going back to it decodes
  nothing again. Slices decoded for a volume that fails, or is overtaken by another, are kept
  the same way and not thrown away.
- A turntable's pictures are put together into its GIF on a worker thread, so the window
  stays alive while that is done (about a third of a second for 72 pictures of 512 x 320).
  It says "Saving ..." meanwhile and "Saved ..." when the file is written.

### Fixed

- A debug build stopped the moment it decoded a JPEG 2000 picture: the decoder frees a null
  pointer as it tidies up, and a build with debug checks stops the program for that. The
  checks are off for that one crate. Release builds were not affected.
- The message shown when the reader stops part-way through a series being built for 3D had a
  long run of spaces in the middle of it.
- Putting a second large volume on the graphics device could find it with no room, and the
  volume was then shown at reduced resolution without need. The first volume's memory is now
  given back before the second is put there.
- Running out of graphics memory while a volume was being handed over would have stopped the
  program. It is now watched for, and the volume is halved and tried again.
- If the graphics device has no room for a volume at all, the scene now says that it cannot
  be drawn. Before, it would have gone on showing the volume of the series before.

### Known limits

- The planes follow the scan's own axes: a plane cannot be turned by hand, and there are no
  slabs or curved planes.
- An annotation cannot be moved or reshaped once made; it is removed and drawn again.
  Measuring is in the plane views and the 3D scene, not in the 2D viewports.
- Nothing moves the words beside annotations apart: several close together in a small panel
  run into one another.
- Linking is within one study. Series of different studies are not brought into register.
- A region's numbers are worked out once, when it is drawn, from the scan's own values.
- Drawing with the mouse, the grips in 3D, the keys and the save dialogs have been tested
  without a window but not tried by hand; see `docs/acceptance-0.3.0.md`.

## [0.2.0] - 2026-10-09

The second milestone: the series being looked at is also shown in 3D. Voxa is not a medical
device and is not for diagnostic use.

### Added

- A 3D scene in the large part of the window, showing the series of the active viewport, with
  the 2D viewports in a column beside it. A button on the column swaps them, and Escape gives
  the 3D view the large part again.
- Volumes built in the background from every slice of a series and placed in the patient by the
  series' own geometry, so thick and tilted slices are drawn in their true proportions. A
  series that cannot be a volume (locating images, a single slice, uneven spacing) says why in
  plain words, and its slices are still shown.
- Three looks, drawn by following a line of sight through the volume for every pixel on the
  graphics device: Surface (a solid, lit surface), VR (see-through, brighter tissue more solid)
  and MIP (the brightest value on each line of sight). All follow the window of the active
  viewport, so contrast is changed the same way in 2D and 3D.
- Turning the scene by dragging, moving it with Shift or the middle button, zooming about the
  pointer with the wheel, six named views (A, P, L, R, S, I), fitting with `F`, and a
  double-click on a surface to put that point in the middle. The picture is drawn coarsely
  while the camera moves and in full once it is still.
- Up to three cut planes. A cut face shows the scan there, as a slice would. Each cut has a
  handle to move it and a knob to turn it.
- The slice shown in 2D drawn inside the scene, with its outline and a handle that moves
  through the slices.
- Over the scene: a letter at each edge, a scale bar that is true across the whole picture (the
  camera has no perspective), and captions that say what the picture cannot show: slices far
  apart being interpolated between, and a volume reduced to fit the graphics device.
- A volume larger than the graphics device can hold is halved until it fits, and the scene
  says so.
- Saved views: the camera, look, window and cuts under a name, kept between sessions in a
  small JSON file in the settings folder, filed under a hash of the series so that the file
  says nothing of the scan.
- Export: the scene as a PNG picture, or as an animated GIF that goes once round the patient.
  The file holds the scene and nothing else, and the name offered is `voxa-3d`.
- `cargo run -p voxa-app --example check_volumes` checks a whole export in 3D without a window
  and prints nothing from its files. Benchmarks of the renderer: `cargo bench -p voxa-render`.

### Changed

- Text in the corners of a 2D viewport is cut short, with an ellipsis, where it would run into
  the letter at the middle of the edge.
- Opening a series now also builds its volume, which takes memory: four bytes for every voxel,
  beside the decoded slices.

### Known limits

- The scene shows one series at a time, without perspective, and has no measurements, labels
  or plane views; those are later milestones.
- A large volume is put on the graphics device from the interface's thread, so the window
  pauses for a moment when a large series is first shown in 3D.
- In the Surface look a hard edge in the data shows a fine grain.
- Dragging, the handles, the menus and the save dialogs have been tested without a window but
  not tried by hand; see `docs/acceptance-0.2.0.md`.

## [0.1.0] - 2026-10-09

The first milestone: a viewer that opens an MRI export and shows any series slice by slice.
Voxa is not a medical device and is not for diagnostic use.

### Added

- Workspace of five crates and an `xtask` package, with every quality gate running through
  `cargo xtask check`: formatting, lints, a 400-line limit on source files, tests, licence and
  advisory checks, and the documentation build. `cargo xtask coverage` holds the three crates
  that need no graphics device to 80% line coverage.
- Opening a folder, a zip file (read in place, nothing extracted), a `DICOMDIR` or a single
  file: by a path on the command line, by dropping it on the window, or through the Folder and
  File buttons.
- Indexing from headers alone into studies by date and series by number, with slices ordered
  along the way they face. Classic files and files holding a whole series are both read. A
  series is marked when its slices are unevenly spaced, share positions, differ in size, lie in
  different planes or lack position data, or when its compression cannot be read.
- Decoding of uncompressed, RLE, JPEG and JPEG 2000 images to values with the rescale of each
  frame applied. A damaged slice shows a message in its place and leaves the rest alone.
- A study browser with a thumbnail, description, slice count and spacing for each series. The
  most detailed series without problems opens by itself.
- One, two or four viewports. In each: scroll, zoom about the pointer, pan, mirror, quarter
  turns and fit, by mouse and by keys, with a list of both under `?`.
- Window and level by right drag, by typing, and by three presets taken from percentiles of the
  slice; an automatic window chosen once for a series. Contrast is applied on the graphics
  device, so changing it neither decodes nor uploads anything again.
- Over each slice: a letter at every edge from the slice's own geometry, a scale bar in round
  millimetres, and the study, series, slice number, slice location and window in the corners.
  Radiological display by default, with a labelled switch to neurological.
- A tag browser for the file behind the slice shown, with search.
- Playback through the slices at 1 to 60 a second, looping or not.
- Privacy: no network connection is made and nothing is logged from a file. The patient's name
  and identifier are hidden in the viewports and the tag browser until asked for.
- For development: a made-up study written by `cargo run -p voxa-app --example write_phantom`,
  a checker for real exports that prints nothing about the patient
  (`--example check_export`), benchmarks (`cargo bench -p voxa-app`), and `VOXA_CAPTURE` to
  save a picture of the window.

### Known limits

- Colour images, three-plane views, measurements and 3D are for later milestones.
- A file holding many frames is read again for each request for its frames.
- The toolbar wraps onto a second line in a window narrower than about 1440 points.
- On Windows a console window opens beside Voxa when it is started from Explorer.
