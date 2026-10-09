# Changelog

All notable changes to Voxa are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
