# Acceptance record for 0.3.0

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 10 October 2026. Machine: a Windows 11 desktop with integrated graphics and a discrete
graphics card. Build: `--release`, Rust 1.99.0.

This record says what was checked, how, and with what result. Nothing in it describes a patient:
a real export was also checked, on the machine, and nothing from it is recorded here. Pictures
made from it were looked at and are kept out of this repository. What was not checked is listed
as such at the end.

## Automated gates

| Gate | Result |
|---|---|
| `cargo xtask check` (formatting, lints as errors, 400-line limit, tests, licences and advisories, documentation) | passes |
| Tests in the workspace | 653, all passing |
| `cargo xtask coverage` (line coverage, floor 80%) | `voxa-core` 96.6%, `voxa-dicom` 98.4%, `voxa-volume` 99.2% |

What the tests of this milestone hold to:

- **Planes.** The three planes of a straight, a sagittally scanned and a tilted grid face as
  radiology shows them; a plane drawn on the graphics device with no window agrees with the
  same plane read on the processor to within a grey level on average; thick slices keep their
  true proportions; outside the volume a plane is black.
- **Measuring.** Distances, angles and the values inside an ellipse are worked out from
  patient millimetres and scan values, never from the screen: a 3-4-5 triangle is 5.0 mm, a
  right angle 90.0 degrees, an ellipse on the test ball has the ball's values. A ruler says how
  exactly it can be known: one voxel along its own direction.
- **Annotations.** Adding, removing, renaming, clearing, undoing and redoing; the list written
  to a file and read back; a file that cannot be read is set aside, not overwritten.
- **Export.** The table and the JSON hold the measurements and the names typed, quote what
  needs quoting, write a name that a spreadsheet would run as a formula as text, and contain
  no identifier or description of the test series.
- **Linking.** A viewport of the same study goes to the slice nearest the position; one of
  another study, or with no place, does not; a viewport that was scrolled stays where it was
  scrolled to.
- **Smooth.** The curve passes through every measured value; between slices the smooth
  picture is within two grey levels of the true value where the plain one is at least five
  off; on the hard-edged test ball the device's picture is the curve as the processor works it
  out, within a grey level on average.

## Which graphics device

Every figure below was measured on the machine's **integrated graphics** (Intel UHD Graphics
770, through Vulkan), which is the adapter the renderer with no window is given first. The
discrete card was not measured. The window itself asks for the faster adapter.

## Times

| What | Time |
|---|---|
| One plane, 513 x 513 pixels, drawn and copied back, plain | 2.1 to 3.3 ms |
| The same drawn smooth (eight blended reads a pixel) | 2.5 to 3.3 ms |
| Working out the curve's coefficients for 512 x 512 x 64 voxels, on one worker thread (`cargo bench -p voxa-render`, "curve coefficients") | 0.20 s |

A plane view costs no second copy of the volume: it reads the 3D scene's own texture. Smooth
costs one more texture as large as the volume, held only while it is switched on.

## The made-up study

Checked with `cargo run --release -p voxa-app --example check_planes`, which opens an export
through the same session, workers and volume store as the window, and for every series that has
a 3D view draws its three planes through the middle, plainly and smooth, with the same renderer.
It prints counts and times and nothing from the files.

| What | Made-up study |
|---|---|
| Series | 2 |
| Series with plane views (those with a 3D view) | 2 |
| Every picture shows something | 2 of 2 |
| The smooth pictures are drawn from the curve, and differ from the plain ones | 2 of 2 |
| A ruler across what is drawn, along the line the axial and coronal planes share, agrees between the two pictures within one voxel | 2 of 2 (0.00 voxels apart) |
| Working out the curve | under 0.01 s |

A real export was also checked this way, locally. Nothing from it is recorded here.

## Looked at in the window

Pictures of the window were taken with `VOXA_CAPTURE` on the made-up study, at 1440 x 900 and at
the minimum size of 900 x 600, and looked at:

- **Planes in the column.** Axial, coronal and sagittal one above another, each under a bar of
  its own colour (red, green, yellow, as in 3D Slicer), with its name and where it lies, a
  letter at each edge, a scale bar, and the lines of the other two planes crossing at the
  position. The axial panel shows the slice the 2D viewport shows, the same way round.
- **The four-panel view.** Axial and sagittal above, coronal and the 3D scene below, the 2D
  viewport in the column. In the scene the three planes are outlined in their colours and
  cross at the position.
- **Measurements.** A ruler, an angle, a region and a marker in the axial plane, each with its
  words beside it, and the same four drawn in the scene. The Measure window lists them with
  their values, what qualifies each, and the buttons to save and clear.
- **Link.** The position is marked on the 2D viewport by a small cross on the slice it lies
  in.
- **Smooth.** Each panel says "Smoothed: a curve, not more detail", or "Interpolated and
  smoothed: layers 2.4 mm apart" where it runs across the slices.

**Faults found by looking and fixed.**

- At 900 x 600 the Link switch, first placed beside the column's tabs, ran over the Enlarge
  button. Link and Smooth are on the toolbar, which wraps.
- A marker's coordinates, written beside it, ran over its neighbours. Beside a marker only
  its name is written; where it is can be read in the Measure window.
- Words beside an annotation near the edge of a small panel ran off the panel, and then over
  the panel's caption. They are now kept inside the panel and beneath its caption.
- A 3D scene that had been turned, given a quarter of the window in the four-panel view, kept
  its scale and showed a corner of itself. A scene the person has moved now keeps what it
  shows when its part of the window changes size.
- In a small scene the captions ran down its side in a column of single words. In a narrow
  picture they go beneath the letter at the top, across the whole width.
- On the hard-edged test ball the smooth planes show a faint rim beside each edge. That is
  what a curve through the values does beside a sharp edge (a test holds the device to the
  curve), and the Smooth switch says so.

**A fault found in review and fixed.** A kept-measurements file damaged into something that
is not text was read as if there were no file, and the next change would have written over
it. It is now put aside like any other file Voxa cannot read; a test with such a file fails
without the change.

**Left as it is.** At 900 x 600, with a ruler, an angle, a region and a marker all in one
layer of a panel 210 points wide, their words run into one another, and in the 3D scene the
words of annotations that lie in one plane pile up when that plane is seen edge-on. Nothing
moves words apart.

## Not checked

These need a person at the window and have not been tried by hand:

- Drawing with the mouse: a ruler by press and release, an angle by three clicks, a region by
  dragging, a marker by a click, in the plane views; a ruler and a marker by clicking surfaces
  in 3D. (What each press, move and release makes is tested with no window; the wiring from
  the real mouse to those is not.)
- Moving the position by clicking and dragging in a plane view, the wheel through layers,
  zooming and panning a panel, and how these feel.
- Dragging the planes' grips and the position in the 3D scene.
- Typing a name in the Measure window with a real keyboard, and the keys: M, Ctrl+Z, Ctrl+Y,
  Esc. (Each is tested as a function of the key pressed.)
- "Save as CSV…" and "Save as JSON…" through the system's dialog. (What is written is tested;
  the dialog is not.)
- Scrolling a linked viewport and watching the others follow in the running window, and
  whether following keeps up while playing.
- Switching Smooth on a large series and seeing how long the planes take to change.
- Anything on the discrete graphics card.
