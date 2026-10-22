# Acceptance record for 0.5.0

Date: 22 October 2026. Machine: a Windows 11 desktop with 31.7 GB of memory, a 12th Gen Intel
Core i7-12700K, an NVIDIA GeForce RTX 5070 Ti (16 GB) and Intel UHD Graphics 770. Build:
`--release` (the gates use the debug build), Rust 1.99.0.

This record says what was checked, how, and with what result. Everything labelled here is the
made-up head that the model converter paints, which is nobody's scan: nothing in this record
describes a patient, and no real scan was labelled for it. A real export is tried on the
developer's machine, and nothing from it is recorded. The pictures were made with
`VOXA_CAPTURE` and are kept in the workspace folder, never in this repository. What a person
must try by hand is listed as not checked at the end.

**Linux was neither built nor run.** This machine has no Linux target, and continuous integration
runs on GitHub and was not run for this branch. Everything below is Windows.

## Automated gates

| Gate | Result |
|---|---|
| `cargo xtask check`, formatting | passes |
| clippy, pedantic lints as errors, all targets and features | passes |
| 400 lines to a source file | passes |
| `cargo test --workspace --all-features`, with `VOXA_MODELS` and `VOXA_RUNTIME` unset | 1 276 passed, 0 failed, 16 ignored (the 16 need the model folder and the runtime) |
| `cargo deny check` | licences, bans and sources pass; advisories fail on six notices that are not this release's, below |
| the documentation build, as the gate runs it, warnings as errors | passes |

The gate stops at the first failure, so the documentation build was run as a command of its own
after `cargo deny`. The gate ran in the foreground, in one go, with nothing else building
Voxa, and nothing stalled.

**The six advisories are `main`'s, not this release's.** RUSTSEC-2026-0367, -0368 and -0369 are
against `zip` 8.6.0, which `voxa-dicom` and `voxa-app` use to read ZIP exports. RUSTSEC-2026-0370,
-0371 and -0372 are notices that `zvariant`, `zbus_names` and `zvariant_derive` are no longer
maintained; they come in with `main`'s own `accesskit` feature of the interface library. `main`
alone fails on the same six. This branch changes neither those dependencies nor `deny.toml`. The
gate counts as passed when these six are its only failures, and they were. `main` needs a `zip`
upgrade, which may need code changes in `voxa-dicom`.

`cargo xtask coverage`, with the variables unset, passes. Lines covered, against a floor of 80%:
`voxa-core` 94.4%, `voxa-dicom` 97.1%, `voxa-volume` 99.2%, `voxa-labels` 99.1%, `voxa-models`
99.5% (without `runtime.rs` and `cards.rs`, which need a runtime or a card) and `voxa-segment`
94.9%.

`cargo xtask check-models`, with `VOXA_MODELS` naming the model folder and `VOXA_RUNTIME` the
runtime's, ran 16 of 16 tests and all passed, in 18 minutes. It runs in the debug build and the
machine was busy (below), which is why it took about twice as long as it has. Against SynthSeg's
own answer on the made-up head (3 594 240 voxels; the networks see a cube of 160):

| Model | Device | Voxels that differ from Python | Least Dice | Scores differ by |
|---|---|---|---|---|
| Standard (32 structures) | processor | 0 | 1.0 | 0 |
| Standard | integrated graphics | 0 | 1.0 | 0 |
| Standard | discrete card | 0 | 1.0 | 0 |
| Robust with cortex areas (98 structures) | processor | 0 | 1.0 | 0 |
| Robust with cortex areas | integrated graphics | 25 (99.9993% the same) | 0.99979 | 0.0001 |
| Robust with cortex areas | discrete card | 10 (99.99972% the same) | 0.99979 | 0.0001 |

These are the figures that every earlier run of these tests gave, and they meet the marks: the
standard model on the processor at every voxel, every other pairing at 99.99% or more, a Dice
of 0.999 or more, and scores within 0.001. The whole path through Voxa's own code on the discrete
card (10 voxels differ) and the placing of a left structure on the patient's left of its twin
passed too. Every whole run was held to the bound on the ordinary memory it may ask the system
for, and each was within it (integrated graphics are shown the comparison and not held to it):

| Run | Asked the system for | Bound |
|---|---|---|
| Standard, processor | 3.65 GiB | 4.87 GiB |
| Robust with cortex areas, processor | 3.84 GiB | 5.29 GiB |
| Standard, discrete card | 8.88 GiB | 11.50 GiB |
| Robust with cortex areas, discrete card | 8.89 GiB | 11.50 GiB |
| The whole path, discrete card | 8.88 GiB | 11.47 GiB |

## Written to be run, and run

`write_phantom head CHECK_FOLDER OUT [whole]` writes the converter's made-up head as a study.
A test writes a small made-up check folder, writes the head from it, opens the study as Voxa does
and holds every value and the place of voxels to what the check folder says; it was seen to fail
for three planted faults (the sign of x in the head's place, no air before the head along the
slices, the rescale slope doubled), each for its own reason. `check_labelling EXPORT` labels the
largest series with a 3D view, with no window, on every place Voxa would try, and prints for each
the voxels the networks were given, the place, the seconds, the parts and the model's check.
Both are used below.

## Labelling the made-up head

All with no Python on the path: before each run every folder holding a `python`, `python3` or
`py` program was taken off the `PATH`, and the shell found none.

**The button to the parts, on the discrete card.** The session that the window is a view of was
driven with the same actions the button sends. The line under the button read "Standard model
with cortex areas, on the graphics card (NVIDIA GeForce RTX 5070 Ti). Standard was picked
because the slices are no more than 3 mm apart." Pressing it labelled the head in 23.3 s
(98 parts; the model's check passed with nothing below 0.65). A picture of the real window
(`VOXA_CAPTURE_SHOW=labelled,parts`, which sends the same action) shows the Parts tab with the
model's check first, "Labels made by Voxa with SynthSeg 2.0, standard model, with cortex areas,
on NVIDIA GeForce RTX 5070 Ti, 2026-10-21. Not for diagnostic use.", the table's source, **Save as
CSV…**, **Save as…**, and the list of brain structures and cortex areas with their volumes;
and one with the Parts look shows the solids with the same sentence in the scene. (The day in
the caption is Greenwich's; the machine's clock had passed midnight.)

**The times asked for.** The targets were, from the button to the parts on a 16 GB discrete card
after its first run, under 5 s for the standard model and under 30 s for the robust model with
cortex areas. `check_labelling` on the 160 cube (4 096 000 voxels), each pair of runs a process
apart, gave on the card:

| Model | First process | Second process |
|---|---|---|
| Standard, no cortex areas | 20.9 s | 51.3 s |
| Robust with cortex areas | 33.9 s | 23.5 s |

The robust target was met in its second run; the standard target was not met in any. A run of
the standard model with the stages timed gave 19.4 s and then 16.7 s: the first network 4.7 s
and 3.6 s, the second 3.5 s and 3.5 s, the model's check of its own work 10.7 s and 9.2 s, and
tidying under half a second. **These are not the conditions of the target, and the record cannot
say whether the target can be met.** (Sizes in this record are in GiB, as the system shows
them.) Another program on the computer held 8.4 to 10.2 GiB of the card's 15.9 GiB throughout
(10.5 GiB in use before a run began, by the card's own count). While the run went the card's
total rose to 13.8 GiB and no further, so this process held about 3.4 GiB on the card where its
networks take 4.8 GiB and the check of its own work 8.1 GiB; the rest goes to ordinary memory,
across the bus. That fits a check taking 9 to 11 s where it took 1.5 s on a free card, and the
networks taking 2 to 3.5 times as long as they did on a free card. The nearest
evidence of a free card is the earlier measurement at the time of the findings, in a release
build on the same card: 5.0 s for the standard model and 5.9 s for the robust model with cortex
areas, from the start of preparing to the tidied labels. Whether the standard network written
with the blur as three one-axis convolutions, which replaced the one grouped convolution after
those figures were made, is slower on a card has not been measured on a free card. The
processor, on the same runs, took 31 to 258 s for the standard model and 39 to 220 s for the
robust one, varying with what else the computer was doing: another program's build was running
during the slow ones.

**The made-up whole head.** `write_phantom head ... whole` puts the head in a grid of air of
176 x 256 x 256 voxels, which pads to 192 x 256 x 256 (12 582 912 voxels). With 13.0 GiB of
ordinary memory still free to the system, `check_labelling` was offered one place only: the
scan cut to the head, 192 x 192 x 192 once padded (7 077 888 voxels), on the processor. By the
sums in `docs/models.md` the whole scan does not fit the card (its networks need 20.5 GiB
and the system lets Voxa use 14.9), the whole scan on the processor would ask for up to 16.1 GiB
of ordinary memory, and the cut on the card up to 17.3 GiB, so none of those was offered or
tried. The labelling on the processor took 112 s and gave 98 parts and a passing check. In the
window the result said, in order: the check passed; labels made by Voxa "on the processor";
"The scan was cut to the head to fit. Where two parts meet, some voxels may be labelled
differently than without the cut." So the run did not complete on the card; it went straight to
the first place that fit, and said where and that it cut. It had no card to give way, so there
is no note that the card had no room. With more free memory the whole scan would be offered on
the processor first.

**Closed and opened again, the kept result offered and shown.** A first session, with a
settings folder, labelled the head, and the Parts tab's state said labels made on that day were
kept. It was dropped, and a second session on the same settings folder, opening the same
study, was offered the kept result as soon as the scan's 3D view was built. **Show** brought back
98 parts with the same check and the same caption, and every voxel, and every part's name, side
and volume, was the same as made. **Delete** left nothing kept. This was two sessions of the
code the window uses, not the window closed and opened by hand, because a picture of the window
reads and writes no settings.

**The runtime folder taken away.** With the `runtime` folder renamed away from beside the
program, the window showed the Parts tab with **Label this scan** greyed and, beneath it, "The
runtime is not in the folder given. docs/models.md says what goes where." (with nothing chosen
it now says otherwise: see below). Everything else was
there: the study listed, the 3D view built. A session with the runtime folder named as an empty
folder gave the same sentence, said labelling cannot run, started no run when the button's
action was sent, and still opened a label file made elsewhere (4 parts, with its check shown
as failed, as the made-up phantom's table says), and a picture of the window with the file open
shows the Parts tab as 0.4.0 has it ("Made by another program. Voxa shows these labels as
given."). The gate passes with no runtime and no model folder on the machine, which is how
every test of 0.4.0's behaviour was run.

## Looked at in the window

Pictures at 1440 x 900: the Parts tab before pressing the button; after a run on the 160 cube
head, with the solids, and on the whole-head grid; with the runtime away; and a label file
opened with it away. Things seen that are not faults of this work, or are left as they are:

- The sentence under the button when the runtime is missing says "in the folder given" even when
  no folder was given, as when nothing is beside the program and nothing was chosen. It reads
  as if the person had chosen one. (Changed after this record, in the fix round that followed
  the review of the whole branch: with nothing chosen the sentence says that Voxa found none,
  and to choose the folder under **Options**; the same for the model. It was not looked at in
  the window again.)
- The text under the button, "Labels are parts of the head that another program has outlined
  on this scan...", is the 0.4.0 text and is now half true: Voxa also makes labels. (Changed
  after this record, in the same round: it now says that labels are outlined by Voxa, when it
  labels the scan, or by another program.)
- A picture of the window keeps nothing, so a result in one ends with the notice that it could
  not be kept on this computer and will be gone when Voxa closes. That is the picture, not the
  window.
- In the whole-head picture the scene was in the MIP look and turned from the default front
  view. The window was open for two minutes on a computer in use; nothing in labelling sets
  either.
- The system listed the discrete card twice, under two identities, on this machine, which also
  has a remote display adapter. `check_labelling` tries a card by its name once. The **Runs on**
  menu could not be opened in a picture, so how it lists the card was not seen. (After this
  record the menu offers each name once, the first card of it; that too was not seen.)

## Not checked

These need a person at the window and have not been tried by hand:

- Pressing **Label this scan**, **Stop**, **Show**, **Make again** and **Delete** with the mouse,
  and the stage line while a run goes. (What each does was run through the actions the buttons
  send, and a picture of the window waited for a run, but no button was clicked.)
- **Options**: the model, the cortex areas, **Runs on**, keeping, and the two folders through
  the system's dialog; **Save as…** and **Names…** through the dialog; whether the choices are
  remembered when Voxa is closed by hand.
- Stopping a run in progress on a card, and the 3D view's speed while the card is busy.
- A real driver reset. None was provoked, and the lost-device handling was tested with a
  destroyed device and not otherwise.
- The robust model chosen automatically on a scan with thick slices, in the window.
- The standard and robust times of the targets on a card that no other program holds. The
  integrated graphics, chosen by hand in the window.
- A runtime other than 1.24.4, a Windows without a `DirectML.dll` of its own, and a model
  folder damaged in each way, in the window. (Each way a folder can be wrong is tested without
  a window.)
- Linux and macOS, and the continuous-integration build.
- Labelling a real export. Any such check is made on the developer's machine, and nothing
  from it is recorded.
