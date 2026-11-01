# Acceptance record for the redesigned window

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 31 October 2026. Branch `claude/ui-redesign-081656`, from commit `fe4976a`; the version in the
workspace is still 0.6.0, which this work does not change. Machine: a Windows 11 Pro (10.0.26300)
desktop with 31.7 GB of memory, a 12th Gen Intel Core i7-12700K, an NVIDIA GeForce RTX 5070 Ti and
Intel UHD Graphics 770, the display at 200% (two pixels to a point). Builds: the gate's debug build,
and `--release` for the pictures. Rust 1.99.0.

This record says what was checked, how, and with what result, for the work in
`docs/superpowers/specs/2026-10-30-voxa-ui-redesign-design.md`. Everything shown in a picture or
used in a walk-through is a made-up study that nobody's scan is part of: the phantom that
`write_phantom` writes, a made-up report and PDF, and a made-up export with brain-like descriptions.
Nothing from a real export is in this record or in the pictures. Pictures were taken with
`VOXA_CAPTURE` and were kept in the builder's scratch folder, not in the repository, and are not
retained: this record's descriptions are what remains of them, except for the five pictures of the
README (below), which are in the repository.

**What this record does not say.** It does not say that the success test is passed (section 3 of
the design): no person has used the redesigned window, and the builder's walk-through (below) is
not a participant. It does not say that every requirement is met without qualification: two rows are met with a
known limitation (UI-13 and UI-25, below), and two risk rows have no check on this branch
(R-UI-11, and the replacement of labels in R-UI-12). What needs a
person at the window is listed as not checked at the end.

**Linux and macOS were neither built nor run**, and continuous integration on GitHub was not run
for this branch. Everything below is Windows.

## Automated gate

`cargo xtask check` was run in the background, in the shell's ordinary environment (no
`WGPU_BACKEND`, `VOXA_MODELS` or `VOXA_RUNTIME` set), from the tree at `fe4976a`, three times: before
the documents and pictures of this commit were written, after most of them, and last of all just
before the commit, with the same result each time. All three exited 0.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | passes |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` (pedantic lints as errors) | passes |
| 400 lines to a source file | passes |
| `THIRD-PARTY-NOTICES.txt` names every package built into Voxa | passes |
| `cargo test --workspace --all-features --no-fail-fast` | **1,923 passed, 0 failed, 17 ignored** |
| `cargo deny check` | advisories, bans, licences and sources all ok; duplicate-version warnings only |
| `cargo doc --workspace --no-deps --all-features` with warnings as errors | passes |

The 1,923 are 1,114 in `voxa-app`'s own tests and 11 more tests of `voxa-app` that run its
programs, and the rest in the other crates. The 17 ignored need a model folder and ONNX Runtime
(`VOXA_MODELS` and `VOXA_RUNTIME`) and were not run: 7 in `voxa-models` and the 10 of
`crates/voxa-segment/tests/with_models.rs`. Each run took about 1 minute 20 seconds (18:55 to 18:56,
19:13 to 19:15 and 19:16 to 19:18) because the debug build was already up to date for this tree: it compiled
nothing, and the tests all ran. That is well inside the 25 minutes the task allows.

Every test named in `docs/quality/requirements.md` and in `docs/quality/risk-register.md` was also
checked to exist: 389 references, 342 distinct test names. For each, the source file holds the
function, and `cargo test --workspace --all-features -- --list` lists a test of that name (1,940
tests are listed: the 1,923 that passed and the 17 ignored). **Four references were wrong** (cited in the
rows UI-26, UI-30, UI-32 and UI-36), naming files the tests had since moved out of
(`ui/tabs/labels/tests.rs` to `ui/tabs/labels/tests/short.rs`, and `prefs/tests.rs` to
`prefs/carried_tests.rs`). They are corrected in this commit. These are the figures of `fe4976a`;
the gate on the head after the final review is under "After the final review".

## The requirements

There are 39 rows, UI-01 to UI-39. **Every row has something in "Shown by".** As the rows now
stand, counted from them:

- **Met, shown by tests, 37 rows:** UI-01 to UI-12, UI-14 to UI-24, UI-26 to UI-39. Four of these
  rest on a capture looked at by eye as well as on tests: UI-01 (the whole window), UI-29
  (themes), UI-30 (the smallest window) and UI-02 (the disclaimer at twice the size, looked at
  again after the final review's fix). UI-25, below, does too. This record describes those
  pictures.
- **Met, with a known limitation, 2 rows:**
  - **UI-13** (patient details appear nowhere while off, and never in a saved thing): met within
    the limits the row states. A name written another way inside a report's free text (an
    initial, a title, a different spelling, a surname alone) cannot be found reliably and may
    show, and an identifier of fewer than four letters or digits is not looked for, so that it
    cannot change a number. The rail, the viewports' corners, the tags, the documents, the
    notices, a card's hover and screen-reader words, the log, and every saved picture, table and
    file name are shown by tests.
  - **UI-25** (measurement labels never overlap): shown by tests with room to move them and by
    the pictures at the sizes above, and not by the smallest window at twice the size, where the
    labels cover one another (below, Known limitations).
- **Partly met: none.** Three rows were partly met when this record was first written (UI-03,
  UI-07 and UI-13); the final review's fixes made each of them met, UI-13 within its limits.
  UI-04 and UI-05 were counted met while UI-03 was not; now that the labelling strip's buttons,
  the Labels tab's, the notices, About's, the report's Close and Save a copy as PDF…, and the
  slice Loop switch are all in the list, they are met as they say, with the exceptions they
  state (a row that belongs to one item, the answers to a question, and the turntable's Cancel).
- Things a row's evidence does not reach, so as not to be read as more: the call into the window
  library that sets the window's size and place on first start is not run by a test (UI-31); the
  names a screen reader would say are tested as the accessibility tree and were not listened to
  (UI-33); the frame cost (UI-37) was measured on this computer only and says what it does not
  show; and the turntable's progress window is outside the walk of buttons (UI-03, UI-04).

## The risks

| Row | Checked by | State |
|---|---|---|
| R-UI-01 | tests of the naming | filled earlier |
| R-UI-02 | tests with made-up CT, with and without a rescale type, and with MONOCHROME1 | filled earlier |
| R-UI-03 | tests on made-up files with each kind of spacing | filled earlier |
| R-UI-04 | seven tests of label placement, and captures with `measured` | label placement shown; the labels still cover one another in the smallest window at 200% (below) |
| R-UI-05 | tests of an export of two people, and of who is one person | filled earlier |
| R-UI-06 | tests of the card's mark and the notice | filled earlier |
| R-UI-07 | clearing the measurements through search and its undo, the three questions asked before work is thrown away, and every action through search | shown; why the other actions search reaches throw no work away is in the register |
| R-UI-08 | the wording test over text kept in constants, and a reading against the register | filled earlier; text written in drawing code and not kept in a constant is outside the test, as UI-38 says |
| R-UI-09 | the contrast test, three tests that the pictures stay dark, a test that the edge letters and the scale bar are the same colours in both themes, a capture test, and captures in both themes | shown |
| R-UI-10 | tests on made-up reports, including deep and long ones, and on what hiding a name does to a number | shown |
| R-UI-11 | **None yet, and it stays so on this branch.** | see below |
| R-UI-12 | **None yet** for the replacement of labels that Voxa made and did not keep; the question before closing them is tested | open: nothing asks when another run, file or export replaces them |
| R-UI-13 | tests of the edge letters in both conventions, of the corner that says orientation is unavailable, of the default, and of the topic | shown |
| R-UI-14 | the three tests of UI-26 and of the check in plain words | shown |
| R-UI-15 | the tests of UI-02 | shown |

**R-UI-11** (a deeply nested file stops Voxa) has no check on this branch. It is guarded on the
`release` branch: `e3a5477` makes the index refuse a file nested too deeply before the DICOM
library builds it, `0f43193` shows the refused files as a warning that says why and gives readers a
16 MiB stack, `2937954` says so in `SECURITY.md`, and `93d0995` makes its tests read an export of
100 slices so that the refusal is tested on the several-threaded path too. None of these is in
this branch (`git merge-base --is-ancestor 93d0995 HEAD` says it is not). The row is marked
mitigated, with those tests' names, when `release` is merged in.

## Looked at in the window

Pictures of the real window, `--release`, made-up studies, at 640×480, 800×600 and 1400×900, in
both themes (`VOXA_CAPTURE_SHOW=light` for the light one; a picture is dark otherwise, whatever the
system is set to). Sizes are in points; the files are twice as many pixels. Each was looked at, most
in contact sheets of four. These are the pictures as they were taken, before the final review's
fixes (see "After the final review" for what has changed since, among it the Loop box that is now
a switch, the empty window's words and the notice's). What the pictures showed, by group:

**Each tab, at each size, in each theme: 42 pictures** (the phantom with its made-up labels; capture
words `tab-view`, `tab-measure` with `measured`, `tab-3d` with `3d`, `tab-labels` with the labels
opened, `tab-tools`, `search`, `other`).

- *At 1400×900:* the whole window. The top bar is one line: **Open**, the study's name and date
  ("2 January 2020 · Made-up phantom"), the search box ("Find a tool or part", Ctrl K), **?**, the
  theme button and **⋯**. The rail of scans at the left, headed **Scans**, with a card for each of the
  three scans (a thumbnail, the plain name "Axial scan", the description "ball, fine" under it,
  and a line of facts such as "40 slices · 2.4 mm · Axial") and **Other files**, closed, with
  its count, 2. In the middle the strip (**Slices** with 1, 2 and 4, **Slices + 3D**, **Planes**,
  **3D**, and "slice 21 of 40" at its right end) over one slice, with R, L, A and P at the edges,
  the date and name in a corner, a 20 mm scale bar, "Slice 21 of 40 · 48.0 mm" and "W 591 L 316".
  At the right the panel with the tabs **View, Measure, 3D, Labels, Tools** and a **»** that folds it.
  The bottom line: "Voxa is not a medical device and is not for diagnostic use." at the left, and
  "2 files skipped" with an information mark at the right (now "2 files are not scans or documents Voxa lists").
- *View:* Brightness (W 590.97, L 315.60, Auto, Soft, Hard), Orientation ("Patient's left on the right"),
  Slices (Play, 10 /s, Loop), Picture (Mirror across, Mirror down, Turn, Fit), Display (Smooth the planes,
  Link the views, Show patient details).
- *Measure:* the tools (Move, Ruler, Angle, Region, Marker), Undo, Redo, "On this series", and the four
  measurements of the phantom with their values (Ruler 1 38.4 mm, with "Its ends can be placed to
  about one voxel: ± 0.6 mm along this line."; Angle 2 99.3°; Region 3 mean 404.7, SD 28.8, area
  1.7 cm²; Marker 4 at L 28.5, P 4.5, S 48.0 mm), then Save as CSV…, Save as JSON… and Clear all. On the
  picture each is written beside its shape: no label was on another.
- *3D:* the 3D arrangement chosen, a surface of the ball with a note that slices are 2.4 mm apart and
  detail between them is interpolated; Look (Surface, VR, MIP, Parts greyed), Seen from (the six sides
  with their words), Cuts, Slice in 3D, Views and Export.
- *Labels:* "Made-up phantom"; "Made by another program. Voxa shows these labels as given."; the table's
  own check in amber ("The labelling model's own check failed for shell (0.52); below 0.65 fails.
  Take every part here with caution…", which is what the made-up table says); Save as CSV…, Show all,
  Close, Names…; On planes (Off, Outline, Filled); Search parts; Groups (Outside, 3 parts; Inside,
  1 part, each with Hide); the parts with their colours and volumes. Who made the labels and the
  check come before the groups and the list.
- *Tools:* "Part of Voxa", "Label this brain scan", **Label this scan** greyed and **Options**, with
  "Voxa found no model: there is no folder called models beside it, and none has been chosen…"
  (this computer has no model folder); "Added by you", with the line that tools you add will be
  listed there and that adding tools comes in a later version.
- *Search:* the box open, "Search actions, parts, scans and help", with "Type a few letters of what you
  want to do or find." under it, hanging from the top bar over the pictures and the panel. A picture
  cannot type, so no results are shown; results are shown by the tests of UI-05 and UI-06.
- *Other files:* open in the rail at 1400×900, with two cards: "Locating images" with its thumbnail,
  "locating", "3 slices · 8 mm", and "Other picture", "a picture in colour", "1 picture · Axial", with a
  page icon and not a thumbnail, as UI-13 asks while patient details are off.
- *In the light theme* every one of these is the same arrangement with light bars, rail and panel and
  dark text, pale-blue chips where dark has bright blue, and the amber of the check darker. The
  pictures stay black, and R, L, A and P, the scale bar, the outlines and the measurements are as in
  the dark theme (R-UI-09).
- *At 800×600:* the same, with the rail folded to a strip (a **»** and an icon of a list) by the
  window itself and the panel open; the strip above the pictures still shows all its buttons. In
  the *Other files* picture the other files cannot be seen, since the rail is folded (the capture words
  have no way to unfold it): open Other files is seen at 1000×700, 1366×768 and 1400×900 (below). In the
  *search* picture the box's list hangs over the top of the panel.
- *At 640×480:* the top bar is still one line and keeps **Open**, the search box (with its Ctrl K), **?**, the
  theme and **⋯**, with the study's name cut to "2 January 2020 · Made-…". The rail is a strip. The
  strip above the pictures is one **Slices ▾** menu with "slice 21 of 40" beside it, but with the 3D
  arrangement all four arrangement buttons show. The panel is open when the picture asks for a tab,
  and the pictures keep the larger share; with no tab asked for (the search and Other files pictures)
  both columns are strips: at the left the rail's, at the right the panel's five tab icons
  (a picture, a ruler, a cube, a tag, a pair of sliders) with a **«**. The disclaimer is whole at the
  bottom in both themes, with the notice beside it.

**A report, a PDF and notices:**

- *A report open* (a made-up report alone in a folder, so that the export opens on it; 640×480,
  800×600 and 1400×900, both themes): Other files open with one card, "Report", "Made-up phantom
  rep…", "2 January 2020", with a page icon; in the middle the title "Report", the line
  "Made-up phantom report · 2 January 2020", **Show the file's tags** and **Close**, and
  "Shown as text. The original report is the reference.", then the report's items in its order:
  "Description: A made-up ball, written to try Voxa. It is nobody's scan.", "Measurements" with
  "Diameter: 96 mm", "Volume: 463 ml" and "Shape: Sphere" indented under it, "Written on: 20200102" (now "2 January 2020")
  and "Picture the report refers to: an image, not shown". The top bar shows no study's name for an
  export of documents alone. In the light theme the report's page is white. In the smallest window the
  line under the title is cut ("Made-up phant…") and the buttons stay inside.
- *A PDF open* (1400×900, dark): "Report (PDF)", "This report is a PDF. Voxa does not show PDFs." and
  the button **Save a copy as PDF…**.
- *Notices* (1400×900, dark, `notices`): the list open above the bottom line at the right, with
  "2 files skipped" (now "2 files are not scans or documents Voxa lists"), **Dismiss**, and the names of the two files, "labels/labels.nii.gz" and
  "labels/regions.json" (the made-up labels, which the phantom keeps in the study's folder).
- *Other files open at 1000×700 and 1366×768:* the rail stays open at both, with the same two cards.

**Arrangements and the README's pictures (1400×900, dark, from the phantom's scans alone, so no notice
shows):** the slices as the window opens, Planes with measurements, Slices + 3D with a cut and
the slice drawn in the 3D view, Planes with the made-up labels filled over them and as solids
in 3D with "shell, left" chosen, and the made-up two-echo series on its second volume with Other
files open ("slice 9 of 16 · volume 2 of 2 · echo 80 ms"). These are the five pictures of the
README. In Planes with measurements, the four labels in the 3D panel stand close, on adjacent lines,
but none was seen on another.

**The smallest window at 200%** (`VOXA_CAPTURE_SIZE=320x240` with `VOXA_CAPTURE_ZOOM=2`, which is a
640×480 window drawn at twice its usual size): see "Known limitations".

**The empty window** (nothing opened, 1400×900): the rail says, in small grey type, "Open a scan
export to begin: a folder, a zip file or a DICOMDIR."; the middle says "No series is shown here";
the View tab shows its controls with W 1.0 and L 0.0.

## The five pictures of the README

`docs/images/voxa-phantom.png`, `-planes.png`, `-3d.png`, `-labels.png` and `-kinds.png` showed 0.6.0.
They are replaced, keeping their names, by pictures of the real window at 1400×900 points, in the
dark theme as the old ones were, from the made-up phantom only, taken with these capture words (the
labels from the phantom's `labels` folder, read with `--labels`):

| File | Capture words | Shows |
|---|---|---|
| `voxa-phantom.png` | `tab-view` | how Voxa opens the study: the rail, one slice, the View tab. **Not linked from the README before; it is now**, first in "A closer look". |
| `voxa-phantom-planes.png` | `planes,measured,tab-measure` | the three planes and the 3D view with the four measurements, and the Measure tab listing them. The README's first picture. |
| `voxa-phantom-3d.png` | `slices3d,turned,cut,slice,tab-3d` | a slice beside the 3D view, turned and cut, with the slice drawn in it, and the 3D tab. |
| `voxa-phantom-labels.png` | `planes,solids,filled,chosen,tab-labels` | the made-up labels filled over the planes and as solids, the shell's left half chosen, and the Labels tab. |
| `voxa-phantom-kinds.png` | `other,stepped,tab-view` with `VOXA_CAPTURE_SERIES="two echoes"` | the second of the two volumes, Other files open. |

The study had its `labels` folder taken out for these, so that the bottom line does not say that two files
are not scans or documents Voxa lists; the scans are the phantom's. The pictures were made smaller with a lossless re-encoding (no
pixel changes), to between 355 KB and 760 KB each, which is smaller than the ones they replace (0.6 to
1.3 MB). The README's captions and alt text are rewritten to say what each picture shows, in the
redesign's names for the parts of the window. `write_phantom head` was not used: it needs a model
folder's `check` folder, which this computer does not have, and the README's labels picture was always the
ball's made-up labels.

## The builder's walk-through of the success test

Recorded in `docs/quality/usability.md`, section 6, as **a dry run by the builder, not a participant**.
It walked S-01, S-02 and S-07 as a first-time user would, with pictures of the real window taken
with the capture words and with Voxa's own headless interface tests, on the phantom and on a
made-up export with brain-like descriptions. It gives no time for the task as if a person had done
it and claims no pass. It found:

- the steps: from an open window S-02 takes two clicks (the Measure tab, the Ruler) and one drag;
  with Voxa opening on the best scan, S-01 needed no click on the made-up brain export;
- **S-07 could not be done as written:** no key placed or moved the end of a measurement, so S-02 could not
  be done by keyboard alone (D-07). Keys were added afterwards on the slices (see below); S-07 has
  still not been done by a person;
- what would trip a first-time user, D-01 to D-08: the empty window's one instruction in small grey
  type, the rail folded to an unlabelled strip below 960 points as the window opens, plain names cut in the rail and "Cspine"
  reading as a typing mistake, no on-screen word on how to draw once the Ruler is chosen, "2 files
  skipped" on the first screen (now "2 files are not scans or documents Voxa lists"), the View tab's controls beside a report, and that the test script's
  made-up study has no brain scan in it.

Keys that place a measurement were added after this walk, on the slices (Task 22): with a measuring tool
in hand and the keyboard on a viewport, the arrows move a point and Enter places it. Headless tests show
that it makes the annotation the mouse makes and that Up and Down then move the point and not the
slice (`ui::viewport::by_keys_tests` and `by_keys_window_tests`, under UI-22 and UI-33 of the requirements). S-07 has not been done
again, by anyone, and no pass is claimed; the planes and the 3D view have no such keys.

## After the final review

The final review read the whole branch in three parts (the window, the scans and their data, and
the documents) and its fixes were made test first, each with the tests that were written for it. What
they changed in what this record says, and what the user sees:

- **One list of actions, with its last exceptions stated.** Every button that does something is an
  entry of the list: the labelling strip's, the Labels tab's, the notice area's, About's, the
  report's Close and Save a copy as PDF…, and the slice Loop switch, which is now the **Start again
  after the last slice** switch of the View tab. A walk of the window in a labelling run, with a
  kept result, with Voxa's own labels, with a notice, with a PDF open, with About open, with a
  question open and with each menu open fails on a button that is not in the list. The
  turntable's progress window is outside the walk, and its Cancel is by hand on purpose.
- **Questions before work is thrown away.** Closing the labels, deleting the labels kept for a
  scan and deleting a saved view each ask first, from the button and from search; Clear all can be
  undone after another scan was looked at and this one is shown again.
- **Keys.** After Tab the letter keys of the window still work, and on a picture the arrows step
  through its slices, with a ring; Tab reaches the notices last; Space and Enter stay a focused
  button's; a letter pressed while a menu is open belongs to the menu, and the Esc that closes it
  does not also close a window (new in the last fix: before it, such a letter reached the picture
  behind the menu).
- **Patient details.** A document-only export hides the name its own file gives; the notices
  never name the patient while details are hidden and name a skipped file by its last part only;
  masking never changes a figure of a report and says under the heading when it replaced
  something; each report scrolls on its own; nothing a saved picture, table or file name holds
  names the patient, with details shown or not (tests).
- **Words.** The model's own check is said in plain words with no score on the screen; a report's
  dates are said in words; body parts such as "Cspine" are spelled out; the notice of files that
  are not scans reads "N files are not scans or documents Voxa lists"; search says where an action
  lives and ranks a spelt-out word of an action with a word of its name; a card says a distance
  only when the files gave it; the empty window says in its middle how to begin.
- **The smallest window at 200%.** The folded panel's icons share the height there is, so none is
  cut off. A capture of that window with the phantom, taken for this record after the fix and
  looked at, shows the five icons whole, the disclaimer whole with no line across it, and the
  search box showing "Fi…" with its shortcut. See Known limitations for what is still not right.
- **Gate.** The gate was run again on the head of the branch after these fixes: `cargo xtask check`, in the shell's ordinary
  environment, exited 0 in about three and a half minutes: formatting, lints (pedantic, as errors),
  the 400-line limit, the notices file, **2,001 tests passed, 0 failed, 17 ignored** (1,186 of them
  `voxa-app`'s own; the 17 ignored are the ones that need a model folder and ONNX Runtime), `cargo
  deny` and the documentation build. Every test named in `docs/quality/requirements.md` and
  `docs/quality/risk-register.md` was checked again to exist: 514 references to 407 distinct test
  names, for each of which the source file holds the function and `cargo test --workspace
  --all-features -- --list` lists a test of that name (2,018 tests are listed); none is wrong.

## Known limitations

- **Measurement labels cover one another in the smallest window at 200%.** With the four
  measurements of the phantom on a slice (`VOXA_CAPTURE_SHOW=measured`, `VOXA_CAPTURE_SIZE=320x240`,
  `VOXA_CAPTURE_ZOOM=2`), the picture is just over 200 points of the interface across. Looked at
  again after the final review's fixes: the region's words ("Region 3: mean 404.7, SD 28.8") cover
  the scale bar's "20 mm" and touch the angle's words; the ruler's and the marker's labels are
  clear. UI-25 says labels never overlap; it is shown by tests with room to move them and by the
  pictures at the sizes above, and not by this one. The tests also cover a label that has nowhere
  clear to go, which "is still written inside the picture".
- **R45, the folded panel's strip of tab icons cut off at 640×480 at 200%, is no longer one**
  (fixed after the dry run; see above).
- A name written another way inside a report's free text may show, and so may an identifier of
  fewer than four letters or digits (UI-13).
- Labels that Voxa made and did not keep are replaced without a question by another run, another
  label file or another export (R-UI-12).
- A measurement can be placed by keyboard on the slices (arrows move a point, Enter places it), shown
  by headless tests only (`ui::viewport::by_keys_tests`); not tried by a person, with a screen reader or
  on a real scan, and not on the planes or in 3D, which take the mouse alone (D-07, S-07).

## Not checked

These need a person at the window, or something not on this computer, and have not been done:

- **The success test**, by anyone. The formative sessions, and the summative one, are not run, and the
  dry run above is not one.
- **A real mouse, touchpad or touch screen** on any control: pressing, dragging a measurement,
  dragging the column edges, the scroll bars. What each does was run through the actions the buttons
  send and the pointer events the tests make.
- **A screen reader.** The names are tested as the accessibility tree; none was listened to. The
  keyboard order is tested, not tried by hand. F11, the interface-size keys and the size menu were not
  tried by hand.
- **The system's dialogs**: Open (folder, zip, DICOMDIR), Save as CSV, JSON, PDF and the picture, the
  choice of a model folder. Dropping an export on the window.
- **The system's own light or dark choice** changing while Voxa runs, and the window fitting a
  screen on first start on a second monitor or a screen under 640×480 (tested for the sums, not run).
- **A real export** of any kind. Voxa has been opened on real exports on the developer's computer in
  earlier work; nothing from one is recorded here.
- **Other displays**: only a display at 200% was used. 100% and 150%, a 1366×768 laptop screen with
  a scaled interface, and other graphics devices than this computer's were not tried.
- **Labelling** a scan (no model folder here), and the 17 tests that need one.
- **Linux, macOS, and the continuous-integration build.**
- **R-UI-11's guard**, which is on `release` and not in this branch.
