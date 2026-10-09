# Changelog

All notable changes to Voxa are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
