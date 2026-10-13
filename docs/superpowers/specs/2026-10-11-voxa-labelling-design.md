# Voxa: labelling a scan inside the app

Date: 2026-10-11
Status: approved by the owner on 2026-10-11

This is the specification for the second half of milestone 4 of
[the Voxa design](2026-10-05-voxa-design.md): running a labelling model inside Voxa. The first
half, showing labels made elsewhere, shipped as 0.4.0. This half ships as 0.5.0.

**Voxa is not a medical device and is not for diagnostic use.** Nothing here changes that. A
label made by a model is a guess that looks certain; this design keeps saying so on the result.

The owner asked for this design to be written from defaults instead of through questions.
Every choice below is therefore marked: **(owner)** where the owner said it, **(default)**
where it is the writer's call and the owner has not been asked. The defaults that most
deserve a look are gathered in section 12.

## 1. What it is for

Today a person who wants the parts of a brain labelled runs a Python program beside Voxa,
waits, and opens the file it wrote. That works, and has three costs: a second program to keep
working, a wait of several seconds before the model even starts, and a run on the processor
that takes from seconds to minutes depending on what else the machine is doing.

After this milestone a person opens a scan in Voxa, presses **Label this scan** in the Parts
tab, and a little later sees the same list of parts, outlines and solids that 0.4.0 shows for
a label file, with the model's own check of its work said in a sentence at the top. No Python
runs. On a capable graphics card the wait is seconds.

The model is SynthSeg 2.0 (Billot et al., Medical Image Analysis 2023; the robust variant,
Billot et al., PNAS 2023), which labels about thirty brain structures on a scan of any
contrast, and with a second network divides the cerebral cortex into 34 areas a side.

## 2. What the feasibility check found

A check was run on 2026-10-10, outside this repository, with made-up volumes painted from a
sample label map that comes with SynthSeg. Its notes are kept outside the repository. What it
established:

- Every SynthSeg 2.0 network converts to ONNX (tf2onnx 1.17, opset 17) and gives the Python
  result. On a made-up brain of 3.59 million voxels the standard model gave the same label at
  every voxel on the processor, on a discrete card and on integrated graphics. The robust
  chain with cortex areas differed at 0 voxels on the processor, 10 on the discrete card and
  25 on integrated graphics, with Dice never below 0.9997 for any structure. The model's
  scores of its own work differed by at most 0.00005.
- SynthSeg's whole graph does not convert as its own code builds it, because one layer picks
  an axis at run time. Each network converts on its own; the small steps between networks have
  to be written in Voxa.
- ONNX Runtime with DirectML ran on both graphics adapters of the test machine. On a 160 cube:
  about 1.5 s for the standard model and 11 s for the robust chain with cortex areas and
  scores on a 16 GB discrete card; 30 s and 112 s on integrated graphics; 5 s to 83 s and
  17 s to 174 s for Python on the processor, depending on how busy the machine was.
- Memory is the limit. DirectML needed about 1.4 KB for each voxel at full precision (5.8 GB
  for the 160 cube). A 256 cube overflowed the 16 GB card into ordinary memory: the standard
  model still finished, in 66 s, and the robust chain ran out at the cortex-area network.
- DirectML cannot hand back more than about 2 GB from one run. The probabilities of 33
  structures for a 256 cube are 2.2 GB, and of 69 cortex areas 4.6 GB.
- Half precision halves the memory and changes the labels badly, because some of the model's
  numbers do not fit. It is not a shortcut.
- On integrated graphics the model was slower than on the processor.
- The first adapter DirectML offers is not always the fast one. On the test machine it was
  the integrated graphics.
- CUDA was no faster than DirectML and needs about 2 GB of NVIDIA's libraries. ONNX
  Runtime's WebGPU provider was about ten times slower and twice made the graphics driver
  reset. A pure-Rust runtime on wgpu cannot run this model today.
- Python with TensorFlow needs about 8 s before the first network can start. ONNX Runtime
  opens the same network on the processor in under half a second, and on a card in one or
  two.

What the check did not do: build anything in Rust, or redo SynthSeg's preparing and tidying
steps outside Python. Section 10 puts those first.

## 3. Goals

1. **Label a brain scan from inside Voxa** with SynthSeg 2.0: the standard model, the robust
   chain, the cortex areas, and the model's scores of its own work.
2. **No Python at run time. (owner)** Python is used once, outside Voxa, to turn SynthSeg's
   files into the files Voxa reads.
3. **The same answer as SynthSeg's own program**, shown by a test, on any device Voxa runs
   the model on.
4. **Any graphics card that can hold the run, and the processor when none can. (owner:
   ONNX Runtime with DirectML as the route for every vendor's card.)**
5. **Every result carries its check and its history**: the model's verdict in plain words,
   which model made it, on what device, and anything Voxa did that SynthSeg's own program
   would not have (section 7).
6. **Voxa without the labelling files is Voxa as it is now.** Nothing else depends on them.

## 4. Not goals, and things that must not happen

Not in this milestone:

- Any other model: the inner ear (milestone 5), the predicted 1 mm scan, click to outline.
- The parts outside the brain (eyes, bone, air spaces). They need a method of their own.
- A brain-only view of the scan made from the labels.
- Fetching model files or the runtime over a network from inside Voxa.
- CUDA, the WebGPU provider, Windows ML, half precision, working in tiles.
- Graphics cards on Linux or macOS. There the processor is used.
- Correcting labels by hand; more than one label set at once.

Must not happen, even if everything else works:

- **Model weights in the repository, in a release, or fetched by Voxa. (owner)** SynthSeg's
  weights have no settled licence of their own.
- **Anything from a real scan in the repository. (owner)** Tests, documents and commit
  messages use made-up volumes and invented values.
- **A result that looks surer than it is.** A failed check is said first, on the result.
- **A silent change of plan.** If Voxa runs on the processor because the card is too small,
  or cuts the scan to the head to make it fit, the result says so.
- **Any network access or telemetry** from Voxa or from the runtime it loads.
- **Voxa failing to start, or to do anything it does today, because the runtime or the model
  files are missing, damaged or of the wrong version.**

## 5. What a person sees

**Before.** The Parts tab, for a study with no labels open, shows what it shows now and adds
a **Label this scan** button. Beneath it one line says what will happen: which model, on
which device, for example "Robust model with cortex areas, on the graphics card". A small
menu beside the button chooses:

- the model: **Automatic**, **Standard** or **Robust**. Automatic picks Robust when the
  series' slices are more than 3 mm apart, otherwise Standard, and says which it picked and
  why. **(default; the 3 mm rule is the one the owner's pipeline uses)**
- **Cortex areas**: on or off. On by default. **(default)**

If labelling cannot run, the button is greyed and the line says why and what to do, in a
sentence: the runtime files are not there, the model files are not there or do not match
their list, the series is not a volume, or the run would not fit in this computer's memory.

**During.** The button becomes a progress line that names the stage in plain words
("Preparing the scan", "Finding the structures, 2 of 3", "Dividing the cortex", "The model is
checking its work", "Tidying up") with **Cancel**. Everything else in Voxa keeps working.
The 3D view may draw more slowly while the card is busy, and the line says so.

**After.** The result becomes the study's label set, exactly as if a label file had been
opened: the list, the outlines over the planes, the Parts look, volumes, left against right.
Three things differ from a file made elsewhere:

- The sentence at the top is the model's verdict, in the words 0.4.0 already uses, from
  SynthSeg's own scores with a fail mark of 0.65.
- The caption says the labels were made by Voxa, with which model, on which device, when,
  and "not for diagnostic use". It no longer says "made elsewhere".
- A **Notes** line lists anything a reader should know: "The slices of this scan are 6 mm
  apart, so this is a rough map", "Run on the processor: the graphics card did not have
  room", "The scan was cut to the head to fit. Where two parts
  meet, some voxels may be labelled differently than without the cut."

**Later.** The result is kept (section 8). When the same series is opened again the Parts
tab says labels made on such a date are kept for it, and offers **Show**, **Make again** and
**Delete**. **Save as** writes the label file and its table of names wherever the person
chooses, for other programs.

**Three settings**, behind the same menu, since Voxa has no settings screen yet: where the
model files are, where the runtime files are, and **Runs on**: Automatic, each graphics
adapter by name, or Processor. They are kept in a file in the settings folder with a format
number, like everything else Voxa keeps.

## 6. How it is made

### 6.1 The runtime

- **ONNX Runtime, through the `ort` crate, with DirectML on Windows and the processor
  everywhere. (owner for ONNX Runtime and DirectML; default for the crate.)** `ort` is
  MIT or Apache-2.0. ONNX Runtime is MIT.
- **The runtime is loaded when first needed, from files beside Voxa, and is not built into
  the executable. (default)** `ort` is used with its `load-dynamic` feature and without its
  default features, so the build fetches nothing from outside crates.io and `cargo deny`
  sees every dependency. Voxa looks for `onnxruntime.dll` and `DirectML.dll` in a `runtime`
  folder beside the executable, then in the folder named in Settings. Reasons: Voxa stays
  one executable that needs nothing else for everything it does today; Microsoft's own
  build with DirectML, the one the check measured, can be used as it is; and the runtime can
  be replaced without a new Voxa.
- **Which build.** The newest build of ONNX Runtime with DirectML that the check could find
  was 1.24.4, with DirectML 1.15.4, while ONNX Runtime itself had reached 1.31. DirectML is
  frozen: supported, shipped with Windows, given no new features. `ort` 2.0.0-rc.13 is made
  for ONNX Runtime 1.28 and its notes say it skipped four versions, which puts rc.12 at
  1.24; the first task confirms that. The builder pins whichever release loads the 1.24.4
  build, with an exact version, since `ort` 2.0 is still a release candidate. **(default)**
- **The build needs no runtime.** Voxa compiles and passes its ordinary tests on Windows and
  Linux with no ONNX Runtime anywhere on the machine.
- **`DirectML.dll`** is under Microsoft's own licence: it may be distributed inside an
  application for use on Windows, it is not open source, and its notices must stay. It is
  never committed. A release may carry it beside the executable with its licence text; the
  README says so. Windows 11 has a copy of its own that may be new enough; the first task
  finds out, and if it is, nothing needs to be carried.
- **Telemetry off.** Microsoft's Windows builds of ONNX Runtime report usage (not images)
  unless told not to. Voxa tells it not to before anything else is asked of it, and a
  decision record says so. **(default: Microsoft's build with the switch off, not a build of
  our own.)**
- **DirectML's rules** are kept in one place: one run at a time for a session, no memory
  pattern, sequential execution, and the adapter named, never left to DirectML's first
  choice.
- **The runtime sits behind a small trait**, so the arithmetic around the networks is tested
  with a stand-in and no runtime, and so another runtime can replace this one. DirectML
  being frozen makes that likely within a few years.

### 6.2 The model files and the converter

- **Voxa ships no model and fetches none. (owner)** A person makes the files once and tells
  Voxa where they are.
- **A model is a folder**: the networks as `.onnx` files and one `model.json`. The JSON
  holds: a format number; the kind of model (`synthseg-2.0`); for each file its size and
  SHA-256; the lists that go with the networks (the structures and their names, which are
  left and right of each other, the classes the tidying step uses, the cortex areas, the
  groups the scores are given for); the versions of the tools that made it; the SHA-256 of
  the SynthSeg files it was made from; and the result of the converter's own check against
  Python. It holds no weights and nothing about any scan. **(default)**
- **Voxa checks before it trusts.** The format number is one it knows, the kind is one it
  knows how to run, every file is there with the size and SHA-256 listed, and the lists have
  the shapes that kind needs. Anything else is refused with a sentence that says which
  check failed. The SHA-256 shows a file is the one the list describes, not who made it; the
  documents say so.
- **Voxa knows how to run SynthSeg; the folder does not tell it how.** The steps of
  section 6.4 are Rust code in Voxa. The JSON carries lists, never instructions.
- **The converter is a Python script kept in this repository under `tools/`, with its own
  README. (default)** It is not built or run by cargo and is no part of Voxa. Kept here, it
  stays in step with the format Voxa reads. It takes a SynthSeg checkout and its `.h5` files
  and writes a model folder. It:
  1. converts each network with tf2onnx;
  2. tidies each graph: each upsampling step becomes one `Resize`, where tf2onnx writes
     three rounds of work on six-dimensional tensors;
  3. makes each network return only what Voxa needs (section 6.5);
  4. runs SynthSeg's own program and the converted networks on a made-up volume and refuses
     to write `model.json` unless they agree to the marks in section 9;
  5. writes, beside the model, the made-up volume and what Python made of it at each step,
     for Voxa's own test to use.
- Getting SynthSeg's `.h5` files is the person's own step, under the terms they come with.
  A page in `docs/` says where they come from and how to run the converter.

### 6.3 The crates

As the design's section A3 foresaw, two new crates:

| Crate | Owns | Depends on |
|---|---|---|
| `voxa-models` | The trait a network is run through; ONNX Runtime behind it; finding and checking a model folder; the list of devices and what each can hold. No image mathematics. | `voxa-core` |
| `voxa-segment` | Everything SynthSeg does around its networks: preparing a volume, the steps between networks, tidying the result, the verdict; and writing a label file. | `voxa-core`, `voxa-volume`, `voxa-labels`, `voxa-models` |

`voxa-app` gains the button, the progress line, the settings, one worker and the actions and
effects for them. `voxa-render` does not change. Dependencies still point downward only.
`voxa-segment` joins the crates held to 80% line coverage; the part of `voxa-models` that
can run without a runtime does too.

`voxa-labels` gains a writer for the label file and table it already reads.

### 6.4 What a run does

The series in the 3D scene is the one labelled: its volume is already in memory. The steps
are SynthSeg's own, in its order, and each is tested against what SynthSeg's Python made.

1. **Prepare.** Resample to 1 mm along the volume's own axes if any spacing is more than
   0.05 mm from 1 mm. Turn the axes, without resampling, to the order and direction the
   model was trained on. Scale brightness so the 0.5th percentile is 0 and the 99.5th is 1,
   and clip. Pad with zeros so each side is a multiple of 32 and at least 128.
2. **Run the networks.**
   - Standard: the network once as it is and once mirrored left to right; the mirrored
     answer is turned back, its left and right structures swapped, and the two averaged.
     (The network's own small blur of its answer is inside the converted file.)
   - Robust: three networks in a row; between them, the most probable class at each voxel
     is what the next network is given.
   - Cortex areas: a fourth network, given the scan and where the cortex was found.
   - Scores: a fifth network, given the structures found, in a 224 cube centred on them.
3. **Tidy**, as SynthSeg does: keep the largest connected piece of everything labelled;
   within each of the model's classes of structures keep the largest piece; choose the most
   probable structure at each voxel; put the cortex areas where the cortex is.
   For the robust model SynthSeg's own program takes a shorter way, and so does Voxa: the
   largest piece of everything, then a structure's chance of 0.2 or less counts for
   nothing, and no largest piece within a class.
4. **Make the label set.** The labels are on the 1 mm grid of step 1, turned back to the
   volume's own axes, with their own place in the patient. They are not resampled onto the
   scan, as decision record 0004 requires. Names and sides come from the model folder. The
   groups are "Brain structures" and "Cortex areas". Colours are Voxa's own, by number,
   unless the folder gives them. Every part's trust is "model". The verdict comes from the
   scores: failed if any group scores below 0.65, with the groups that did named.
   **(default for groups and colours)**

A part's volume is its voxels counted, times the size of one, as for any label set in Voxa.
SynthSeg's own table instead adds up probabilities, so the two differ a little; the
documents say so. **(default)**

One run at a time. A run belongs to the series it was started on; if that series is closed
or another export is opened, the run is stopped and its result dropped.

### 6.5 What each network returns

So that nothing large is brought back from the card: **(default)**

- The networks between the robust stages return the class at each voxel, one byte, worked
  out inside the file. Voxa never sees their probabilities.
- The cortex network returns the area at each voxel, one byte. By SynthSeg's own code that
  is all its tidying step uses of it for the labels; task 3 of section 10 confirms it.
- The scores network is given classes, one byte a voxel, and spreads them inside the file.
- The last network of the standard and robust models returns the probabilities of the 33
  structures, which the tidying step needs. When that would pass 2 GB it is fetched in two
  halves.

The builder may join networks into one file where that runs as well (the three robust
networks did convert as one).

### 6.6 Memory, and which device

**(default, all of it)**

- **Before a run Voxa works out what it needs** from the padded grid: so many bytes for each
  voxel for each network, on a card and on the processor. The figures are measured on
  made-up volumes during the build and kept in the code with a quarter added for safety.
  The check's first figures: 1.4 KB a voxel on a card, 2 KB on the processor.
  *(Amended after the first real runs: about 0.7 KB a voxel on the processor, once the
  runtime's arena is off and the standard network's blur is three one-axis convolutions;
  and every run, on a card too, is held to what it asks the system for in ordinary
  memory, which a test on the real networks checks. The plan's Conventions have the
  figures.)*
- **It knows what each device has**: a card's own memory, less what Voxa itself is holding
  there for the scan and its labels; the computer's free memory.
- **Automatic** tries, in order:
  1. the whole grid on the discrete card, if it fits in the card's own memory;
  2. the whole grid on the processor, if it fits in free memory;
  3. the grid cut to the head on the discrete card, then on the processor;
  4. nothing: a sentence saying what the run needs and what the computer has.
- **Cut to the head** means the smallest box holding everything brighter than the air
  around the head, with a margin, padded as in step 1 of section 6.4. It is used only when
  the whole grid does not fit, because it is not what SynthSeg's own program does and the
  labels at the edge of the box may differ. The first tasks measure by how much; the Notes
  line says it was done.
- **Integrated graphics are never picked automatically**, since the check found them slower
  than the processor. A person can pick them in Settings, where the line beside them says
  so.
- **A card is never allowed to spill into ordinary memory by plan.** It works, and is ten or
  more times slower than fitting.
- **If a run fails on a card** for want of memory or because the device was lost, Voxa
  tries the next rung once and the Notes line says so. If the processor fails too, the
  error is shown.
- **Finding the adapter.** *(Amended: the system's list changes its order while a program
  runs, so Voxa knows a card by the system's identity for it and looks its number up each
  time a network is opened.)* DirectML wants the adapter by its number in the system's own
  list, and Voxa needs each adapter's memory. How Voxa learns both on Windows is the
  builder's choice. If it needs `unsafe`, that goes in one small module of `voxa-models`
  with a comment on each use, and a decision record makes the exception, as the design
  already does for `voxa-render`.

### 6.7 What is kept

**(default)**

- A result is written as a label file and its table of names, the two files 0.4.0 already
  reads, in a `labels` folder under the settings folder, named by the hash of the series
  that views and annotations are already filed under. The table records the model, its
  SHA-256, the device, the date, the verdict and the notes.
- Nothing kept says whose scan it was. No path or file name goes into a log.
- One result is kept for a series; making it again replaces it. **Delete** removes it.
- A setting turns keeping off, for a person who wants nothing derived from a scan left on
  the computer.

## 7. What Voxa does differently from SynthSeg's own program

Said here once, and on the result where it applies:

- Volumes are counted voxels, not added probabilities.
- The scan may be cut to the head when memory is short (never otherwise). Measured on a
  made-up head: where the head lies in the networks' box moves about one labelled voxel in
  a hundred, along the borders between structures everywhere in the head. SynthSeg's own
  program does the same when the head lies elsewhere in its box, so neither answer is the
  truer; the result says that it was cut.
- The cortex network's answer is reduced to areas inside the file, which gives the same
  labels and no table of soft volumes.
- Nothing else. In particular the preparing and tidying steps are SynthSeg's, step for step.

## 8. When things go wrong

Every one of these is a plain sentence where the person is looking, never only a log line:

- The runtime files are not found, are not the build Voxa was made for, or will not load.
- The model folder is not found, is of a kind or format Voxa does not know, lacks a file,
  or holds a file whose size or SHA-256 is not the one listed.
- The series is not a volume, has too few slices to resample, or holds no values that
  differ.
- The run does not fit on any device, with the figures.
- A network failed. The runtime's own message goes to the log, without paths.
- The run was cancelled, or its series went away.
- The result could not be kept (the folder cannot be written). The result is still shown.
- The model's check failed: shown first, in the words Voxa already uses.

Nothing a model folder, a runtime file or a network's answer contains may stop the program.
A panic inside the runtime's worker becomes an error for that run.

A driver reset while a run is on takes Voxa's own drawing with it. What Voxa does then
today is not known; the builder finds out early, and at the least nothing a person has made
(annotations, saved views, a kept result) may be lost.

## 9. How it is tested, and when it is done

**Without any model files** (every commit, Windows and Linux):

- Each preparing and tidying step against arrays SynthSeg's own Python made from small
  made-up volumes. Those steps need no weights, so the arrays are a few kilobytes and are
  committed, with the script that makes them under `tools/`.
- The steps between networks, the mirrored run, and the 224 cube, with stand-in networks.
- The model folder check, with folders made in the test: every way a folder can be wrong.
- The memory sums and the choice of device, with made-up devices.
- The reducer and the Parts tab, by actions and state, with no window.
- Writing a label file and reading it back.

**With model files and a runtime** (`cargo xtask check-models`, on a machine that has them;
skipped, and saying so, where they are not):

- On the converter's made-up volume, from the array the networks see to the labels:
  - Standard model on the processor: the same label as Python at every voxel.
  - Every other pairing of model and device: at least 99.99% of voxels the same, Dice at
    least 0.999 for every structure, scores within 0.001. The check measured better than
    this on both cards.
- From the made-up volume itself through Voxa's own preparing: the same input array as
  Python made, to within rounding, when no resampling is needed; when it is, the first task
  measures how close Rust's resampling comes and a mark is set from that.
- A run on each device offered, and a forced fall from the card to the processor.

**Done when:**

- A person can label the made-up study from the Parts tab on Windows with no Python
  installed, see the verdict, close Voxa, open the study again and be shown the kept result.
- `cargo xtask check` passes with no model files and no runtime present.
- `cargo xtask check-models` passes on the owner's machine on the processor and on each
  graphics adapter.
- On a 16 GB discrete card, the made-up 160 cube is labelled in under 5 s by the standard
  model and under 30 s by the robust model with cortex areas and scores, counted from the
  button to the parts on screen, after the card's first run. A made-up whole head
  (192 × 256 × 256 once padded) either completes on the card or falls to the next rung and
  says so. These are targets from the check's figures, to be confirmed by the first tasks.
- Voxa starts and does everything it did at 0.4.0 with the `runtime` folder absent.
- Decision record 0005 is written; the README, the architecture notes and the design's
  sections A3, A4 and A7 say what is now true; an acceptance record for 0.5.0 exists.
- The owner has tried it on their own scans. What they see is not written down here.

## 10. What to prove first

In this order, before anything else is built. Each is small; each can end the plan early.

1. **Rust can do what Python did.** A throwaway program: `ort` with `load-dynamic`, the
   1.24.4 DirectML build, one tidied network, the adapter chosen by number, telemetry off.
   The same answer as the check, in about the same time. `cargo deny` clean.
2. **Whether Windows' own `DirectML.dll` is enough**, or one must be carried.
3. **The reduced networks** (classes and areas worked out inside the file) run on DirectML
   and give the same labels, and a large answer can be fetched in halves.
4. **Rust's preparing step against Python's**, with and without resampling.
5. **Memory for each network**, on a card and on the processor, and how the sums hold when
   Voxa's own textures are on the same card.
6. **How much cutting to the head changes the labels** on the made-up head.
7. **What a driver reset does to Voxa.**

If 1 fails, the plan stops and the design's old fallback, a helper program, is looked at
again.

## 11. Left to the builder

The shapes of the traits and types; whether the robust networks are one file or three; how
the adapter list is read; how many threads the processor run takes; the exact words of each
sentence, within the rule that every sentence is plain and says what the picture cannot
show; how the progress line is drawn; the layout of `model.json` beyond what section 6.2
says it holds; the head box's threshold and margin; how a run is cancelled inside a network.

## 12. Defaults the owner should look at

These change what gets built and were not asked:

1. **The runtime as files beside the executable**, loaded when needed, not built in (6.1).
   The other way gives one larger executable and a build that fetches a prebuilt runtime
   from the `ort` project's server.
2. **The converter kept in this repository** as a Python script under `tools/` (6.2). The
   other way keeps it with the owner's pipeline, and Voxa's documents describe the format.
3. **All of SynthSeg in one milestone**, built in the order standard with scores, then
   robust, then cortex areas, with a release possible after any of them (section 3).
4. **Results kept on the computer by default**, with a switch to turn that off (6.7).
5. **Microsoft's build of the runtime with telemetry switched off**, not a build made from
   source (6.1).
6. **Cutting to the head only when memory is short** (6.6). The other way cuts always, which
   is faster and lighter everywhere and never quite what SynthSeg's own program gives.
7. **Counted volumes**, not SynthSeg's added probabilities (6.4).

## 13. Documents this changes

- A new decision record, 0005: Voxa runs a labelling model and ships none. It replaces the
  fifth point of record 0004 and records the runtime, how it is loaded, telemetry, and any
  exception for `unsafe`.
- The README's "Voxa shows labels; it does not make them", the caption of the Parts look,
  and the top of `voxa-labels` all change to say which labels Voxa made and which it did not.
- The design: the row for milestone 4 in A4 ("downloaded on first use" becomes "provided by
  the user"), the crate table in A3, and the first risk in A7, which is now answered.
- `deny.toml` gains nothing if `ort` and what it brings are MIT or Apache-2.0; anything
  else is a question for the owner.
