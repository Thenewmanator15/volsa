# Changelog

All notable changes to Voxa are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

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
