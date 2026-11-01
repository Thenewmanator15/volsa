# 6. Voxa runs a labelling model and ships none

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 2026-10-22. Status: accepted. It replaces the fifth point of
[record 0004](0004-labels-own-grid.md), which said that Voxa does not run a labelling model.

## Context

Until 0.4.0 a person who wanted the parts of a brain labelled ran a Python program beside Voxa,
waited, and opened the file it wrote. That is two programs to keep working, several seconds
before the model even starts, and a run on the processor of seconds to minutes. 0.5.0 labels a
brain scan inside Voxa: a person presses **Label this scan** in the Parts tab, and a little
later sees the same list of parts, outlines and solids that a label file gives.

The model is SynthSeg 2.0 (Billot et al., Medical Image Analysis 2023; the robust variant,
Billot et al., PNAS 2023): the standard network, the three robust networks, a network that
divides the cortex into 34 areas a side, and a network that scores the model's own work. Its
code is Apache 2.0. Its trained weights carry no licence statement of their own and are
distributed with FreeSurfer, under FreeSurfer's terms.

A label made by a model is a guess that looks certain, and on a poor scan it is as polished
as on a good one. Four things must not happen, even if everything else works: model weights
in the repository, in a release, or fetched by Voxa; anything from a real scan in the
repository; any network access or telemetry; and Voxa failing to start, or to do anything it
did at 0.4.0, because the runtime or the model files are missing, damaged or of the wrong
version.

## Decision

- **Voxa runs SynthSeg 2.0 and ships none of it.** No network, no `.onnx` or `.h5` file and no
  runtime library is in the repository, in a release or fetched by Voxa. A person makes a
  *model folder* once, outside Voxa, with the converter in `tools/synthseg-to-onnx` (Python
  and TensorFlow; cargo neither builds nor runs it), from SynthSeg's own files obtained under
  the terms they come with. `docs/models.md` says how.
- **The runtime is ONNX Runtime, through the `ort` crate, and is loaded from files, not built
  in.** `ort` is pinned to `=2.0.0-rc.13` (a release candidate, so an exact version) with its
  default features off and `std`, `load-dynamic` and, on Windows, `directml` on. Nothing is
  linked and nothing is fetched when Voxa is built. The library, `onnxruntime.dll`, is loaded
  as Voxa starts, by the first check of whether this computer can label a scan, once that
  check has found a model folder (with none it is not loaded), from a `runtime` folder beside
  the executable, or else from the folder the person names in the Options menu. So a runtime
  that would crash as it loads would now take Voxa down as it starts, where 0.4.0 loaded none.
  A program holds one runtime for as long as it
  lives, so a runtime folder changed later takes effect when Voxa is next started. Microsoft's
  build 1.24.4 with DirectML 1.15.4 is the one measured. `ort` asks the runtime for interface
  version 17, so any runtime from 1.17 on loads; an older one is refused with a sentence.
  Everything `ort` brings is MIT, Apache-2.0 or ISC (`libloading`), so `deny.toml` gained
  nothing.
- **Telemetry is switched off before anything else is asked of the runtime.** Microsoft's
  Windows builds of ONNX Runtime report usage (not images) unless told not to. Voxa makes the
  runtime's settings with telemetry off and commits them as the first thing it does; `ort`
  then switches telemetry off in the runtime as soon as the runtime's environment exists,
  before any session can. If something else had given the runtime its settings first, Voxa
  would refuse to use it, since telemetry could not be known to be off. Voxa itself makes no
  network connection.
- **Cards are named by what they are, and the place is looked up when it is used.** DirectML
  wants an adapter by its number in the system's list, and that list changes its order while a
  program runs (it did, in the middle of one 20-second run, and the run went on quietly on
  the slower integrated graphics). So a card is a `CardId`, the system's own identity for it,
  and its number is looked up inside `Runtime::open`, `budget` and `in_use` at the moment of
  use. A choice kept between runs is kept by the card's name. Integrated graphics, which share
  the computer's memory and were found slower than the processor, are never picked unless the
  person picks them. DirectML's rules are kept in one place: one run at a time for a session,
  steps one after another, no memory pattern.
- **One module may use `unsafe`: `voxa-models/src/cards.rs`.** On Windows the list of
  adapters, what the system lets this program use on each, and how much ordinary memory it can
  still promise are read through DXGI (`CreateDXGIFactory1`, `EnumAdapters1`, `GetDesc1`,
  `QueryVideoMemoryInfo`) and `GlobalMemoryStatusEx`, and every call into them is marked
  unsafe. Each use carries a `SAFETY` comment. The rest of the crate denies `unsafe`, and
  running a network needs none; `voxa-render` remains the other exception. This extends
  [`CONTRIBUTING.md`](../../CONTRIBUTING.md)'s rule.
- **DirectML is frozen, and the runtime sits behind a trait.** Microsoft supports DirectML and
  ships it with Windows, and adds nothing to it; the newest ONNX Runtime found with DirectML
  was 1.24.4 when ONNX Runtime itself had reached 1.31. Everything above `voxa-models` sees a
  network only as the `Network` trait. What would replace DirectML is not decided. The
  candidates, and why none is used now: ONNX Runtime's WebGPU provider (about ten times slower
  here, and it twice reset the graphics driver; Voxa never uses it on this machine); CUDA (no
  faster than DirectML, needs about 2 GB of NVIDIA's libraries, and only NVIDIA cards);
  Windows ML (not tried); a runtime written in Rust on `wgpu` (cannot run this model today).
  Replacing DirectML means another implementation of `Runtime` and no change above it.
- **Voxa checks a model folder before it trusts it.** The format number is one it knows, the
  kind is `synthseg-2.0`, every file is there with the size and SHA-256 `model.json` lists,
  and the lists have the shapes that kind needs. The SHA-256 shows a file is the one the list
  describes, not who made the folder. Voxa knows how to run SynthSeg; the folder carries
  lists, never instructions.
- **Every result carries its check and its history, and a change of plan is never silent.**
  The sentence at the top is the model's verdict from its own scores, failed when any group
  scores below 0.65, and a failed check is said first. The caption says Voxa made the labels,
  with which model, on which device, on what day, and "Not for diagnostic use." A Notes line
  says when the slices are more than 3 mm apart (a rough map), when the run fell to the
  processor because a card had no room or stopped answering, when the scores network ran on
  the processor, and when the scan was cut to the head.
- **A run is bounded in memory and belongs to its series.** Before a run Voxa works out what
  it asks of each place (see the findings) and goes only where the system can still promise
  it. One run goes at a time. A run is stopped when the scene moves to another series, or
  another export is opened, and its result is dropped. A card is never allowed to spill into
  ordinary memory by plan. On the processor the runtime's arena and memory pattern are off.
- **A result is kept on this computer unless the person switches that off.** It is written as
  a label file and its table, in a `labels` folder under the settings folder, named by a hash
  of the series, so that nothing kept says whose scan it was and no path or file name of a
  label file goes into a log. One result is kept for a series; **Delete** removes it, and
  **Save as** writes the files wherever the person chooses. What was chosen for labelling is
  kept in `choices.json` beside them.
- **Labels from files and labels from a run are told apart wherever they are drawn.** A label
  file made by another program is still "made elsewhere" and is still shown with what its
  own table says. Record 0004's other points stand: the labels keep their own grid, they are
  never resampled, and a part's volume is its voxels counted.

## What Voxa does differently from SynthSeg's own program

The preparing and tidying steps are SynthSeg's, step for step, and each is held against arrays
SynthSeg's own Python made. Three things differ, and the result says where they apply:

- Volumes are counted voxels, not added probabilities.
- The scan may be cut to the head when memory is short, and never otherwise. Where the head
  lies in the networks' box moves about one labelled voxel in a hundred, along the borders
  between structures everywhere in the head. SynthSeg's own program does the same when the
  head lies elsewhere in its box, so neither answer is the truer.
- The cortex network's answer is reduced to areas inside the file: the same labels, and no
  table of soft volumes for the areas.

## Findings

Every figure below is of a made-up volume, never of a scan. The made-up head is painted from
a label map that comes with SynthSeg, 144 x 160 x 156 voxels of 1 mm (3 594 240 voxels), which
the networks see padded to a cube of 160 (4 096 000). The machine was a Windows 11 desktop
with a 16 GB discrete card and integrated graphics.

**Agreement with SynthSeg's own program on the same array.** The marks: the standard model on
the processor gives its label at every voxel; any other pairing of model and device, at least
99.99% of voxels the same, a Dice of at least 0.999 for every structure, and scores within
0.001. Voxa's own code (preparing, the networks, tidying) meets them, and equals the
converter's own check to the voxel:

| Model | Device | Voxels that differ from Python | Least Dice |
|---|---|---|---|
| Standard | processor, integrated graphics, discrete card | 0 | 1.0 |
| Robust with cortex areas | processor | 0 | 1.0 |
| Robust with cortex areas | integrated graphics | 25 (99.9993% the same) | 0.99979 |
| Robust with cortex areas | discrete card | 10 (99.99972% the same) | 0.99979 |

The scores agree to 0.0001 at the worst. Preparing a volume gives Python's image to the last
bit when no resampling is needed, and within one step of a 32-bit float (0.00000006) when it
is. Each test was also seen to fail for a planted reason: a voxel changed, a structure renamed,
a score moved, the volume mirrored.

**Two habits of SynthSeg's code that Voxa keeps.** The 1 mm grid is sometimes one voxel longer
than the spacing times the count, because the count is that of a floating-point range (about
one size in twenty). Which axis is right, front or top is judged from the transform back from
the patient, the largest number in each column. SynthSeg's own program stops on a stack of 10
or fewer slices, and on one that is 10 voxels or fewer from bottom to top at 1 mm; Voxa
refuses those, with a sentence. The localiser series of an ordinary export are in this range.

**Time.** On the discrete card, in a release build, from the start of preparing to the
tidied labels: the standard model 5.0 s and the robust model with cortex areas and scores
5.9 s (opening a network takes 0.4 to 0.9 s); on the processor 25.3 s and 36.8 s; on
integrated graphics, in a debug build, 84 s and 145 s. The times on the card were measured
before the standard network's blur was written as three one-axis convolutions (below), and
were not measured again on a free card after. SynthSeg's own Python took 13 s and
16 s on the processor, after about 8 s of starting TensorFlow. Times on this machine
wandered by several times while other programs ran, and the acceptance record
(`docs/acceptance-0.5.0.md`) gives what was measured for this release.

**Memory is what limits a run.** On a card every network but the scores network takes about
1 400 bytes of the card for each voxel of its grid (the most measured was 1 330); the scores
network takes 8.1 GiB whatever the scan, because its box is always 224 cubed. On the
processor, once the runtime's arena is off, they ask the system for about 700 bytes a voxel
(900 with the cortex network) and the scores network 3.1 GiB. A network on a card also asks
the system for as much ordinary memory as it holds on the card and a little more, because
Windows sets ordinary memory aside behind what a program puts on a card: 1 900 bytes a voxel,
and 8.4 GiB for the scores network. Voxa itself holds about 150 bytes a voxel beside a
network (the chances it has found, with, for a moment, the answer that has just arrived,
which is blended into them, and small lists). A run is therefore held to what it asks the
system for at its peak: 200 bytes a voxel for what Voxa holds, and the larger of the
labelling networks' ask and the scores network's, with a quarter added; a run starts only
where the system can still promise that, and for a card the scores network is tried on the
card and then on the processor. A test on the real networks holds every whole run to this
bound. Measured: the whole standard run 3.65 GiB on the processor and 8.89 GiB behind a
card; the robust run with cortex areas 3.83 GiB and 8.88 GiB. Nothing is held at once that
is more than a network and two sets of chances, the second only while it is blended into
the first: each network is let go before the next is opened.

**The processor first asked for far more.** With the runtime's own habits (it keeps what it
has asked for, asks for more than it needs each time it grows, and after a first run asks
for one block for the whole of the second), the standard network on the processor asked the
system for 38.3 GiB, and one run failed with the runtime's `bad allocation` at the node that
blurs the answer. Two things were changed. The runtime's arena and its memory pattern are
off on the processor (no slower; 38.3 GiB fell to 17.1 and the robust model to under half).
And the converter writes the blur, which is a 3 x 3 x 3 Gaussian kernel, as three one-axis
convolutions instead of one grouped convolution over the 33 structures, which asked for
12 GiB by itself: the network's ask fell from 14.70 to 2.64 GiB, no slower on the processor
(6.6 and 6.5 s against 6.7 and 6.6 s; it was not timed on a card), and the labels
are still Python's at every voxel. A variant with no convolution at all was as good on the
processor and cost a card 4 GiB more.

**DirectML hands back no more than about 2 GiB at once, and `ort`'s `Tensor::to` must not
be used with it.** The probabilities of 33 structures for a 256 cube are 2.2 GB. `Tensor::to`
copies by opening a second session on a device of its own, and a tensor that lives on the
first session's device cannot go in there: the program dies with an access violation and
nothing to catch. So an ordinary run fetches every output, and when together they would pass
2 GiB each output is fetched by a run of its own (the same answer; the last network takes
twice as long, which arises only for grids over about 16 million voxels). Networks between
stages give a class or an area for each voxel as one byte, worked out inside the file, so
nothing large comes back from a card. Half precision halves the memory and changes the
labels badly.

**Windows' own `DirectML.dll` was enough** on the Windows 11 tried: with the file taken out of
the runtime folder, the card found Windows' own copy (1.15.5, against the 1.15.4 that comes
with the runtime) and gave the same answer to the last digit, in the same time and memory. An
older Windows was not tried.

**The cut to the head.** A cut changes two things the networks are given: the brightness,
scaled by the cut's own percentiles, and where the head lies in their box. Where the head
lies is what changes the labels. On the made-up head laid 8 voxels otherwise in its box,
about one labelled voxel in a hundred came out otherwise (0.87% with the standard model, 1.1%
with cortex areas), every one at a border between two structures or next to one, and the
small structures and thin cortex areas move most (a hippocampus to a Dice of 0.96, one cortex
area to 0.86). SynthSeg's own Python gave 16 781 other labels for the same head with 16
voxels of air beside it (standard, cortex areas), and Voxa's answer was Python's at every
voxel. The brightness alone moves 0.04% to 0.14%. On a head three quarters the size, on which
the model's own check fails whole and cut alike, 6.8% and 11.0% differ. This is why a cut is
used only when the whole scan does not fit, and why the result says that it was cut.

**A lost graphics device cannot be recovered by the interface library.** `eframe` makes the
window's device once and keeps it, and `wgpu` panics on whatever is made on a lost device.
Voxa watches its device from the start: after a loss nothing panics, the work in hand goes
on, and the person is told once, in the window and in a message box of the system, to close
Voxa and open it again. Annotations, saved views and a kept result are written by the
session's workers and read from files, so none of them depends on the card, and nothing a
person made is lost. A real driver reset was never provoked.

**Other things found.** The system's list of cards changed its order, and listed the same
card twice under two identities; **Runs on** offers each name once, the first card of it, as
a choice kept by name is found again. The runtime's own words for memory it could not have are
sorted as "no room" so that a run falls to the next place. A run on a card finishes what it
was handed before it stops; on the processor it stops between two steps.

## Consequences

- Voxa is still one executable that needs nothing else for everything it did at 0.4.0.
  Without the runtime or the model folder, the button is greyed and says what is missing and
  where to put it; nothing else changes.
- A person must get SynthSeg's files under their terms and run the converter once. The weights'
  licence is not Voxa's to settle, and Voxa does not pretend that it is.
- Voxa depends on a release candidate of `ort`, pinned exactly, and on DirectML, which will
  not improve. Both sit in `voxa-models` behind a trait, so either can be replaced.
- `unsafe` is allowed in a second module, small and commented, and nowhere else outside
  `voxa-render`.
- Linux and macOS use the processor only, and have not been built or run: this machine has no
  Linux target, and the continuous-integration build runs on the private repository's remote.
- The scores network needs about 8 GiB of a card or 3 GiB of ordinary memory whatever the
  scan, so even a small scan needs a computer that has that. Where it has not, the run is
  found not to fit when it starts, after the button is pressed, and a notice says what the
  run needs and what the computer has.
