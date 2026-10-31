# Changelog

All notable changes to Voxa are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Plugins: models a person adds themselves. **Add a plugin…**, under **Added by you** in the
  Parts tab, takes a folder holding a `plugin.json` and an ONNX network someone else made.
  Voxa checks the folder, shows a card that says first "Added by you, not part of Voxa. Voxa
  has not checked what this model does. Not for diagnostic use.", then what the plugin says of
  itself and every setting Voxa will prepare a scan with, and copies it only on **Add**. It
  then measures what the network needs on the processor and on each graphics card with memory
  of its own. **Run** prepares the scan in the open from a fixed menu (the grid along the
  scan's own axes at the plugin's spacing, one centred box or blocks, percentile, z-score or
  fixed scaling), runs the network, and opens its labels with the banner first in the list,
  the plane and 3D captions, the kept files, a saved table and the parts exported as CSV. A
  scan of a modality the plugin does not list is refused; a contrast the scan's description
  names that the plugin does not list is warned of, run only after **Run anyway**, and stamped
  on the result; a description that names none is stamped "Contrast not confirmed". Label maps
  run now; one number, a value map and an alignment are part of the format and refused as not
  yet run. Voxa downloads no plugin, recommends none and links to none.
  [docs/plugins.md](docs/plugins.md) is for people who make one, and
  [record 0007](docs/adr/0007-plugins.md) says why it is built as it is.

### Security

- A plugin's network runs only in a helper process, `voxa.exe --plugin-helper`, held in a
  Windows job object that ends it with Voxa or on **Stop**, lets it start no process and caps
  its memory at what it was measured to need; if it fails or ends early, Voxa says so and goes
  on. Before ONNX Runtime sees a network file, Voxa reads its structure itself and refuses any
  operator outside standard ONNX, local functions, data outside the plugin's listed files, and
  files past the reader's limits. Every listed file is checked by name, size and SHA-256 when
  added, and the network's sum again by the helper before every run. Scan values reach the
  helper through pipes only. SECURITY.md has a "Plugins" section; the helper is not a sandbox,
  so add plugins only from sources you trust.

## [0.6.0] - 2026-10-22

The sixth release, made for a first public release: a download for Windows, the notices and
credits that go with it, a window that fits any screen, and smaller fixes found by an audit of
what a public release needs. Voxa is not a medical device and is not for diagnostic use.

### Added

- A download for Windows. `voxa-<version>-windows-x64.zip` holds one folder with `voxa.exe`, a
  `README.txt` for someone who has just downloaded it, `INTENDED-USE.txt`, `MODELS.md`, this
  changelog, the two licences, `NOTICE`, `THIRD-PARTY-NOTICES.txt`, and ONNX Runtime's two
  libraries with their licence and notices in a `runtime` folder. `cargo xtask dist [--runtime
  FOLDER [--local]]` makes it, and refuses a runtime folder without ONNX Runtime's licence and
  notices unless `--local` asks for a package that is not for release, named so. The program in
  it records no path of the computer it was built on, and carries an application manifest: the
  system's message windows have today's look, and it follows each screen's scale from the
  start. A tag that starts with `v` has GitHub run every check, build the zip with
  ONNX Runtime 1.24.4 from Microsoft's package (taken only if its SHA-512 matches the one
  nuget.org published, pinned in the workflow), write the zip's SHA-256 beside it and make a draft release, with that
  version's section of this changelog as its notes.
- `voxa.exe` needs no Visual C++ redistributable of its own: Microsoft's C runtime is built into
  it, and `cargo xtask dist` reads the program to make sure. It carries its icon and the details
  Explorer lists, among them that Voxa is not a medical device and is not for diagnostic use.
- `docs/intended-use.md` says what Voxa is for and what it is not for, and that the labelling
  model's weights come under FreeSurfer's licence. The README links it beside the disclaimer,
  and the About window shows its first paragraph.
- `THIRD-PARTY-NOTICES.txt` gives the licence and notice texts of every package built into Voxa
  and of the four typefaces in the interface. Where a package offers a choice of licences it
  says which one Voxa takes, and packages that ship no licence file carry the copyright lines
  from their repositories. `cargo xtask notices` writes it, and `cargo xtask check` fails when it
  is out of date. `NOTICE` credits SynthSeg (Copyright 2020 Benjamin Billot, Apache License
  2.0), whose code Voxa's labelling is ported from, and each ported file says so at its top.
  The About window names all four typefaces, credits SynthSeg, and has a **Licences and
  notices…** button that shows both files.
- Voxa opens at a size that fits the screen it is on: the size it was left at if that fits,
  and otherwise most of the screen, centred. Its window can be made as small as 640 by 480 at
  the display's scale.
- The list of series and the column beside the large part each take a share of the window's
  width, can be dragged wider or narrower, and keep what was dragged from one run to the next.
  On a large screen the column grows with the window instead of staying a narrow strip.
- **B**, or **Series** in the toolbar, shows or hides the list of series, which starts hidden in
  a narrow window. **F11** enters and leaves full screen. **Size** in the toolbar draws the
  interface larger or smaller, as Ctrl+= and Ctrl+- do (Ctrl+0 for its usual size), and the
  size is remembered.
- Keys to move through the slices of the active viewport, as the wheel does: Up and Down one
  slice, Page Up and Page Down ten, Home and End the first and the last. No key acts while you
  are typing in a field.
- The measurements table and the table of parts (CSV) carry on every line a `notice` column
  with the disclaimer. The table of parts also says which model made the labels (`model`) and
  how the model's own check of its work went (`model_check`).
- `SECURITY.md` says how to report a vulnerability privately, and never with a real scan. Forms
  for reporting a fault or asking for something open with a warning never to attach a scan, an
  export, a screenshot showing patient details or tag values; pull requests have a template.
- For pictures of the window, `VOXA_CAPTURE_ZOOM` draws the interface at another size, so one
  display can show what a larger screen would, and `VOXA_CAPTURE_SHOW` gains `nolist` and
  `fitted`.

### Changed

- Exports open much faster. An ordinary zip is opened where its end record says its directory
  is, instead of after a search of the whole file; indexing reads of each file only what its
  header needs, never the pixels, and finds exactly what it found before; and the workers read
  a zip or a folder at the same time, each through a reader of its own. On a made-up export of
  1,500 slices of 512 x 512, from starting Voxa to its slice and 3D scene on the screen takes
  1.3 s instead of 4.5 s for a compressed zip (1.8 s instead of 8.4 s when the file is not in
  the system's cache), and 1.1 s instead of 1.6 s for a folder; indexing the compressed zip
  takes 0.27 s instead of 3.06 s. A zip with data before it, or a second end record near its
  end, is searched as before, and a DICOMDIR is passed over without being read. A zip whose
  file is damaged after a file's header no longer stops indexing; the damage is reported when
  that slice is read.
- Labelling no longer chooses a graphics card that other programs have nearly filled. Before a
  run Voxa also counts how much of the card's memory every program holds (Windows' own count,
  the one Task Manager shows), not only the system's budget for Voxa, which does not fall as
  other programs fill the card. Such a run goes to the processor instead of spilling into
  ordinary memory, and the Parts tab says how much of the card was free. A card chosen by hand
  that other programs have filled is refused with what it actually has free.
- The program is now `voxa.exe` (it was `voxa-app.exe`).
- What comes of your scans is kept on this computer only. Saved views, annotations, the
  choices for labelling and kept labelling results move from `%APPDATA%\Voxa\config`, in the
  roaming profile that a managed network may copy to its servers, to `%LOCALAPPDATA%\Voxa\data`,
  beside the log; only `preferences.json` stays. On the first start Voxa moves what an earlier
  version left there, and says in the log what it moved.
- With **Patient details** off, the tag browser also hides, and cannot find by searching, the
  accession number, the institution's name and address, the physicians and operators, the
  station's name, the device's serial number, the order and visit numbers and every private
  tag, and says how many values it has hidden.
- When something keeps Voxa from labelling a scan, the message points to `MODELS.md` beside the
  program. When ONNX Runtime will not load because a library it needs is missing or too old,
  Voxa says it needs Microsoft's free Visual C++ 2015-2022 x64 redistributable.
- `docs/models.md` says that a download comes with ONNX Runtime and a build from source fetches
  it by hand, that ONNX Runtime needs the Visual C++ redistributable, that Windows' own DirectML
  is used (in Windows 10 from version 1903 and in Windows 11), and what FreeSurfer's licence
  says of the model's weights.
- The README is for someone who downloads Voxa: how to download, start and remove it, the
  SmartScreen warning, what it needs, what it keeps on your computer, and how to get help.
  CONTRIBUTING.md is for someone new to Voxa. The design records are kept as a record of their
  time, say so, and no longer speak of the developer's own scans or of tools outside the
  repository.
- The window is no longer opened at a fixed 1440 by 900 points, and a remembered size that does
  not fit the screen is not taken.
- `cargo xtask check` runs every test program even after one fails, so one failure no longer
  hides another; the gate still fails if any test did. CI's Windows build keeps the zip that
  `cargo xtask dist` makes.
- `zip` 9 in place of 8.6, which `cargo deny` refused for RUSTSEC-2026-0367, -0368 and -0369,
  all about extracting an archive to disk; Voxa reads a ZIP export in place and extracts
  nothing. `deny.toml` passes the notices that `zvariant`, `zbus_names` and `zvariant_derive`
  are no longer maintained (RUSTSEC-2026-0370 to -0372), which come only through accessibility
  on Linux and have no safe upgrade yet, giving the reason for each.

### Fixed

- On Windows' software graphics device, which a computer with no graphics card draws with, the
  Parts look drew nothing when a line of sight took every step instead of stepping over the
  empty blocks, which is what the tests do; GitHub's Windows checks failed on it. It now draws
  the same picture either way.
- The About window no longer calls every library's licence permissive; one is under the Mozilla
  Public License 2.0.
- The list of keys, the About window and the licences and notices window scroll when Voxa's
  window is too short to hold them, and always show their scroll bars, so a list cut short no
  longer looks whole. The About window's **Licences and notices…** button stays in sight.
- The disclaimer stays whole and uncovered at every size of the interface. The status line is
  laid out first; when its line cannot hold the disclaimer, the newest notice and **Dismiss**,
  the notice and the button go onto a line of their own. In a small window with the interface
  enlarged, the bar across the top takes no more than about 40% of the window's height, and
  what it holds beyond that scrolls.
- When Voxa stops unexpectedly, for example because another program holds the graphics card's
  memory, it says so in a message window with the path of its log, instead of closing without
  a word.
- In a narrow window the bars put whole buttons on the next line, a long notice in the status
  line is cut short instead of covering the disclaimer, and a long word on a series card ends
  in an ellipsis.

### Security

- A small DICOM file that claimed a huge image could make a decoder ask for many gigabytes and
  stop Voxa. Voxa now refuses, with a sentence and before decoding, a file whose images would
  each take more than 256 MiB once decoded, far more than any MRI image, and a file whose
  uncompressed pixels are fewer bytes than its header says.

## [0.5.0] - 2026-10-22

The fifth release, which finishes the fourth milestone: Voxa labels a brain scan itself, with
SynthSeg 2.0's networks run through ONNX Runtime on this computer. It also reads more kinds of
MRI export. Voxa is not a medical device and is not for diagnostic use. A label made by a model
is a guess that looks certain, and every result says so.

### Added

- Labelling a brain scan inside Voxa. In the Parts tab, for a scan with a 3D view and no
  labels open, **Label this scan** labels about thirty brain structures on a scan of any
  contrast and, if asked, divides the cortex into 34 areas a side. No Python runs. The result
  becomes the study's labels, as a label file does: the list, the outlines over the planes,
  the Parts look, volumes, left against right. While a run is on, the Parts tab names its
  stage ("Preparing the scan", "Finding the structures, 2 of 3", "Dividing the cortex", "The
  model is checking its work", "Tidying up") beside **Stop**, and the rest of Voxa keeps
  working. Only one run goes at a time, and a run is stopped, and its result dropped, when the
  scene moves to another series or another export is opened.
- **Options**, beside the button: the model (Automatic picks Robust when the slices are more
  than 3 mm apart and Standard otherwise, and says which it picked and why), whether the
  cortex is divided into its areas, where a run goes (Automatic, a graphics card by its name,
  or the processor), whether results are kept, and the folders of the models and of the
  runtime. What was chosen is remembered between runs.
- Every result carries its check and its history. The model's own check of its work comes
  first, in the warning colour when it failed (any group of structures scoring below 0.65,
  which are named). The caption says Voxa made the labels, with which model, on which device,
  on what day, and "Not for diagnostic use." A Notes line says when the slices are more than
  3 mm apart (a rough map), when the run fell to the processor because a card had no room or
  stopped answering, when the model checked its own work on the processor, and when the scan
  was cut to the head. A change of plan is never silent. See
  [docs/models.md](docs/models.md).
- Where a run goes. Before a run Voxa works out what the scan needs and goes only where the
  system can still promise it: the whole scan on a graphics card with memory of its own, then
  on the processor, then the scan cut to the head on each. Integrated graphics are used only
  when chosen. A run that fits nowhere says what it needs and what the computer has. Every run,
  on any device, is held to an upper bound on the ordinary memory it asks the system for;
  the figures are in the decision record. On Windows, graphics cards run the networks
  through DirectML; elsewhere the processor does.
- A result is kept on this computer for its series, unless keeping is switched off, and is
  offered again when the series is opened: **Show**, **Make again** and **Delete**. **Save
  as…** writes the label file and its table of names wherever the person chooses, for other
  programs, with SynthSeg's own numbers for the structures. A kept result is a label file
  and a table filed under a hash of the series, and says nothing of whose scan it was.
- The same answer as SynthSeg's own program, shown by tests. On a made-up head the standard
  model on the processor gives SynthSeg's label at every voxel, and every other pairing of
  model and device gives at least 99.99% of voxels the same, a Dice of at least 0.999 for
  every structure, and scores within 0.001. Preparing a scan and tidying the answer are
  SynthSeg's steps, held to arrays that SynthSeg's own Python made from small made-up volumes.
- Voxa ships no model and no runtime and fetches none. A person provides the runtime (three
  files from Microsoft's ONNX Runtime with DirectML, in a `runtime` folder beside the
  executable or one named under **Options**) and a model folder made once, outside Voxa, with
  the converter in `tools/synthseg-to-onnx`, from SynthSeg's own files obtained under their
  terms. Voxa checks every file of a model folder against its list before it trusts it.
  Telemetry in the runtime is switched off before anything else is asked of it. See
  [docs/models.md](docs/models.md) and
  [docs/adr/0006-voxa-runs-a-labelling-model.md](docs/adr/0006-voxa-runs-a-labelling-model.md).
- Two new crates, `voxa-models` (the networks, the runtime, the graphics cards, where a run
  fits) and `voxa-segment` (SynthSeg's steps around its networks), and a writer in
  `voxa-labels` for the label file and table it already read. `cargo xtask coverage` holds
  both new crates to 80% of lines, leaving out the two files that need the runtime or a
  graphics card. `cargo xtask check-models` runs, with the real networks, the tests that
  `cargo xtask check` leaves out; without a model folder and a runtime it does nothing.
  `cargo xtask check` needs neither on the machine, and passes but for six advisories that
  `cargo deny` reports against dependencies `main` already has (see Known limits).
- `write_phantom head CHECK_FOLDER OUT [whole]` writes the converter's made-up head as a
  study, so there is something with a brain's shape to label that is nobody's, and
  `check_labelling EXPORT` labels the largest series that has a 3D view with no window, on
  every place Voxa would try, and prints for each the size of the grid, the place, the
  seconds, the number of parts and whether the model's own check passed.
- The series list says what a series is when it is not a volume: locating images, a single
  image, pictures in colour, a stack that cannot be shown in 3D with the first reason why, or
  images compressed in a way Voxa cannot read, with the way named. A card shows the plane,
  whether the scan was acquired in 2D or 3D and the family of sequence, when the files say.
  Resting the pointer on a card shows everything known, with the repetition, echo and inversion
  times, the flip angle and the field strength. A plane more than 15 degrees from the nearest
  is called oblique. Images in several planes are said to be in none. No weighting is guessed.
- A series that holds several volumes, such as two echoes, diffusion weightings, time points,
  trigger times or repeated acquisitions, is listed once, as "2 volumes". A viewport shows one
  volume at a time, says which under the series' name, and steps to the volume before or after
  with the buttons either side of it in the toolbar, or `[` and `]`, keeping the slice, the
  view and the 3D camera. See [docs/adr/0005-volumes-of-a-series.md](docs/adr/0005-volumes-of-a-series.md).
- Pictures stored in colour, as red, green and blue or as brightness and two colour
  differences, 8 bits a sample, are shown in their own colours, through no window. They have
  no 3D view, and their card counts pictures, not slices.
- Voxa keeps a log, `voxa.log`, in the folder the system gives it: what it opened, what it
  could not open and why, and the graphics device it draws with. It holds Voxa's own words
  only, never a patient's details or a tag's value, and is cut back when it passes 1 MB. The
  About window says where it is.
- Voxa remembers between runs the window's size, how many viewports there are, which way
  round left and right are shown, whether the viewports are linked, and how labels are drawn
  over the planes.
- The release program for Windows opens no console window beside its own. Its help, its
  version and a failure to start are said in a message window. The window has an icon.
- Screen readers are told what the interface holds, through AccessKit.
- The made-up study that `write_phantom` writes also holds a series of two volumes of the ball
  at two echo times, three locating images in three planes, and a picture in colour. A picture
  of the window can ask for a series by words from its description (`VOXA_CAPTURE_SERIES`) and
  for its next volume (`stepped`).
- Labels named with `--labels` that cannot be opened yet, because the series shown has no 3D
  view, now say so in a notice, once. They open on the first series shown that has one.

### Changed

- The words about labels no longer say that another program made them where Voxa did: the
  list, the plane views and the scene say "made by Voxa", with the model, the device and the
  day, and "Not for diagnostic use." Labels from a file are still "made elsewhere". The
  strip at the top of the Parts tab is shown while no labels are open on the series, while a
  run is on and while a result is kept.
- If the graphics card the window draws with is lost, which a driver reset can do, Voxa no
  longer stops at the next thing it asks of the card. The window cannot be given another card:
  Voxa says once, in the window and in a message box of the system, that it must be closed
  and opened again, and nothing a person has made (annotations, saved views, a kept result)
  is lost. A real driver reset was not provoked, so this was tested with a destroyed device.
- Automatic contrast counts the values again, over a narrower range, when its first count
  leaves the window across only a few of its bins, as a few very bright voxels make it do; it
  looks again at most three times.
- A file holding many frames is opened once for all of them, not once a frame.
- Tests now read a JPEG picture, a JPEG 2000 picture and a deflated file.
- In the tests, graphics devices without a window come from one adapter and start one at a
  time. The drawing tests stopped for good on one machine when every thread started its own;
  they run on every thread again.
- In the Parts look the camera is fitted to the labels when they reach beyond the scan, as a
  whole head's labels do over a thin slab, so that the solids are seen whole; it is fitted to
  the scan again on leaving the look. The camera keeps the way it faces. Labels no larger
  than the scan leave it where it is.
- A part faded to less than half solid no longer takes the pointer from a solid part seen
  through it: resting on, or clicking, what is seen names and selects that. A faded part with
  nothing solid behind it is still there to point at.
- The selected part is paled on a flat face, such as a cut's, where it has no rim to mark it.
- The labelled solids are drawn faster: a line of sight steps over the blocks of the label
  volume that hold no part, which the graphics device finds from the labels it holds. The
  picture is the same, pixel for pixel. A made-up 200 x 230 x 150 label volume on integrated
  graphics: 37 ms to 18 ms while moving, and about 90 ms to about 73 ms still.
- A volume's values are handed to the graphics device by the worker that made them ready, and
  no longer by the window, which paused to do it when a series first appeared in 3D. For
  512 x 512 x 300 on a discrete card the window's own part went from between 21 and 29 ms to
  between 3 and 5 ms. On integrated graphics the same change was made and could not be timed
  cleanly on a busy machine.
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

- A view saved in the moment before a series' kept views had been read from their file was
  lost when they arrived. It is now kept, after them. A test that saved a view as soon as a
  series was ready failed for this on a slow machine.
- Two tests of the worker that reads label files waited for a quiet moment where they
  should have waited for what they were about, and failed when the machine was slow.

### Known limits

- Labelling needs a runtime and a model folder that Voxa does not come with and does not
  fetch. SynthSeg's trained weights have no licence statement of their own, so none is
  shipped; the person gets them under the terms they come with.
- Labelling is for the structures of a brain, as SynthSeg 2.0 knows them. It labels no part
  outside the brain and no inner ear, takes no correction by hand, and keeps one set of labels
  open at a time. It refuses a stack of 10 or fewer slices, or one 10 voxels or fewer from
  bottom to top at 1 mm, as SynthSeg's own program stops on those; the localiser series of an
  ordinary export are in this range.
- Memory limits it. The check of the model's own work needs about 8.2 GiB of a graphics card
  or 3.1 GiB of ordinary memory whatever the scan, and the networks need 1 400 bytes of a
  card (700 or 900 of ordinary memory on the processor) for each voxel, each with a quarter
  added. A whole head scanned at 1 mm is too large for a 16 GB card whole, so it goes to the
  processor or is cut to the head.
- A run on a card also asks the system for ordinary memory, since Windows sets some aside
  behind what a program puts on a card: 1 900 bytes a voxel, and 8.4 GiB for the check, with
  a quarter added. A run starts only where the system can still promise that, so a whole head
  runs on a card only when some 30 GB of ordinary memory can be promised; otherwise it runs
  cut to the head, or on the processor, and says so.
- A scan cut to the head, as it is when memory is short, gives different labels for about one
  voxel in a hundred, along the borders between structures, than the whole scan would.
  SynthSeg's own program does the same when the head lies elsewhere in its box, so neither
  answer is the truer. The result says that the scan was cut.
- A part's volume is its voxels counted, not SynthSeg's added probabilities, so the two
  differ a little, and the cortex areas come with no table of soft volumes.
- The runtime is loaded as Voxa starts, once a model folder is found, and once in the life of
  the program, so a runtime folder changed after that takes effect when Voxa is next started.
  DirectML is frozen: Microsoft supports it and adds nothing to it.
- Integrated graphics are slower than the processor and are used only when chosen.
- Graphics cards run the networks on Windows only. On Linux and macOS the processor does;
  this release has not been built or run there. A run takes a large part of a card's memory,
  and the 3D view may be slow while the card is busy.
- The day a result says it was made is the date at Greenwich (UTC), not always the local one.
- The stage of a run and **Stop** are shown in the Parts tab only.
- A lost graphics card cannot be recovered without closing Voxa and opening it again.
- `cargo deny` reports six advisories against dependencies that `main` already has, and this
  release changes none of them. RUSTSEC-2026-0367, -0368 and -0369 are against `zip` 8.6.0,
  the library that reads a person's ZIP export; `main` needs `zip` 9.0 or later.
  RUSTSEC-2026-0370, -0371 and -0372 are notices that `zvariant`, `zbus_names` and
  `zvariant_derive`, which come with the interface library's `accesskit` feature, are no longer
  maintained.

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
