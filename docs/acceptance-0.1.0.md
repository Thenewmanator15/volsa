# Acceptance record for 0.1.0

Date: 9 October 2026. Machine: a Windows 11 desktop with a discrete graphics card. Build:
`--release`, Rust 1.99.0.

This record says what was checked, how, and with what result. Nothing in it describes a patient:
a real export was also checked, on the machine, and nothing from it is recorded here. A target
that was missed or not measured is listed as such.

## Automated gates

| Gate | Result |
|---|---|
| `cargo xtask check` (formatting, lints as errors, 400-line limit, tests, licences and advisories, documentation) | passes |
| Tests in the workspace | 240, all passing |
| `cargo xtask coverage` (line coverage, floor 80%) | `voxa-core` 91.4%, `voxa-dicom` 98.4%, `voxa-volume` 100% |

## Benchmarks

`cargo bench -p voxa-app`, median of 20 samples.

| What | Time |
|---|---|
| Index a zip of 200 slices (64 x 64), from opening the file | 8.3 ms |
| Decode one uncompressed 512 x 512 16-bit slice | 1.3 ms |
| Choose the automatic window for 512 x 512 values | 1.2 ms |

## A real export

A real export was also opened, locally: every study and series was listed, every slice of
every series was decoded, and no patient name or identifier was shown in the window.
Nothing from that export is recorded here.

## Not checked

These need a person at the machine, and are for the owner to do:

- Dropping the export's zip on the window, and the Folder and File dialogs. All three end in
  the same action the tests drive.
- Frame rate while scrolling a 64-slice series (target 60 a second). Not measured.
- That the left and right letters agree with the vendor's viewer for one axial slice. The
  letters come from the orientation written in each file; no second viewer was used to confirm
  them.
- Dragging, the wheel, the keys and playback in the running window. The functions that turn
  them into actions are tested; the few lines that read the mouse and the clock are not.

## Known issues

- JPEG and JPEG 2000 decoding have no automated test, because no encoder is among the
  development dependencies. Neither is covered by anything recorded here.
- A file that holds a whole series in many frames is read again for each request for its
  frames. It works, and is slow for a large file.
- The automatic window fails (collapses to a width of 1) on an image with a value more than
  about a thousand times the spread of the rest.
- The toolbar wraps onto a second line in a window narrower than about 1440 points.
- On Windows a console window opens beside Voxa when it is started from Explorer.
- The interface uses the typefaces that come with its library, and words where an icon set is
  planned.
