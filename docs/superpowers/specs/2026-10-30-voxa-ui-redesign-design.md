# Voxa's window, redesigned: design

Date: 2026-10-30
Status: approved by the owner on 2026-10-30, with the calls marked *my call* and the new
intended-use wording (section 8). Decisions the owner made are marked *owner*, with the day;
the rest are mine, marked *my call*, for the owner to overrule.

Living documents this work starts, and keeps current from now on:
[requirements](../../quality/requirements.md),
[risk register](../../quality/risk-register.md),
[usability file](../../quality/usability.md).

## 1. Why

Voxa works, and is fast, but it is hard to find your way around. In 0.6.0 the window opens
with about forty controls in three rows across its top, in no grouping by task: "Patient
details" sits beside "Smooth", the 3D buttons beside the slice buttons. The scanned paperwork
is listed, and opened, before the scans. The measurements window opens over the list of
series. Warnings such as "! several planes" sit in the list as if they were faults. The
labelled parts are one long list with no overview. In a small window the rows wrap and push
the pictures down.

The owner wants Voxa used by people generally, and wants it to look modern and make things
easy to find before it is released publicly (*owner*, 2026-10-29).

## 2. Who it is for

1. People viewing their own scans: an export on a CD or a download from a hospital.
2. Students and researchers: teaching, checking labels over scans, trying things.

Never diagnosis. Voxa is not a medical device and is not for diagnostic use (the disclaimer
stays word for word).

## 3. Success test

A person who has never seen Voxa opens an export, finds the brain scan, and measures
something on it, within a minute, unaided. The usability file holds the script for this
test and its results; it is run on 0.6.0 by a small private preview (*owner*, 2026-10-30)
and again on the redesign.

## 4. Goals, non-goals, anti-goals

**Goals** (agreed scope, *owner*, 2026-10-29):

- G1. Layout A, task panels (*owner*, 2026-10-30): a slim top bar, a rail of scans on the
  left, the pictures in the middle, a panel of tabs on the right.
- G2. Every action can be found by typing in one search box, and run from the keyboard.
- G3. Every DICOM file opens at a basic level: scroll, brightness and contrast, measuring in
  millimetres where the file gives the spacing, and its tags. Reports are shown as text and
  scanned pages as pictures.
- G4. Full profiles for MR and CT, chosen from the file's Modality: CT gets named windows in
  Hounsfield units (brain, bone, lung, soft tissue) and the value under the pointer in HU;
  MR keeps auto, soft and hard.
- G5. A Tools tab holding Voxa's own tools (labelling a brain scan) that the plugin system,
  built later in its own session, fills.
- G6. Plain-language scan names, with the scanner's own name underneath.
- G7. Patient details hidden unless asked for; one quiet place for notices.
- G8. Light and dark themes; fits any screen down to about 640×480; interface size;
  draggable column widths; full screen with F11; screen-reader names on everything.
- G9. The standards' documents started and kept: requirements traced to tests, a risk
  register, a usability file.

**Non-goals** (not in this work):

- Loading or running plugins (the plugin session), and a researcher "advanced" area. The
  redesign leaves room for both and builds neither (*owner*, 2026-10-30).
- CT reconstruction grouping, 3D bone from thresholds, thick-slab views, whole-body MRI,
  new analyses.
- Full profiles for PET, ultrasound, X-ray, mammography, RT and SEG objects (each later, as
  its own project with public test data).
- A made-up sample study inside the app, a recent-scans list, comparing two studies,
  de-identified sharing. The first screen leaves room for a sample; the rest stay out, and
  nothing here stops them (*my call*).
- Any change to how scans are read, cached or drawn: that is the large-scans session's.

**Anti-goals** (would make this a failure even if it worked):

- A radiologist's workstation: more controls, more modes, more jargon.
- Anything that reads as diagnostic: "normal", "abnormal", "lesion", a finding, a score.
- Losing what people have saved: settings, 3D views kept per series, measurements.
- Hiding a fact that changes what a picture means (a series shown as a plain stack, labels
  from another program, a model's failed check) for the sake of a tidy look.
- Making Voxa slower or hungrier on large scans.

## 5. Constraints

- Rust, egui and eframe 0.36, wgpu 30, Windows only. No new third-party crate unless the
  plan says why; any that is added goes through `cargo deny` and the notices.
- The gate, `cargo xtask check`: rustfmt, clippy pedantic as errors, files of at most 400
  lines and functions of at most 60, every test, `cargo deny`, rustdoc with warnings as
  errors. Green before every commit.
- How a scan is read, cached, built into a volume and drawn on the GPU stays as it is, and
  so do the workers. The window's state and its rules change where the new window needs
  them: `AppState` gains the arrangement (7.5) in place of today's `MainPanel` and
  `ColumnShows`; the window's own state (beside today's list width and interface size)
  gains the open tab, the theme and the folds; the reducer gains actions and lets
  measuring happen on slices without a volume (7.7); the file of measurements kept per
  series gets a new version that still reads the old one, and the saved CSV and JSON gain
  columns saying where a measurement's millimetres came from (*my call*, after the builder
  check).
- Nothing from a real export in code, tests, documents or commit messages. Examples use the
  `write_phantom` study or invented values.
- Large scans: the window may add no work per frame that grows with the size of a scan, and
  holds no scan data of its own (see 7.17).
- Two other sessions work in parallel on their own branches from `release`: large scans
  (voxa-dicom, voxa-volume, voxa-render, loading) and plugins (voxa-models, voxa-segment).
  This work stays in voxa-app's window code, with small, additive exceptions in voxa-dicom
  and voxa-core: listing documents (7.15), and two facts the window needs to measure and
  label values safely (9.2).

## 6. Approaches

### Layout (chosen by the owner, 2026-10-30)

- **A, task panels** (chosen): every tool has a visible, named home, which suits a first-time
  user working unaided. Costs width; folding panels answer that on small screens.
- B, search first: the most room for the picture and the fastest by keyboard, but a newcomer
  must know to search. Its search box is kept in A.
- C, mode rail: one task at a time, tidy, but each mode hides the others' tools.

### How it is built (chosen by the owner, 2026-10-30)

1. Rearrange in place: move today's controls into the new places and write the search as its
   own list. Quickest to look different, but the keys already live in two tables and in
   hand-written button hints; a search list and screen-reader names would be further copies
   that drift apart.
2. **One list of actions first, then the new window around it** (chosen). Each thing Voxa
   can do is described once; search, keys, the help window, button hints and screen-reader
   names all read that list, and a test can prove every action is findable. Then the window
   is rebuilt piece by piece. About a day longer before anything looks different.
3. A docking library (egui_dock, egui_tiles) letting people drag panels anywhere. Ruled
   out: a dependency that must follow every egui release, more freedom than people viewing
   their own scans need, harder to test, and Voxa's columns already drag wider and narrower.

## 7. The design

### 7.1 The window

```
┌ Open · Head MRI, 2 January 2020 ·  [🔍 Find a tool or part   Ctrl K]   ? ☀ ⋯ ┐
├──────────┬──────────────────────────────────────┬───────────────────────────┤
│ Scans    │ [Slices 1 2 4 | Slices+3D | Planes | 3D]       slice 88 of 176   │
│ ▣ Brain… │                                      │ View Measure 3D Labels Tools│
│ ▣ …      │            the pictures              │  (the chosen tab)          │
│ ▸ Other  │                                      │                            │
├──────────┴──────────────────────────────────────┴───────────────────────────┤
│ Voxa is not a medical device and is not for diagnostic use.      ⓘ 2 notices │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **Top bar**, one line: Open; the study's name and date; the search box; help; theme; a
  menu (⋯) holding interface size, full screen, keys and mouse, the log's place, and About.
- **Scans rail**, left (7.4).
- **Pictures**, middle, under a thin strip that chooses the arrangement (7.5).
- **Tab panel**, right: View, Measure, 3D, Labels, Tools (7.6 to 7.10). Exactly one tab is
  open; the panel remembers the last one.
- **Bottom line**: the disclaimer, word for word, and the notice area (7.11).
- Both side columns can be dragged wider or narrower (as the series list can today), and
  folded to a strip of icons; folded, a click or the search opens them. `B` folds the rail
  (as it hides the list today); a key the plan picks folds the panel.
- Nothing floats over the scans rail. Windows that must float (About, help, the tags of a
  file, saving dialogs) open over the pictures and close with Esc (*my call*).

### 7.2 The list of actions

One table describes every action a person can take. Each entry has:

- a stable identifier;
- its name in plain words, as on its button ("Measure a length");
- other words people may search with ("ruler", "distance", "size");
- its key, if it has one;
- where it lives: the top bar, a tab, a scan's card, the menu;
- when it can be used, and when not, why not, in a sentence ("Measuring needs a scan with a
  scale in its files.");
- which profiles offer it (7.6), when not all;
- what it does: the `Action` (or window request) it sends.

Read from the table: the search (7.3), the keys, the help window's list of keys, every
button's hint, and every control's screen-reader name. Parts of the open label set, the
scans of the open export, and help topics are searched too, but are not actions and are
not in the table.

Tests over the table: every action is found by its own name and by each of its other
words; no two actions share a key in the same place; every key in the table does what the
table says; every button drawn has an entry (a test walks each tab and the bars and checks
each control's accessible name against the table). The plugin system later adds its tools
to the same table at run time.

### 7.3 Search

- Opened by clicking the box, Ctrl+K, or `/`. Esc closes it. Arrows move, Enter runs.
- Finds actions, parts in the open label set ("hippocampus"), scans in the open export
  ("T2"), and help topics ("How measuring works"), grouped under those four headings, the
  best match first.
- Matching: whole words and starts of words before letters scattered through a word;
  case and accents ignored; no new crate (*my call*).
- An action that cannot be used now is still listed, greyed, with why not; Enter on it says
  why in the notice area rather than doing nothing (*my call*).
- An action that throws work away (clear the measurements, close the labels) asks first,
  or can be undone, whether run from a button or from search.
- Nothing typed is kept between runs.

### 7.4 Scans rail

- Studies by date, newest first, as today. Each scan is a card: a thumbnail, a plain name,
  the scanner's own description in small type beneath it, and one line of facts (slices,
  spacing, plane).
- **Plain names** (*my call*, waiting on the owner's answer to the open question in 9.1):
  built only from what the files say, never guessed from timings. The scanner's own words
  are expanded where they are a known abbreviation (for example the word "FLAIR", "TSE",
  "SAG", "3D" in a description), the body part comes from the file's own body-part element,
  and the plane and 2D/3D from the geometry Voxa already reads. When the description gives
  nothing recognisable, the name is facts alone ("Axial scan, 3D"). The original is always
  under it, unchanged.
- **Scans** are every series of pictures of the body: volumes, plain stacks, single
  greyscale images (an X-ray), many-frame series (an ultrasound clip), in grey or in
  colour, and series Voxa cannot read yet. **Other files** are locating images, reports and
  other documents (7.15), and series whose Modality is SC, OT or DOC (scanned pages,
  screenshots), in a group at the bottom, closed by default, with how many it holds. The
  scans open first, always (*my call*, after the builder check).
- While patient details are off, a scanned page's card shows a page icon, not a thumbnail,
  since paperwork often carries the name in the picture itself; opening it shows the page
  (*my call*).
- A series that is a plain stack rather than a volume, or that cannot be read, has a small
  mark on its card. Its sentence (today's `SeriesProblem` sentence) is in the card's hover
  text and in the notice area when it is opened. Words such as "several planes" no longer
  sit in the list.
- A card's menu (right-click or its ⋯): open in the selected view, show the file's tags.
- With more than a screenful of cards the rail draws only those in sight, so a study of
  hundreds of series scrolls smoothly (*my call*).
- While patient details are off, the patient's name appears nowhere in the rail. When they
  are on, it heads that person's studies (7.12).
- When an export holds more than one person's studies, the rail says so at the top ("This
  export holds scans of 2 people.") and heads each person's studies "Person 1", "Person 2",
  or by name while patient details are on (*my call*; risk R-UI-05).

### 7.5 The pictures

- The strip above the pictures chooses one arrangement: **Slices** (one, two or four
  viewports), **Slices + 3D**, **Planes** (axial, coronal, sagittal and 3D, two by two),
  **3D**. Today's "large part and column" becomes these four; the right-hand column of
  pictures goes, since the right side is now the tab panel. The Parts list moves to the
  Labels tab.
- Voxa opens an export on **Slices**, with the remembered number of viewports, one on a
  first start, showing the scan Voxa picks as best today (most voxels among clean stacks);
  with no such scan, the scan with the most pixels; with no scans at all, the first of the
  other files. 3D is one click away (*my call*: a newcomer measures on a slice, and the
  first slice shows at once even on very large scans). Opening another export in the same
  run keeps the arrangement, as today.
- The strip's right end says where you are ("slice 88 of 176"), and which volume of several.
- Arrangements a scan cannot have are offered with why not, as actions are.

### 7.6 View tab, and profiles

A **profile** is chosen for each scan from its Modality: **CT**, **MR**, or **basic** for
everything else (*owner*, 2026-10-29). The profile decides what the View tab offers and
what the readout says; it changes nothing in how the scan is read or drawn.

| | Basic | MR | CT |
|---|---|---|---|
| Brightness presets | Auto | Auto, Soft, Hard (today's) | Brain, Soft tissue, Lung, Bone, and Auto |
| Fine-tune | width and level | width and level | width and level, in HU |
| Value under the pointer | the value, "no unit" unless the file names one | the value, "no unit" | in HU |
| Measuring | where the file gives spacing | yes | yes, regions in HU |

- CT windows, in HU as width/level (*my call*, common teaching values): brain 80/40, soft
  tissue 400/40, lung 1500/−600, bone 1800/400. Each button's hint says the numbers.
- HU is said only when the modality is CT and the file's rescale says HU or says nothing;
  otherwise the value is shown with no unit (risk R-UI-02).
- The View tab also holds: orientation (radiological or neurological, said in words:
  "Patient's left on the right"), play through the slices with speed and loop, flip and
  turn, fit, smooth planes, link views, the volume of several shown, and "Show patient
  details" (off unless turned on, as today).
- Pictures in colour (scanned pages, screenshots) have no brightness presets and say so.

### 7.7 Measure tab

- Today's measurements window becomes this tab: the tools (move, ruler, angle, region,
  marker), undo and redo, the list of measurements of the scan shown, renaming, and saving
  the list as CSV or JSON.
- **Measuring works on the slices too**, not only on the planes and in 3D, so the success
  test can be done in the arrangement Voxa opens on. It works from the slice itself, as
  soon as the slice is shown, without waiting for a 3D volume, which can take long on a
  large scan; a region's values come from that slice (*my call*, after the builder check).
  Measurements are kept as now, in the patient's millimetres, for a scan whose files say
  where its slices lie.
- A scan whose files give a pixel spacing but not where the slices lie (many single images
  and stacks) is measured within the picture, from that spacing, and its measurements say
  so ("from the file's pixel spacing").
- A scan with no spacing at all is not measured in millimetres; the tools say why. Voxa does
  not use a detector's spacing (Imager Pixel Spacing) as if it were the body's (risk
  R-UI-03) (*my call*).
- Measurement labels in a picture never overlap one another: a label that would collide
  moves aside, with a thin line to its shape (0.6.0 draws them on top of each other).

### 7.8 3D tab

Today's second toolbar row: the looks (Surface, VR, MIP, Parts), the six sides
(A P L R S I, with their words), cuts and their removal, the slice drawn in 3D, saved views,
and saving a picture or a turntable.

### 7.9 Labels tab

- With labels open: who made them (another program, or Voxa) and the model's own check,
  first, as today; then an **overview by group** (each group's colour, how many parts, all
  shown or hidden, one click to open the group); then the full list with search, as today.
- Without labels: what labels are, one sentence, and the two ways to get some: open a
  label file, or label the scan in the Tools tab.

### 7.10 Tools tab

- **Part of Voxa**: "Label this brain scan", moved from the top of today's Parts tab, with
  its state, progress and stop, as today.
- **Added by you**: empty in this work, with one line saying plugins come later, and room
  for the plugin session's entries: each tool's name, who made it, where it came from, its
  licence, whether its files passed their check, what it accepts and returns, and a run
  button (*owner*'s plugin mockup of 2026-10-29). Results of a tool land in the Labels tab
  with the banner "Added by you, not part of Voxa".
- Every tool is an entry in the list of actions, so search finds it.

### 7.11 Notices

- One place, at the right of the bottom line: an icon, the newest notice cut to fit, and how
  many there are. A click opens the list of all of them, newest first, each with Dismiss.
- Two kinds only: information, and something to look at (amber). Nothing red: Voxa reports
  no faults in a person.
- The sentence of a series' problem is posted when that series is opened.

### 7.12 Patient details

Hidden unless "Show patient details" is on (as today), in the View tab and in search. When
on, the name and identifier show in the viewports' corners, the tags and the rail's
heading. Voxa never writes them into a thumbnail, a saved picture, a log line or a saved
file name, whatever the setting; a name burned into a picture by the scanner is part of
the picture and cannot be hidden, which is why pages show an icon while details are off
(7.4). The
setting is not remembered between runs (*my call*: the safe default every time).

### 7.13 Themes

System, light or dark, chosen from the top bar and remembered; system by default. The
pictures stay on black in both themes, and their letters, scale bars and outlines keep
today's colours; only the panels and bars change. Colours are defined once per theme, and
each pair of text and background meets WCAG AA contrast, checked by a test (*my call*).

### 7.14 Screen size, scaling, access

- First start: the window fits the screen it opens on. Smallest window about 640×480, where
  both side columns fold and the top bar keeps Open, search and the menu.
- Interface size from the menu, remembered, as today; full screen with F11.
- Every control has a screen-reader name (egui's AccessKit support is already on). The
  hand-drawn parts (scan cards, the viewports, colour swatches, the arrangement strip) say
  what they are: "Brain, 3D T1, 176 slices, selected"; "Axial slice 88 of 176".
- Tab moves through the top bar, the rail, the pictures and the panel in that order; arrow
  keys move between cards in the rail; Enter opens.

### 7.15 Documents: reports and pages

- Scanned pages (DICOM images in colour) are pictures, as today, in Other files.
- A structured report (DICOM SR) is listed in Other files and opens as text in the middle:
  headings, text, numbers with units and dates, in the report's order. Its top says "Shown
  as text. The original report is the reference." This needs one small addition in
  voxa-dicom: today a DICOM file with no image is skipped. The index learns to list such
  files as documents (their kind, date and title) and a new reader turns an SR's content
  into lines of text, with a limit on nesting and length (security note S-UI-01).
- A PDF report stored in DICOM (encapsulated PDF) is listed, says it is a PDF, and offers
  "Save a copy as PDF…" to a place the person chooses (*my call*: Voxa draws no PDF and
  starts no other program; opening it in the system's reader can come later).
- Other files Voxa cannot show (presentation states, key-image notes, RT and SEG objects)
  are listed with their kind in words and their tags.

### 7.16 What is remembered

- Kept and still honoured: window size, interface size, the rail's width, layout of slice
  viewports, the radiological convention, linking, how labels are drawn, 3D views and
  measurements kept per series. An old settings file opens without loss; values whose
  meaning changed are carried over (today's column width becomes the tab panel's width,
  within its limits).
- New: theme, the tab last open, which columns are folded.
- Still not kept: which scans were opened, anything typed in search, patient details on.

### 7.17 Performance

- The window holds no scan data. Thumbnails are made as today.
- No per-frame work grows with the size of a scan or the length of a study: the rail draws
  only the cards in sight; the search's index is rebuilt only when actions, scans or labels
  change, not each frame; the profile is worked out once per scan.
- Budget: drawing the window's bars, rail and panel (without the pictures) takes no longer
  than 0.6.0's bars and list on the same machine at 1400×900, measured in a headless test
  frame, and stays within that with a made-up study of 500 series (*my call*).

### 7.18 Help topics

Short help topics, written in this work, a few sentences each, found by search and listed
under help: opening an export and where to find it on a CD; finding your scan among the
others; brightness and contrast; measuring, and where the millimetres come from; telling
left from right; labels, and who made them; what Voxa is not for. Each is checked by the
wording test (UI-38) and read against the risk register before it is committed (*my
call*, after the builder check).

## 8. Standards documents

This work starts, and from now on keeps, three documents under `docs/quality/`:

- **Requirements** (`requirements.md`): each requirement numbered (UI-nn for this work), with
  the test or recorded check that shows it is met. The plan fills in test names as it builds.
- **Risk register** (`risk-register.md`), ISO 14971 in spirit: each hazard, how harm could
  follow, the control, how the control is checked. This work's entries are R-UI-01 to
  R-UI-10.
- **Usability file** (`usability.md`), IEC 62366-1 in spirit: who uses Voxa and where, the
  user-interface parts that bear on safety, the problems seen in 0.6.0, the test script, the
  preview's and later tests' results.

Also: no new third-party component is planned (if one is added, its risk goes in the
register and its licence in the notices); security notes for reading reports from
untrusted files and saving a PDF copy; and the intended-use statement changes from "MRI
scans" to MRI and CT scans, and other DICOM files at a basic level. The owner approved
that change of wording, with the design, on 2026-10-30.

## 9. Open for the owner

### 9.1 Plain names and guessing

The any-mri design (2026-10-13, item 5, *my call* then) said Voxa never guesses a weighting
such as "T1" or "T2". Plain names (G6) respect that in 7.4 by translating only the
scanner's own words. The alternative, working the weighting out from the timings, would
name more scans but could name one wrongly. Put to the owner on 2026-10-30 with 7.4 as the
default; approving this design keeps it.

### 9.2 Two facts from the shared scan-reading code

Today, when a file gives no pixel spacing, the index fills in 1 mm, and nothing after it can
tell a filled-in 1 mm from a real one; and the index does not read the file's rescale
type, the element that says the values are HU. 0.6.0 is safe because it measures only on
full volumes, which always have real spacing. Measuring on slices (7.7) and showing HU
(7.6) need both facts: two small, additive fields in voxa-dicom's header and index and in
voxa-core's `SeriesInfo`, code the large-scans session also works in.

Decided (*owner*, 2026-10-30): this work adds both fields on its own branch and tells the
large-scans session.

## 10. Coordination with the other sessions

- **Plugins**: the Tools tab and the list of actions are built here; the plugin system
  feeds them (a tool entry as in 7.10, and its actions) rather than drawing its own panel.
- **Large scans**: this work does not touch reading, caching or building volumes. It shows
  whatever progress the session reports, on the scan's card and in the strip. The small
  voxa-dicom addition for documents (7.15) is in new files plus one place in the index
  where a file is skipped today; the two facts of 9.2 are new fields read beside the
  existing ones in `header.rs`, `index.rs` and `SeriesInfo`.
- Files this work restructures: `crates/voxa-app/src/ui/` (the bars, toolbar, scene bar,
  browser, layout, pictures, measurements window, Parts tab, tags, help, keys, status line,
  theme). Other sessions should avoid changes there, or say so.

## 11. Left to the builder

The order of the work, the names of modules and types, the identifiers in the list of
actions, the key that folds the tab panel and any keys for the tabs, the icons, the exact
sizes and spacings, how the overview by group is drawn, how measurement labels move aside,
and how the plain-name vocabulary is stored, so long as each requirement in
`requirements.md` is met and shown by a test or a recorded check.
