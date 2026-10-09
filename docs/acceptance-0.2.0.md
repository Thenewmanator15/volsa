# Acceptance record for 0.2.0

Date: 9 October 2026. Machine: a Windows 11 desktop with integrated graphics and a discrete
graphics card. Build: `--release`, Rust 1.99.0.

This record says what was checked, how, and with what result. Nothing in it describes a patient:
a real export was also checked, on the machine, and nothing from it is recorded here. Pictures
made from it were looked at and are kept out of this repository. A target that was missed or
not measured is listed as such.

## Automated gates

| Gate | Result |
|---|---|
| `cargo xtask check` (formatting, lints as errors, 400-line limit, tests, licences and advisories, documentation) | passes |
| Tests in the workspace | 460, all passing |
| `cargo xtask coverage` (line coverage, floor 80%) | `voxa-core` 96.2%, `voxa-dicom` 98.4%, `voxa-volume` 99.6% |

Tests that draw pictures run on a graphics device with no window and measure the result: where
the test ball's bead is from each of the six sides, that thick and tilted slices keep their true
proportions, that a volume too large for the device is reduced and stays in place, and that
stepping over empty space leaves the picture as it was.

## Which graphics device

Every figure below was measured on the machine's **integrated graphics** (Intel UHD Graphics
770, through Vulkan). The renderer with no window, which the tests, the benchmarks and the
acceptance check use, takes the adapter the system offers first, and on this machine that is
the integrated one. The discrete card was not measured. The window itself asks for the faster
adapter, so pictures in the running application should be no slower than these.

## Frame times against the target

The plan's target: for a 512 x 512 x 64 volume in a 1280 x 800 viewport on integrated graphics,
at least 30 pictures a second while the camera moves (33 ms a picture) and a full-quality
picture within 250 ms of it stopping. `cargo bench -p voxa-render`, median of 20 samples, each
including the copy of the picture back from the device. While the camera moves the window draws
at half quality and half the size each way.

| Look | Moving | Still, full quality |
|---|---|---|
| Surface | 10.6 ms (94 a second) | 31 ms |
| VR | 10.9 ms (92 a second) | 57 ms |
| MIP | 15.2 ms (66 a second) | 88 ms |

Both parts of the target are met in every look. Runs vary by about a fifth.

A second set draws a 256 x 256 x 64 made-up head at the full 1280 x 800 in each look:

| Look | Full quality | skipping empty space | Half quality | skipping empty space |
|---|---|---|---|---|
| Surface | 36 ms | 26 ms | 21 ms | 26 ms |
| VR | 54 ms | 34 ms | 32 ms | 26 ms |
| MIP | 42 ms | (never skips) | 34 ms | (never skips) |

Skipping empty space takes about a quarter off Surface and a third off VR at full quality. At
half quality the difference is within the variation between runs, and no gain can be claimed.

`cargo bench -p voxa-app`, unchanged in substance from 0.1.0: indexing a zip of 200 slices
8.8 ms, decoding one 512 x 512 slice 1.4 ms, choosing the automatic window 1.2 ms.

## A real export

A real export was also checked, locally, with `cargo run --release -p voxa-app --example
check_volumes`, which opens an export through the same session, workers and volume store as
the window, and draws with the same renderer. Nothing from that export is recorded here.

**A fault found there and fixed.** On the first run the Surface look drew almost nothing. A
scan's air is not empty; it holds faint noise, and the automatic window's low end lies inside
that noise. The Surface look made everything above the low end solid, so the air itself was
drawn as a dark fog in front of the patient. It now leaves out what is fainter than a tenth of
the way up the window. A test with noise in the air around the test ball fails without the
change and passes with it.

## Looked at in the window

Pictures of the window were taken with `VOXA_CAPTURE` on the made-up study, at 1440 x 900 and at
the minimum size of 900 x 600, and looked at:

- The 3D scene has the large part, the 2D viewport is in a column at the right under a strip
  that says "Slices" with an "Enlarge" button.
- From the front the letters are R at the left, L at the right, S above and I below.
- The scale bar reads 20 mm; the caption says that slices 2.4 mm apart are interpolated between.
- With a cut and the slice shown: the cut face shows the scan, the slice's outline and both
  handles are drawn, and the bar shows "Remove cut 1".
- Nothing overlaps at either size. Two overlaps found at the minimum size were fixed: the
  scene's captions against its scale bar, and a long description against the letter at the top
  of a narrow 2D viewport.

## Not checked

These need a person at the window and have not been tried by hand:

- Turning, moving and zooming the scene with the mouse, and how it feels while moving.
- Dragging a cut's handle and knob, and the slice's handle. (Each is tested by a press and a
  move on it with no window.)
- A double-click on a surface to centre on it.
- The Views menu: typing a name, saving, applying and deleting a view. (Saving and loading are
  tested through to the file and back.)
- "Save picture…" and "Save turntable…" through the system's dialog, the progress window and
  Cancel. (Drawing, encoding and writing are tested; the dialog is not.)
- The Enlarge button and Escape in the running window.
- Frame times on the discrete graphics card.
- Left and right against another viewer. The 3D letters come from the same rule as the 2D
  ones, and the tests fix which side the test bead appears on from every direction, but no
  3D picture has been compared with a picture from another program.
