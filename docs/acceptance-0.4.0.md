# Acceptance record for 0.4.0

Date: 10 October 2026. Machine: a Windows 11 desktop with integrated graphics and a discrete
graphics card. Build: `--release`, Rust 1.99.0.

This record says what was checked, how, and with what result. Nothing in it describes a patient:
a real export and label files made for it were also checked, on the machine, and nothing from
either is recorded here. Pictures made from them were looked at and are kept out of this
repository. What was not checked is listed as such at the end.

## Automated gates

| Gate | Result |
|---|---|
| `cargo xtask check` (formatting, lints as errors, 400-line limit, tests, licences and advisories, documentation) | passes |
| Tests in the workspace | 749, all passing |
| `cargo xtask coverage` (line coverage, floor 80%) | `voxa-core` 96.6%, `voxa-dicom` 98.4%, `voxa-volume` 99.2%, `voxa-labels` 98.8% |

What the tests of this milestone hold to:

- **Reading labels.** A single-file NIfTI-1 volume in each kind of whole number labels are
  stored as, in either byte order, plain or gzipped, placed by either of the two ways a NIfTI
  file places its voxels, comes back with every number where it was put and its voxels where
  the file says, turned from the file's right-anterior-superior to DICOM's
  left-posterior-superior. A file that is not that (NIfTI-2, two files, more than one volume,
  scaled values, values that are not whole numbers, no placement, too large, cut short) is
  refused with a sentence that says which. Random bytes, and a sound file damaged anywhere in
  its header or cut short, give labels or a reason and never stop the program; the same holds
  for a table of names damaged anywhere (property tests).
- **Names.** The three kinds of table are read in their own order; a part the table does not
  name is listed as not named; a number two names claim goes to the first; names are kept to a
  length. How far a name can be trusted is one of four words, and a table that carries the
  labelling model's own check gives its verdict.
- **Parts.** A part's volume is its voxels times the size of one voxel of the label volume.
  The part at a point of the patient is that of the nearest label voxel; the first part a line
  of sight meets is found among the shown ones only.
- **On the graphics device.** Drawn with no window, from labels on a grid of their own turned
  round from the scan's: a tint falls on each half on the side of the picture its letter is;
  an outline leaves the scan alone and draws the line between parts; with the overlay off the
  picture is the plain one exactly; a hidden part is neither tinted nor outlined, a faded one
  is tinted less, and the selected one is outlined in white; labels keep their place over a
  tilted scan, and labels nowhere near the scan change nothing. As solids, each half is on
  the side the patient has it, with black round them; a hidden part uncovers what is behind
  it and a faded one lets it show; a cut shows the parts' colours flat on its face; the
  selected part has a pale rim.
- **In the state.** Labels are shown on the series of the study they were opened on and on no
  other; labels nowhere near the series are refused; opening another export gives them up;
  the Parts look cannot be chosen without labels, and the scene falls back to Surface when
  they are closed.
- **The list.** What it lists for a search; that clicking down it finds every switch, name
  and button it offers; what it says of where names came from and of a failed check.
- **Left against right, and the table.** 4.0 ml against 6.0 ml is "smaller by 40% of their
  mean"; equal sides are "the same size"; a part with one side is not compared. The saved
  table has a line for every part, hidden or not, quotes what needs quoting, writes a name a
  spreadsheet would run as a formula as text, and holds nothing of the scan.
- **The made-up labels.** What `write_phantom` writes reads back as a shell in two halves, a
  core and a bead of the volumes a ball's formula gives, each where the phantom has it, on a
  1.5 mm grid in another voxel order than the scan's, with one part approximate and a failed
  check.

## Which graphics device

Every figure below was measured on the machine's **integrated graphics** (Intel UHD Graphics
770, through Vulkan), which is the adapter the renderer with no window is given first. The
discrete card was not measured. The window itself asks for the faster adapter.

## Times

| What | Time |
|---|---|
| One plane with labels tinted over it, 513 x 513 pixels, drawn and copied back | 2.0 to 8.0 ms |
| The labelled parts as solids, 513 x 513 pixels, drawn and copied back | 8.0 to 41 ms |
| A made-up label volume of 200 x 230 x 150 voxels as solids in a 1280 x 800 picture (`cargo bench -p voxa-render`, "parts") | 64 ms still, 15 ms while moving |

Labels cost one more texture on the device, two bytes for each voxel of the label volume,
whatever the size of the scan. Hiding, fading or selecting a part sends only the small table of
how each part looks.

## The made-up study

Checked with `cargo run --release -p voxa-app --example check_labels`, which opens an export
and a label file through the same session and workers as the window, and on every series with
a 3D view that the labels reach draws the three planes through a labelled point, with and
without the labels, and the parts as solids, with the same renderer. It prints counts, shares
and times and nothing from the files.

| What | Made-up study |
|---|---|
| Series | 2 |
| Series with the labels on them (those with a 3D view that the labels reach) | 2 |
| The labels change the picture of each of the three planes | 2 of 2 |
| The solids draw something, and hiding every part leaves nothing | 2 of 2 |
| Share of what is labelled that lies inside the series | all |
| Parts, and groups | 4 in 2 |
| Names with a left and a right, and those whose left part lies towards the patient's left of the right | 1, 1 |

A label file does not say which scan it was made from, and labels are refused only when they
lie nowhere near a series. Where labels cover more than a series does, they are drawn whole
in 3D and only where the series is in the planes.

A real export, with label files made for it, was also checked this way and looked at,
locally. Nothing from either is recorded here.

## Looked at in the window

Pictures of the window were taken with `VOXA_CAPTURE` on the made-up study with its made-up
labels, at 1440 x 900 and at the minimum size of 900 x 600, and looked at:

- **The Parts tab.** The parts by group, the shell's two halves on one row with the volume of
  each and a switch for each, the bead marked approximate, the failed check in the warning
  colour, and "Save as CSV…" beneath it.
- **Labels over the planes.** Outlined and tinted, the core, the shell and the bead fall on
  the ball as the scan shows it in all three planes, with the bead towards the patient's left
  and front. Each panel says "Labels: made elsewhere".
- **The Parts look.** The ball as solids in the scene, whole and cut open to the core, with
  the scene's own words that the solids are labels made elsewhere, and "Slice in 3D" greyed.
- **A part selected.** Its name and volume above the list, with the slider and "Show only
  this"; for the left half of the shell, "Left 102.5 ml, right 104.9 ml: the left is smaller
  by 2% of their mean", and that Voxa cannot tell how much of that is noise. The halves were
  drawn equal: the 2% is where the labels' 1.5 mm voxels fall.
- **Fading and selecting redraw the solids.** Pictures taken before and after a part was
  faded, and before and after it was selected, differ where that part is.

**Faults found by looking and fixed.**

- On real labels the solids were first lit from how the labels slope across one voxel, which
  drew every surface as contour lines. The slope is taken across two and four voxels.
- The list's description (what the labels are, the check) first stayed in place above the
  rows and left room for few of them in the smallest window. It scrolls with the rows; only
  the switches, the search box and the part selected stay.
- The column's three tabs did not fit a third of the smallest window. The column is at least
  260 points wide.
- "Save as CSV…" first took a row of its own above the list. It is under the description,
  and scrolls with it.
- "Slice in 3D" could be switched on in the Parts look and changed nothing. It is greyed
  there, with the reason.

**Left as it is.**

- The solids show faint bands, like contour lines, where the label voxels step; plainest on a
  smooth shape such as the made-up ball.
- Choosing the Parts look does not move the camera, so labels larger than the series shown
  run off the picture until it is zoomed out.
- A selected part seen flat-on through a cut is not marked.
- At 900 x 600, with the planes large and labels on them, a panel's notes run to five short
  lines over the corner of its picture.
- In the smallest window, a selected part with two sides leaves room for about four rows of
  the list without scrolling.

## Not checked

These need a person at the window and have not been tried by hand:

- "Open labels…" and "Names…" through the system's dialog, and dropping a label file on the
  window. (What each opens is tested; labels were opened in the window only with `--labels`.)
- "Save as CSV…" through the system's dialog. (What is written is tested; the dialog is not.)
- Resting the pointer on a part for its name, and clicking a part to select it, in the plane
  views and in 3D, with the real mouse. (Which part lies under a point, and what a click asks
  for, are tested with no window; a name was seen beside the pointer in one picture taken
  while the pointer happened to rest on the scene.)
- The search box with a real keyboard, the slider for how solid a part is, the switches for a
  side and a group, "Show only this" and "Show all", and how the list scrolls.
- A ruler or a marker placed on the solids in the Parts look.
- Turning, cutting and zooming the solids, and whether they keep up while moving.
- A label volume too large for the graphics device. (That it is then listed and not drawn is
  written, and was not made to happen.)
- Anything on the discrete graphics card.
