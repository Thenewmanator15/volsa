# Acceptance: reading more kinds of MRI export

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 2026-10-14. Made on the branch `any-mri`, and released in 0.5.0 with labelling (see
[acceptance-0.5.0.md](acceptance-0.5.0.md)). The design is in
[superpowers/specs/2026-10-13-voxa-any-mri-design.md](superpowers/specs/2026-10-13-voxa-any-mri-design.md)
and the plan in
[superpowers/plans/2026-10-13-voxa-any-mri.md](superpowers/plans/2026-10-13-voxa-any-mri.md).

Voxa is not a medical device and is not for diagnostic use.

## Gates

- `cargo xtask check` passed: formatting, Clippy with pedantic lints as errors, file and
  function lengths, 867 tests with none failing, `cargo deny` and the documentation. The
  drawing crate's tests ran with the rest, on every thread, on a discrete card
  (`WGPU_POWER_PREF=high`); before this work they stopped for good on that machine unless
  limited to four threads.
- `cargo xtask coverage` passed, lines covered: `voxa-core` 94.4%, `voxa-dicom` 97.1%,
  `voxa-volume` 99.2%, `voxa-labels` 98.4%, against a floor of 80%.

## Looked at

Pictures of the window on the made-up study that `write_phantom` writes, taken with
`VOXA_CAPTURE` at 1280 x 800 and 1440 x 900 points:

- the series list with a card of each kind: the two series of the ball, a series of two
  volumes, three locating images and a picture in colour;
- the second volume of the set, with the buttons to step between volumes in the toolbar and
  the line naming the volume in the viewport;
- the picture in colour, in its own colours, with the 3D panel saying it has no 3D view.

Looking found five faults that no test had, each fixed with a test that failed first:

1. The card of the locating images named a plane, the first image's, though they lie in three.
2. In a narrow viewport the line naming the volume ran into the scale bar at the bottom. It is
   now under the series' name, at the top.
3. A picture in colour gave a window and level it is not shown through.
4. A picture in colour with no thickness in its file gave one of 1 mm, a stand-in for an
   unknown value. Its card now counts pictures and gives no thickness.
5. Where the toolbar wraps, the buttons either side of the volume's name were split across two
   rows; grouped, they ran off the edge of the window. They now go onto the next row together.

## A real export

A real export was opened once on the developer's own machine, and a picture of the window
was looked at and kept out of the repository. It was indexed and shown with no fault. Nothing
from it is recorded here or anywhere in the repository, and no test, document or commit
message describes it.

## Not checked

- A screen reader. What the interface tells one is tested; no screen reader was used.
- The window's icon and the message window of the release program for Windows were not
  looked at. That the release program opens no console was checked in the program file.
- Diffusion, dynamic, cardiac and repeated series were tried only as made-up series in the
  tests; no real export of those kinds was opened.
- Pictures in colour stored as brightness and colour differences, or compressed, were tried
  only in the tests.
- The log and the preferences were tested, not looked at after a real session.
- Integrated graphics: the machine's integrated card was not reachable in the session used.
- Linux and macOS.
