# Voxa: plugins, models people add themselves

Date: 2026-10-31
Status: draft, for the owner's review (builder check done; the owner's three answers of
2026-10-31 written in)

**Voxa is not a medical device and is not for diagnostic use.** Nothing here changes that. A
plugin is a model someone else made, added by the person using Voxa; Voxa runs it and says
so on everything it makes.

Every choice below is marked: **(owner)** where the owner said it, in the brainstorm of
2026-10-30/31 or in the decisions of 2026-10-29; **(default)** where it is the writer's call and
the owner has not been asked. The defaults that most deserve a look are gathered in section 20.

Built on branch `claude/plugins-4d9099`, cut from `release` (0.6.0, ddc5686). Two other pieces of
work run beside it from the same base: the interface redesign (`claude/ui-redesign-081656`),
which builds the Tools tab and the list of actions this work feeds, and large scans
(`claude/big-scans-c8aec7`), which changes how a model is given its volume. Section 13 says
where they meet.

## 1. What it is for

Voxa runs one family of models today: SynthSeg 2.0, which labels the parts of a brain. Its six
networks come from a model folder the person makes; `voxa-models` checks every file against a
list and never fetches anything. Labels from other programs are already shown "as given".

People want more than that. A researcher wants to try their own model on a scan without
writing Python. Someone else wants a skull strip, or labels of the whole body, or, one day, a
model that marks a tumour. Voxa itself will ship **no diagnostic models** (owner, 2026-10-29):
no brain age, tumour or lesion model is part of Voxa. But a person may add any model as a
**plugin** (owner). That also sidesteps licences that cannot ship in an MIT/Apache app: the
person brings the model under its own terms.

Success: a person presses **Add a plugin…**, picks a folder, reads what the plugin says about
itself, adds it, and runs it on a scan; the result appears in the Labels tab, framed as
"Added by you, not part of Voxa", with the plugin's source, licence and limits. The first real
plugin to work is **MindGrab** (owner), Brainchop's skull strip; TotalSegmentator's MRI model
is the heavier second test (owner).

## 2. Goals, non-goals, anti-goals

Goals:

- G1. A plugin is a folder holding an ONNX network and a description file, `plugin.json`
  (owner). The file says what scans the plugin accepts, how they are prepared, what it returns,
  its source, licence and checksums, and the limits Voxa should show (owner).
- G2. Voxa has one routine per kind of output (owner): a label map with names, one number, a
  value map, an alignment between two scans. This work **builds the label map** and **designs
  the other three** to the level of their description fields and how each is shown, so the
  file format will not need to change for them (owner: "built now / designed now" sizing).
- G3. Adding is an **Add a plugin…** button that checks a folder and copies it into Voxa's
  own plugins folder (owner).
- G4. Only standard ONNX operators; custom-operator libraries, which would run outside code,
  are refused. Every file is checked against its checksum (owner).
- G5. Every run has a memory bound known before it starts (owner, 2026-10-11: "there has to be
  some bounded ram"), and large scans are run in a way that keeps it bounded (owner,
  2026-10-29: large scans need the best performance possible with bounded memory).
- G6. Results are stamped with where they came from and on what, and the stamp goes with them
  everywhere they go.

Non-goals (this work):

- Building the one-number, value-map and alignment routines (designed only, section 12).
- Plugins with several networks (TotalSegmentator's full `total` is five; its fast models are
  one) (default).
- Blending overlapping blocks the way nnU-Net does (section 8.4) (default).
- Drawing the Tools tab: the redesign builds it; this work hands it entries (section 13).
- Click-to-outline models (MedSAM), which need a prompt from the person: later (owner).
- TotalSegmentator as a test plugin: a later piece of work, with its own download (126.4 MB) and
  conversion tools (PyTorch and nnU-Net). Blocks are tested here with small networks the tests
  build themselves (owner, 2026-10-31).

Anti-goals (would make this a failure even if it works):

- Voxa downloading a plugin, linking to one, listing or recommending one, or pointing people
  to a diagnostic model (owner).
- A plugin's result read as Voxa's own, as checked by Voxa, or as a finding.
- Code from a plugin running in Voxa: scripts, custom operators, libraries (owner).
- A plugin run that can exhaust the computer's memory, or take Voxa down with it.
- Anything from the owner's scans in the repository, its tests or its documents.
- SynthSeg's labels changing by a single voxel.

## 3. Constraints

- Windows only, as Voxa is. ONNX Runtime 1.24.4 with DirectML, through `ort` `=2.0.0-rc.13`,
  loaded from the person's runtime folder (record 0006). DirectML hands back no more than about
  2 GiB from one run, and `Tensor::to` must not be used with it (record 0006).
- No new crate dependency without asking the owner. This design needs none; it adds features to
  the `windows` crate already in the tree (section 18).
- The gate: `cargo xtask check` (format, clippy pedantic as errors, 400-line files, 60-line
  functions, tests, cargo deny, docs). `unsafe` only in named, commented modules.
- No downloads without the owner's yes naming the file, its source and its size (owner).
  MindGrab's weights carry no stated licence: testing on this computer is fine, but no converter
  and nothing derived from its weights goes into the repository (owner).
- Every requirement traced to a test, and risks registered, in `docs/quality/` in the format the
  large-scans and redesign branches use, as a new "PL" section in each (owner, 2026-10-29: keep
  the standards' documents current as part of each piece of work).
- Labels keep their own grid and are never resampled (record 0004).

## 4. Approaches

Three ways to build the engine were laid out; the owner chose the first (owner, 2026-10-31).

1. **Voxa does the preparation, and the description picks from a fixed menu (chosen).** Voxa
   holds a menu of preparation steps (resample to a spacing, turn to an orientation, fit to a
   fixed whole grid or cut into fixed blocks, named brightness scalings) and one finishing
   routine per output kind. SynthSeg's own preparing and mapping-back code becomes that menu.
   Every setting is visible on the confirmation card and testable in Rust. Cost: a model that
   needs a step not on the menu cannot be added until Voxa gains that step.
2. **The plugin carries its own preparation inside the ONNX file.** Less code in Voxa, but
   resampling and orientation cannot be done well inside ONNX, and a wrong scaling would be
   invisible on the card. Kept only as one menu setting: scaling "none, done inside the model"
   (owner).
3. **Plugins bring scripts for their preparation.** Ruled out: code from outside running in
   Voxa (owner).

Within the chosen way, one more choice was proposed by the writer and accepted by the owner
(owner, 2026-10-31): **plugin networks run in a helper process**, a second copy of `voxa.exe` with no window, under
a Windows job object that caps its memory and ends it with Voxa. The alternative is to run them
in Voxa's own process, as SynthSeg runs. The helper wins on three counts: a model file from
anywhere cannot take Voxa down when the runtime crashes on it; the memory a run used can be
measured exactly (the helper's own peak is the run's peak); and the system enforces the bound,
so a run that would pass it fails instead of exhausting the computer. It costs a little start-up
per run (starting a process and loading the runtime, not yet measured; section 15 sets the
budget), a small message format over pipes, and a second module allowed `unsafe`
(the job object). SynthSeg stays in Voxa's own process, as it is.

## 5. What a person sees and does

The redesign draws these in its Tools tab and its style; the words are this design's.

### 5.1 Adding

1. **Tools tab, "Added by you" → Add a plugin…** opens a folder picker (owner).
2. Voxa checks the folder where it is, before copying anything (section 7). The first thing
   found wrong is said in a sentence, and nothing is copied (owner).
3. A **confirmation card** shows what the plugin says about itself: name, version, who made it,
   source (as plain text, never a link), licence, its limits, what scans it accepts and what it
   returns; then every preparation setting in plain words ("Resampled to 1 mm, turned to
   left-inferior-anterior, fitted to 256 × 256 × 256, brightness: 2nd to 98th percentile of the
   fitted grid"). Above all of it, Voxa's own words: **"Added by you, not part of Voxa. Voxa has
   not checked what this model does. Not for diagnostic use."** Then **Add** and **Cancel**
   (owner).
4. **Add** copies the listed files, and only those, into `plugins` in Voxa's settings folder,
   beside the kept `labels`, and checks the copy again. The copy is what runs: changing the
   original afterwards changes nothing (owner).
5. **Memory is measured once** (owner): the helper runs the network on one made-up input on the
   processor, then on each discrete card, and the figures are kept with the copy (section
   10.3). Progress is shown; it takes seconds to a minute. If the processor run fails, the
   plugin is not added and the copy is removed, with the reason. If a card run fails and the
   processor's worked, the plugin is added and says it runs on the processor only.
6. A plugin whose `id` matches one already added, with a different version or files, asks
   before it replaces it, naming both versions (owner).

### 5.2 The entry in the Tools tab

Each added plugin is one entry under "Added by you" (section 13.1 lists its fields): its name and
version, who made it, its source and licence, whether its files passed their check, what it
accepts and returns, where it can run, and **Run**. It is also an action in the list of actions,
"Run *name*", so search finds it (redesign). **Remove** asks first, then deletes Voxa's copy;
results already made stay, and still say which plugin and version made them (owner).

### 5.3 Running

**Run** works on the scan in view, which needs a 3D volume, as labelling does.

1. **Modality.** The plugin lists the DICOM modalities it accepts. A scan of any other modality,
   or with none given, is refused with a sentence: "This plugin is for MR scans; this scan is
   CT." Voxa knows the modality for certain, so this is always a refusal (owner).
2. **Contrast.** The plugin lists the contrasts it accepts, or says "any". Voxa reads the scan's
   contrast only from the words of its series description, exactly as the redesign's plain names
   read them (`naming`: its word list and its rule that a description naming two different ones
   of T1, T2 and PD names none). The names a plugin may list are that list's: T1, 3D T1, T2, PD,
   FLAIR, T2*, Diffusion, ADC map, SWI, STIR, Angiography (TOF). Then (owner for the three
   outcomes; how names are matched is a default):
   - **matched**: at least one name read from the description is one the plugin accepts, a
     "3D T1" counting as T1. It runs, and the result notes "The scan's description says T1, as
     this plugin expects." (the note is a default). So "T2 FLAIR" matches a plugin that accepts
     FLAIR, and one that accepts T2;
   - **different**: names were read and none is accepted (a T1 plugin given a scan described
     as T2, or as diffusion). Voxa warns before running, "This plugin expects T1; this scan's
     description says T2.", and runs only if the person confirms. The mismatch is stamped on
     the result and stays with it, in everything saved or exported;
   - **not said**: no name was read, including a description that names conflicting ones
     ("T1 T2"), which the redesign also leaves unnamed. It runs, and the result is stamped
     "Contrast not confirmed: the scan's description does not say."
3. **Spacing.** A plugin may state the thickest slices it was made for; a scan whose voxels are
   further apart than that runs, with a note saying both figures (default).
4. Progress says the stage: preparing, block *n* of *m* (or "running" for a whole grid),
   finishing. **Stop** ends the helper at once. A run of a plugin and a SynthSeg labelling run
   share one turn: one at a time, a new one stops the old (default).
5. The result opens in the Labels tab with the banner (section 11).

## 6. The plugin folder and its description

A plugin folder holds `plugin.json` and the files it lists. Nothing else in the folder is read
or copied. `plugin.json` is JSON, at most 1 MiB, in **format 1** (default).

Every setting that changes the answer must be stated: there are no defaults for them, and a
missing or unknown one is a refusal that names the field (owner: strict, since the file's writer
may be the model's author or someone wrapping a bare `.onnx`).

### 6.1 Fields

| Field | What it holds | Checked |
|---|---|---|
| `format` | `1` | a number Voxa knows |
| `kind` | `label-map`; later `number`, `value-map`, `alignment` | one Voxa has a routine for; a known later kind is refused with "This version of Voxa cannot run plugins that give a value map yet." |
| `id` | a stable identity chosen by its maker, e.g. `example.skull-strip` | 3 to 64 of `a-z 0-9 . -` |
| `name`, `version` | shown | printable, at most 60 and 20 characters |
| `made_by`, `source`, `licence` | shown; `source` as plain text | printable, at most 80, 200, 200 |
| `about` | one sentence of what it does | printable, at most 300 |
| `limits` | up to 8 sentences the plugin's maker wants shown | printable, each at most 300 |
| `files` | `[{ "name", "bytes", "sha256" }]` | plain names, each at most 2 GiB, all together at most 4 GiB |
| `network` | the name of the `.onnx` file among `files` | listed |
| `accepts.modality` | DICOM modality codes, e.g. `["MR"]` | at least one |
| `accepts.contrast` | `"any"`, or words of the vocabulary, e.g. `["T1"]` | known words |
| `accepts.body_part` | shown only, e.g. "head" | printable, at most 60 |
| `accepts.thickest_mm` | optional: the widest voxel spacing it was made for | a number above 0 |
| `input` | the preparation (section 8) | every field present and valid |
| `output` | what the network gives and what it means (section 9) | every field present and valid |

"Printable" means no control or format character (the redesign's `printable` rule). A
description's words are data: Voxa shows them, inside the "Added by you" frame, and never
follows them.

### 6.2 An example

Made up for this document; no real model's sums.

```json
{
  "format": 1,
  "kind": "label-map",
  "id": "example.skull-strip",
  "name": "Example skull strip",
  "version": "1.0",
  "made_by": "An example lab",
  "source": "https://example.org/skull-strip",
  "licence": "MIT",
  "about": "Marks the brain inside the skull on an MRI scan of the head.",
  "limits": ["Trained on adult heads only."],
  "files": [{ "name": "strip.onnx", "bytes": 612345, "sha256": "…64 hex digits…" }],
  "network": "strip.onnx",
  "accepts": { "modality": ["MR"], "contrast": "any", "body_part": "head" },
  "input": {
    "spacing_mm": [1, 1, 1],
    "axes": "LIA",
    "layout": "channels-first",
    "fit": { "whole": [256, 256, 256], "outside": "lowest" },
    "scaling": { "kind": "percentiles", "low": 2, "high": 98, "over": "grid" }
  },
  "output": {
    "gives": "scores",
    "channels": [0, 1],
    "keep_largest_piece": true,
    "names": [{ "id": 1, "name": "Brain", "side": "midline", "group": "Brain" }]
  }
}
```

## 7. Checks

### 7.1 When a folder is offered, in this order

1. The folder exists and holds `plugin.json`, no longer than 1 MiB.
2. `format` is known; `kind` is one with a routine in this version.
3. Every field of section 6 is present and valid; `input` and `output` as sections 8 and 9.
4. Every file in `files`: a plain name (not empty, no separator, no `..`, no `:`, no NUL, the
   rule of `is_plain_name` today); a regular file, not a link or other reparse point; of the size
   listed; of the SHA-256 listed.
5. The network file passes the reading of section 7.3.
6. The network's image inputs (one; two for an alignment, the fixed scan's first) and the output
   it is read from fit what `input` and `output` say: element types, the number of dimensions,
   every fixed dimension equal to the description's, and every free dimension one Voxa will fix
   by name. A network with more than one output must name the one to read in `output.from`;
   the others are never fetched (default).

The confirmation card is shown only when all pass. After **Add**, the files are copied into a
new folder under `plugins`, named by the first 16 hexadecimal digits of the SHA-256 of
`plugin.json` (never by anything the description says), written to a temporary name first and
renamed when whole; then steps 1 to 6 run again on the copy.

### 7.2 Before every run

The helper checks the network file's SHA-256 against the copy's `plugin.json` before it opens
it, so a file changed after it was added is refused: "This plugin's files have changed since it
was added. Remove it and add it again." (default: in the helper, which removes the gap between
check and open). `plugin.json` itself is read again and must have the sum the folder is named
by.

### 7.3 Reading the network file

Voxa reads the ONNX file's structure itself, by the protobuf wire format, without holding its
weights (lengths are skipped over, not read). A small reader written for this, with no new
dependency (default). It refuses the file, with a sentence, when:

- any `opset_import` names a domain other than the standard one (`""` or `ai.onnx`), or a
  version above what the loaded runtime supports (the builder takes the maxima from ONNX
  Runtime 1.24's documentation); ONNX Runtime's own extra operators (`com.microsoft`) count as
  non-standard too (default);
- any node, in the main graph or in a subgraph held by an attribute (`If`, `Loop`, `Scan`, at
  any depth), names a non-standard domain;
- the model holds local functions (default);
- any tensor's data lies outside the file (`data_location` external) at a place that is not a
  plain name listed in `files`, or beyond that file's length;
- subgraphs nest more than 16 deep, the graph has more than 100 000 nodes, or a field's length
  runs past the end of the file (defaults: limits that no real network of this kind comes
  near, so that a hostile file cannot make the reader recurse or allocate without end).

The model's `metadata_props`, `doc_string` and producer are never shown and never followed.

## 8. Preparation: the menu

A volume is turned into what the network sees in these steps, each chosen by the description.
They generalise what `voxa-segment` already does for SynthSeg (resample along the scan's own
axes, turn the axes, cut, scale, pad, and the record `Back` of how to come back), and SynthSeg's
own path keeps its exact steps and its golden tests.

### 8.1 The grid: `spacing_mm`, `axes`, `layout`

- `axes`: three letters, one from each of L/R, A/P and S/I, e.g. `RAS`, `LIA`. Letter *n* is the
  direction in the patient that the *n*-th spatial dimension of the input tensor points to as
  its index grows. The spatial dimensions are in the order they appear in the tensor's shape,
  the last changing fastest (row-major, as ONNX stores tensors).
- Voxa never rotates a scan: it takes the scan's own axes, chooses for each letter the axis whose
  direction is nearest (the largest number in its column of the scan's placement, SynthSeg's
  rule), and turns it round where it runs the other way. The grid therefore stays aligned to the
  scan's own axes, as SynthSeg's does.
- `spacing_mm`: the distance between voxels along each spatial dimension, in the tensor's order.
  Resampling along each of the scan's own axes is linear, as SynthSeg's resampling is.
- `layout`: `channels-first`, the input `[1, 1, d0, d1, d2]`; or `channels-last`,
  `[1, d0, d1, d2, 1]`. The output's layout is the same.
- Only the part of the grid that is needed is ever worked out: for a whole grid, the voxels of
  the box; for blocks, one block at a time (section 8.4). A scan of 2 000 slices is never
  resampled whole into a second copy.

### 8.2 Fitting: `fit`

Every input the network is given has a fixed size, stated in the description (default). This is
what makes the memory measured once exact (section 10.3).

- `{ "whole": [n0, n1, n2], "outside": … }`: one box of that size, centred on the centre of the
  scan's grid. A scan larger than the box along an axis is cut to it, and the result notes how
  many millimetres were left out at each end; a smaller one is surrounded with the `outside`
  value.
- `{ "blocks": { "size": [b0, b1, b2], "step": [s0, s1, s2] }, "outside": … }`: the scan's whole
  grid is covered by blocks of that size, `step` voxels apart (each step from 1 to the block's
  size), the last block of each row laid against the far end, as nnU-Net lays its blocks. A grid
  smaller than a block along an axis is surrounded with the `outside` value up to the block's
  size.
- `outside`: `"lowest"`, the scan's lowest value before scaling (so a CT is surrounded by air,
  not water), or `"zero"`, 0 after scaling. Required.

### 8.3 Brightness: `scaling`

- `{ "kind": "percentiles", "low": p, "high": q, "over": … }`: the value at the `p`-th
  percentile becomes 0 and the `q`-th becomes 1, everything outside held at 0 or 1 (SynthSeg's
  step, with its percentiles as parameters). Percentiles are worked out as SynthSeg's are.
- `{ "kind": "z-score", "voxels": "all" | "above-zero", "over": … }`: the mean is taken away
  and the result divided by the standard deviation, both worked out over the voxels named.
- `{ "kind": "fixed", "clip": [a, b], "subtract": m, "divide": s }`: values held within
  `[a, b]`, then `(x − m) / s`. This covers CT's fixed z-score and a fixed range (subtract `a`,
  divide by `b − a`).
- `{ "kind": "none" }`: the scan's own values, after the file's rescale, as stored; for a model
  that scales inside itself (owner).
- `over`: `"scan"`, every voxel of the scan resampled to the plugin's spacing and turned,
  before any cut or surroundings (SynthSeg's way: its percentiles are of the 1 mm grid); or
  `"grid"`, the fitted whole grid, surroundings included (the way a model trained on conformed
  volumes was scaled). `"grid"` is allowed only with `whole`. Required for percentiles and
  z-score.

Statistics are gathered as the resampled values are worked out, slab by slab, with memory that
does not grow with the scan; the resampled scan is never held whole for them (default). Means and
standard deviations are summed in 64-bit numbers and are exact to that. Percentiles are read from
a histogram of 65 536 equal bins between the lowest and the highest resampled value, two passes,
interpolating within the bin, so a percentile is within 1/65 536 of the range of its exact value;
`docs/plugins.md` says so. SynthSeg's own path keeps its exact percentiles: it does not use this.

### 8.4 Blocks and how their answers are put together

Each voxel takes its label from the block whose centre is nearest to it, the first in order when
two are equally near (default). A block's answer is finished to labels in the helper (section
9), and only the voxels it owns are written into the result. Memory is then one block and the
grid of labels, whatever the number of parts. nnU-Net instead adds every block's scores with a
Gaussian weight, which for 118 parts over a whole body would need gigabytes; near block edges
the two can differ. The result says so when blocks were used (section 11.2), and blending is
left to a later version.

## 9. The label-map routine

### 9.1 What the network gives: `output`

- `"gives": "scores"`: one channel per entry of `channels`; each voxel takes the channel with
  the largest score, the first when several are equal (SynthSeg's rule), and that channel's
  label from `channels` (a label of 0 is nothing).
- `"gives": "chance"`: one channel; a voxel takes `label` where the number is above
  `threshold`. Both fields required.
- `"gives": "labels"`: a label for each voxel, as 8-, 32- or 64-bit integers. A label that is
  neither 0 nor in `names` stops the run: "The plugin gave labels its description does not
  name."
- `"from"`: the name of the network output to read; required only when the network has more
  than one (nnU-Net exports, for one, may keep extra outputs).
- `"keep_largest_piece"`: `true` keeps only the largest connected piece of all labelled voxels
  together (SynthSeg's `largest_piece`), `false` keeps all. Required.
- `"names"`: for each label from 1 to 65 535 that can appear, its name, side (`left`, `right`,
  `midline`) and group; a colour is optional (the colour Voxa gives a number, otherwise). Every
  label `channels` or `label` can give must be named.

Every number a network gives is checked to be finite, and every answer to be of the kind and
size expected, as `find` does for SynthSeg; anything else stops the run with a sentence.

### 9.2 Back to the scan

The labels are mapped back as SynthSeg's are: padding or surroundings taken off, a cut put back
where it was, the axes turned back, so the label grid lies at the plugin's spacing along the
scan's own axes, with its own placement in the patient (record 0004: never resampled to the
scan's grid). A voxel outside the box of a whole grid has no label.

### 9.3 The label set

A `LabelGrid` and a `Table`: the names from the description, each part's trust "made by a
model", no verdict (a plugin has no self-check unless a later version adds one), and the
record of who made it (section 11). An answer with no labelled voxel is said: "The plugin
labelled nothing in this scan."

## 10. Running

### 10.1 The helper

`voxa.exe --plugin-helper` starts the helper: before any window, the program sees the argument
and serves one job over its standard input and output, then ends (default). It loads the
runtime from the same folder Voxa uses, with telemetry off first, as `Runtime::load` does.

Voxa starts the helper with its pipes, puts it into a job object before telling it anything (the
helper's first act is to wait for its first message, so nothing runs before it is in the job),
then sends the job. The job object (default):

- ends the helper when Voxa ends, or when the job is closed (that is how **Stop** works: at
  once, on a card too);
- lets it start no process of its own;
- caps the ordinary memory it may hold at the bound of section 10.3, so a run that would pass it
  is refused memory and fails, instead of exhausting the computer.

### 10.2 What passes between them

Length-prefixed binary messages over the pipes (the builder chooses the exact frame). Voxa sends
where the runtime is, the network file and its sum, the device (the processor, or a card by
name and identity), the input's shape and the names of its free dimensions with their sizes;
then the blocks, one at a time, as 32-bit numbers. The helper answers each block with its labels
as 16-bit numbers, finished as section 9.1 says (the largest-piece step is Voxa's, over the
whole result). Scores never cross the pipe. Nothing goes to disk: no scan value is written to a
file at any point (default). At the end, the helper reports the most ordinary memory it held at
any moment (the system's peak count for the process) and the most of the card it held.

A failure comes back sorted as today's `RunError` sorts the runtime's words: no room, card
lost, stopped, or failed with the runtime's words for the log only. A helper that ends without
answering is a failure: "The plugin stopped without finishing. Voxa is unaffected."

### 10.3 Memory, measured once

When a plugin is added (section 5.1), the helper runs its network once on a made-up input of the
fixed size (a smooth pattern, not zeros) on the processor, then on each discrete card. The
peaks are kept in `measured.json` beside the copy, for that runtime library (by its sum) and that
card (by its name). A run on a device with no figures, or with a runtime that has changed, is
measured first, said in the progress line (default).

Before a run, a place is chosen as `rungs` chooses one today, with the measured figures in place
of SynthSeg's per-voxel ones: on a card, what the network took of it, with a quarter added,
must fit the card's budget, and what it took must fit what no program holds of it; the helper's
ordinary-memory peak with a quarter added must fit what the system can still promise, and that
same figure is the job's cap; and Voxa's own part of the run (one block of 32-bit numbers, the
grid of 16-bit labels, and the statistics) is counted with it. The person's **Runs on** choice is
the one labelling uses (default).

### 10.4 The volume

A run takes its volume as labelling does today, the one the store keeps for the series, and is
stopped when the scene moves to another series or export, as labelling is. When large scans
land, plugins get their volume through the same call labelling then uses (a 32-bit volume made
from the held 16-bit one, of a range of slices) (section 13.3).

## 11. Results

### 11.1 The banner and the stamps

Wherever the labels are shown or written, they carry, first:

> **Added by you, not part of Voxa.** *Name* *version*, made by *made_by*. Source: *source*.
> Licence: *licence*. *Limits.* Not for diagnostic use.

followed by the run's notes: the contrast note or stamp (section 5.3), the spacing note, what a
whole grid left out, "Run in blocks: near the edges between blocks, labels may differ from the
model's own program." when blocks were used, and the device and date as labelling gives them
(owner for the banner and contrast stamps; the other notes default).

Places: the Labels tab (the redesign draws it first), the captions of the plane and 3D views
(which today say "Labels: made elsewhere"), the kept files, a table saved with **Save as**, and
the parts table exported as CSV.

### 11.2 How it is kept

The table's `voxa` object (`Made`) gains an `added_by_you` part: the plugin's id, name, version,
made by, source, licence, limits, and the SHA-256 of its `plugin.json`; and the contrast stamp
among the notes. Written and read like the rest of `Made`: cut to length on both sides, never
making a table unreadable. A table read from a file that holds such a part is shown with the
banner, as it says (default: the direction that cannot mislead).

One result is kept per series and per plugin, in the kept `labels` folder, under a key made from
the series' hash and the plugin's id (default). Removing a plugin keeps its results (owner).
**Show** in the plugin's entry opens it again; **Delete** deletes it, as labelling's do.

## 12. The other kinds, designed now and built later

Each uses sections 6 to 8 unchanged; only `output` and the finishing differ. Their fields below
are part of format 1 now, so the format does not change when they are built: this version reads
and checks them, then refuses the kind with the sentence of section 6.1, and `docs/plugins.md`
lists them marked "not run by this version yet" (default).

- **One number** (`kind: number`). The network gives one 32-bit number. `output` states its
  `name`, `unit` and `decimals`, and may state the `range` its maker says it is meaningful in.
  Shown in the plugin's entry and kept per series, with the banner and stamps; a number outside
  the stated range is shown with that said. Voxa never compares it with norms, colours it as
  good or bad, or names it as a finding. Whole grid only.
- **A value map** (`kind: value-map`). The network gives one channel of 32-bit numbers on the
  grid. `output` states the `unit` and the range to draw. Shown as a coloured layer over the
  planes with its scale and the value under the pointer, saved as a NIfTI of floats with its
  table. Blocks join as labels do (nearest centre).
- **An alignment** (`kind: alignment`). Two scans, a fixed and a moving one, each prepared by the
  same `input` onto its own box, given as the network's two image inputs in its own order, the
  fixed scan's first. The network gives either a field of displacements
  `[1, 3, d0, d1, d2]` (its unit, millimetres or voxels, stated) or a 4 × 4 matrix. Shown by
  drawing the moving scan on the fixed scan's planes, side by side or blended. Whole grid only.
  It needs the redesign's comparison of two scans. Note from the survey: a model that warps
  with 3D `GridSample` will run on the processor, as DirectML 1.15 does not run it.

## 13. Where the code goes, and where this meets the other work

### 13.1 For the redesign: a tool entry

This work defines the type the Tools tab draws, in `voxa-app` (names are the builder's; the
fields are this design's) and writes it, with the actions, to the shared notes folder:

- `id`, `name`, `version`, `made_by`, `source`, `licence`, `about`, `limits`;
- `checked`: whether its files passed their check when last looked at, with the sentence if not;
- `accepts` (modality, contrast, body part) and `returns` (the kind, and the part names' count),
  each as a short sentence;
- `runs_on`: where it can run, from the measured figures ("processor and RTX 5070 Ti",
  "processor only");
- `state`: ready, being added or measured, running with its stage, cannot run now with why;
- `kept`: whether a result is kept for the series in view;
- its actions: Run, Stop, Show, Delete, Remove; and Add a plugin… for the section. "Run *name*"
  goes into the list of actions at run time.

The confirmation card and the contrast warning are drawn by small functions in a new
`ui/plugins/` that the Tools tab calls. Until the redesign lands, this branch shows the entries
under the labelling strip in the Parts tab with one call, which the redesign moves (default;
the redesign session is told).

### 13.2 Crates

- **`voxa-plugins`** (new; `forbid(unsafe_code)`): the description and its checks, the ONNX
  reader, the preparation menu, blocks and their joining, the label-map finishing, the stamps,
  and both ends of the helper's messages over any `Read` and `Write`.
- **`voxa-segment`**: its preparing pieces (resampling, turning, `Back`, percentiles,
  `largest_piece`) are generalised so the menu can call them; SynthSeg's `prepare` keeps its own
  recipe, and every golden test stays as it is.
- **`voxa-models`**: `Runtime::open` fixes free dimensions by the names given (today only `x`,
  `y`, `z`), outputs of 32- and 64-bit integers are read, and a small module starts the helper
  in its job object: `unsafe` allowed there, with `SAFETY` comments, beside `cards.rs`.
- **`voxa-app`**: state and actions for plugins, the workers that add, measure and run, the
  helper's entry in `main.rs`, and `ui/plugins/`.

### 13.3 Merge points

- Redesign: the Tools tab draws section 13.1's entries; the contrast vocabulary is the
  redesign's `naming` words. Whichever lands second makes the other use one copy of them.
- Large scans: plugins take their volume through its call (section 10.4); `voxa-models` gains its
  `total_memory`, which this work does not touch.
- Both branches have started `docs/quality/requirements.md` and `risk-register.md`; this work
  adds a "PL" section to each, after theirs.

## 14. Testing

No download is needed for any automated test (default):

- **Without a runtime**: the description's rules, one test per refusal; the ONNX reader on files
  the tests write themselves, byte by byte (standard and custom domains, subgraphs at depth,
  local functions, external data inside and outside the folder, lengths that lie, a file cut
  short); the preparation steps on made-up volumes, with an off-centre bead to show every axis
  code puts the bead where it is and back (a wrong turn moves it); the scalings against figures
  worked by hand; whole-grid cuts and surroundings; blocks covering every voxel once, nearest
  centre; the finishing kinds; the stamps in every place of section 11.1; modality and contrast
  decisions; the messages, with a stand-in network in the helper; adding, replacing and removing
  plugins in a temporary settings folder.
- **With the runtime** (ignored unless `VOXA_RUNTIME` is set, as the real-network tests are
  now): tiny networks the tests write as ONNX themselves (a 1 × 1 × 1 convolution with chosen
  weights, so the right labels are known) run through the real helper on the processor and on
  each card, whole and in blocks, with memory measured and the job's cap shown to refuse a run
  set to pass it.
- **SynthSeg**: its golden tests unchanged and passing.
- **MindGrab, on this computer only**: with the owner's yes of 2026-10-31 for its two files (`model.json`,
  68 331 bytes, and `model.bin`, 584 948 bytes, from `neuroneural/brainchop` on GitHub,
  `public/models/mindgrab`), a converter kept outside the repository writes an ONNX file and a
  `plugin.json`; Voxa adds it and runs it on a made-up head. The converter is a short Python
  script that reads the browser model's weights and builds the network with the `onnx` package
  of the Python environment already on this computer for the SynthSeg converter, so nothing else
  is downloaded; it and its output live in a git-ignored folder outside the repository
  (`dicom-analysis\models\`) (default). Brainchop prepares a scan with its own conform step
  (1 mm, 256 cubed, eight-bit values), then its 2nd and 98th percentiles; Voxa's menu states this
  as percentiles over the fitted grid and does not copy the eight-bit step, which the local
  `plugin.json` says among its limits. The mark is therefore functional, not voxel-for-voxel: the
  mask covers the made-up brain and leaves out what lies outside it (default). What was checked
  is recorded in the acceptance record without any figure from a real scan, and nothing derived
  from the weights is committed (owner).

## 15. Budgets

Measured on this computer and recorded (default figures):

- Adding a plugin of 100 MB: checks within 2 s.
- A run's start-up (the helper started, the runtime loaded, the network opened) within 1.5 s
  beyond the network's own opening time.
- Preparing a 256 × 256 × 256 whole grid from a head scan on the processor within 2 s.
- Voxa's own memory during a run: one block of 32-bit numbers, the grid of 16-bit labels and the
  statistics, beside the volume it already holds.
- The helper's memory: within the measured bound, enforced by the system.

## 16. Requirements

For `docs/quality/requirements.md`, section "PL: plugins (design:
2026-10-31-voxa-plugins-design.md)". "Shown by" is filled in as the tests are written.

| No. | Requirement | Design | Risk |
|---|---|---|---|
| PL-01 | Adding a plugin copies only the files its description lists, only after every check passes, into a folder named by the description's sum, and checks the copy again; a folder that fails a check is not copied and the first failure is said in a sentence. | 5.1, 7.1 | R-PL-07, R-PL-08 |
| PL-02 | Every listed file is checked for a plain name, being a regular file, its size and its SHA-256 when added; the network file again by the helper before every run. | 7.1, 7.2 | R-PL-06, R-PL-07 |
| PL-03 | A network that names a non-standard operator domain anywhere (subgraphs included), holds local functions, keeps data outside the listed files, or passes the reader's limits is refused. | 7.3 | R-PL-05 |
| PL-04 | A description missing any setting that changes the answer, or holding an unknown value, is refused with the field named. | 6 | R-PL-03 |
| PL-05 | The confirmation card shows "Added by you, not part of Voxa. Voxa has not checked what this model does. Not for diagnostic use.", the plugin's own words, and every preparation setting in plain words; nothing is copied before Add. | 5.1 | R-PL-01, R-PL-03 |
| PL-06 | A plugin's memory is measured when it is added and on any new device or runtime; a run starts only where the measured need fits; the helper's ordinary memory is capped by the system at that need. | 10.3 | R-PL-09 |
| PL-07 | Plugin networks run only in the helper; when it fails, is stopped or ends early, Voxa keeps running and says why. | 10.1, 10.2 | R-PL-05, R-PL-10 |
| PL-08 | A scan of a modality the plugin does not list, or with none, is refused before anything runs. | 5.3 | R-PL-02 |
| PL-09 | When the scan's description names a contrast other than the plugin's, Voxa warns in plain words and runs only on confirmation; the mismatch is stamped on the result. | 5.3, 11 | R-PL-02 |
| PL-10 | When the scan's description names no contrast, the result is stamped "Contrast not confirmed". | 5.3, 11 | R-PL-02 |
| PL-11 | The banner and the stamps are on the result in the Labels tab, the plane and 3D captions, the kept files, a saved table and the exported CSV. | 11 | R-PL-01, R-PL-02 |
| PL-12 | The grid follows `spacing_mm`, `axes` and `layout`; a whole grid is centred and its cut said; blocks cover every voxel; mapping back puts each label where its voxel lies, for every axis code. | 8.1, 8.2, 9.2 | R-PL-04 |
| PL-13 | Each scaling gives the figures section 8.3 defines. | 8.3 | R-PL-03 |
| PL-14 | Scores, chances and labels are finished as section 9.1 says; a label not named, a number not finite, or an answer of the wrong size stops the run with a sentence. | 9.1 | R-PL-03 |
| PL-15 | In blocks, each voxel takes its label from the nearest block's centre, and Voxa's memory does not grow with the number of parts. | 8.4 | R-PL-09, R-PL-11 |
| PL-16 | SynthSeg's labels are unchanged: its golden tests pass unchanged. | 13.2 | |
| PL-17 | Remove deletes Voxa's copy and keeps the results, which still name the plugin and version. | 5.2, 11.2 | R-PL-12 |
| PL-18 | A plugin with the same id and another version replaces the old only after the person confirms, naming both versions. | 5.1 | R-PL-12 |
| PL-19 | Voxa never downloads a plugin, and shows a plugin's source only as text. | 2, 5.1 | R-PL-08 |
| PL-20 | A description's words are shown only inside the "Added by you" frame, printable, cut to their lengths. | 6.1 | R-PL-01 |
| PL-21 | No scan value is written to disk to run a plugin. | 10.2 | R-PL-13 |
| PL-22 | Each plugin is an action "Run *name*" in the list of actions, found by search. | 13.1 | |
| PL-23 | A plugin run and a labelling run never run at once; a new one stops the old. | 5.3 | R-PL-09 |

## 17. Risks

For `docs/quality/risk-register.md`, section "PL: plugins", in its scales.

| No. | Hazard and how harm could follow | Before | Control | After |
|---|---|---|---|---|
| R-PL-01 | A plugin's labels are taken for Voxa's own, or for checked ones, and believed. | serious / likely | The banner first wherever they are shown or written (PL-05, PL-11); the plugin's words only inside the frame (PL-20); no diagnostic wording in Voxa's own text. | serious / rare |
| R-PL-02 | A plugin is run on a scan it was not made for (another modality or contrast), and gives confident wrong labels. | serious / likely | Modality refused (PL-08); contrast warned and stamped (PL-09, PL-10); the plugin's accepts and limits on its entry. | serious / possible |
| R-PL-03 | A description states the preparation wrongly (scaling, axes, spacing), and the labels look plausible but are wrong. | serious / possible | No defaults for answer-changing settings (PL-04); every setting in plain words on the card (PL-05); each setting tested to do what it says (PL-13, PL-14). Residual: Voxa cannot know a setting is wrong; the banner says Voxa has not checked the model. | serious / possible |
| R-PL-04 | Labels are mapped back to the wrong place or the wrong side. | serious / possible | One generalised mapping, tested with an off-centre bead for every axis code (PL-12), SynthSeg's mapping unchanged (PL-16). | serious / rare |
| R-PL-05 | A hostile network file runs code, or crashes Voxa and loses unsaved work. | serious / possible | Standard operators only, reader limits (PL-03); the helper process, ended with Voxa, starting nothing (PL-07). Residual: a fault in ONNX Runtime itself runs with the person's own rights; SECURITY.md says to add plugins only from sources they trust. | serious / rare |
| R-PL-06 | A plugin's files are changed after it was added, and something else runs under its name. | serious / rare | Files copied into Voxa's folder; sums checked by the helper before every run (PL-02). | serious / rare |
| R-PL-07 | A description's names lead a copy or a read outside the folder. | serious / possible | Plain names only, no links followed, a folder named by a sum (PL-01, PL-02). | minor / rare |
| R-PL-08 | Voxa is taken to vouch for a plugin, or for its licence. | minor / possible | Never downloads or links (PL-19); the licence is the plugin's own words and the person's responsibility, said in the docs. | minor / rare |
| R-PL-09 | A run takes so much memory that the computer slows or another program loses work. | serious / likely | Measured once (PL-06); a run starts only where it fits; the system caps the helper; blocks keep Voxa's own part small (PL-15); one run at a time (PL-23). | minor / rare |
| R-PL-10 | A run stops part-way and a part-made result is shown as whole. | serious / rare | Only a finished answer becomes a result; a helper that ends early is a failure said in a sentence (PL-07). | serious / rare |
| R-PL-11 | Labels near block edges differ from the model's own program's, and a small structure there is judged absent. | minor / possible | Said on the result when blocks were used (11.1). | minor / possible |
| R-PL-12 | A result made by an older version, or by a removed plugin, is taken for the newer one's. | minor / possible | Version and sum on every result (PL-17, PL-18). | minor / rare |
| R-PL-13 | Pixels of a scan are left on disk by a plugin run. | serious / possible | Pipes only, nothing written (PL-21). | serious / rare |

## 18. Security notes and third parties

For `SECURITY.md`, a section "Plugins":

- S-PL-01. Plugin network files come from anywhere. Voxa accepts only standard ONNX operators,
  reads the file's structure with limits before ONNX Runtime sees it, and runs it in a helper
  process in a job object that ends with Voxa, may start nothing, and is capped in memory. The
  helper has the person's own rights: it is not a sandbox. Add plugins only from sources you
  trust.
- S-PL-02. Names in a description never lead outside its folder; links are not followed; the
  copy lives in a folder named by a sum.
- S-PL-03. The network file's sum is checked by the helper before it is opened, every run.
- S-PL-04. A description's words are shown as plain text inside the "Added by you" frame, never
  as links, never as Voxa's own words.
- S-PL-05. Scan values pass between Voxa and its helper through pipes only.
- S-PL-06. The helper's runtime has telemetry off before anything else, as Voxa's does.

Third parties: no new crate. The `windows` crate gains the features for job objects, process
memory counters and the threading calls they need. ONNX Runtime's risk changes: it now parses
files from outside, which the helper and the reader above stand in front of. The SOUP list and
record 0006 are updated.

## 19. Documents this work writes or updates

- `docs/adr/0007-plugins.md`: plugins, the helper process, and the second module allowed
  `unsafe` in `voxa-models`.
- `docs/plugins.md`: for people who make a plugin: the folder, `plugin.json` field by field, the
  menu, how to check a plugin, and that Voxa recommends none.
- `SECURITY.md` (section 18), `docs/intended-use.md` (plugins are added by the person, not part of
  Voxa, not for diagnostic use), `docs/models.md` (a pointer), `CHANGELOG.md`,
  `THIRD-PARTY-NOTICES.txt` if anything changes there.
- `docs/quality/requirements.md` and `risk-register.md`, section PL; a usability entry for the
  add-and-run flow when the owner first tries it.
- An acceptance record for the MindGrab check (section 14), with no figure from a real scan.

## 20. Left to the builder, and the calls made

Left to the builder: the module boundaries inside each crate, the names of types, the message
frame, how percentiles and statistics are gathered (as long as SynthSeg stays exact), the made-up
input used to measure, the exact limits ONNX Runtime 1.24 supports for IR and opset versions,
and the test writer for ONNX files.

Decided by the owner on 2026-10-31, after the builder check: plugin networks run in a helper
process (`voxa.exe --plugin-helper`) under a job object that caps its memory and ends it with
Voxa (section 4); TotalSegmentator is left for a later piece of work, with its own download and
conversion (section 14); MindGrab's two files may be downloaded for the check on this computer
(section 14).

The calls made without the owner (default), most important first:

1. (Accepted by the owner: the helper process, above.)
2. Every input a network sees has a fixed size: a whole grid or fixed blocks; no model whose
   input grows with the scan (section 8.2).
3. Blocks are joined by nearest centre, not nnU-Net's Gaussian blending (section 8.4).
4. One network per plugin in this version (section 2).
5. Only the standard ONNX domain; ONNX Runtime's own extra operators and local functions are
   refused too (section 7.3).
6. The network's sum is checked by the helper itself before every run (section 7.2).
7. Measured figures are kept per card name and runtime, and a new device is measured before its
   first run (section 10.3).
8. One result kept per series per plugin; plugins use labelling's **Runs on** choice and share
   its one-at-a-time turn (sections 5.3, 10.3, 11.2).
9. A matching contrast is noted on the result too (section 5.3).
10. Until the redesign lands, the plugin entries sit under the labelling strip in the Parts tab
    (section 13.1).
11. `plugin.json` in JSON, format 1, with a maker-chosen `id` (section 6).
12. How contrast names are matched: any name read that the plugin accepts is a match, "3D T1"
    counts as T1, and a description naming conflicting contrasts counts as not saying (section
    5.3).
13. Percentiles for plugins come from a fine histogram of the resampled scan, not an exact sort,
    so memory stays bounded on large scans (section 8.3).
14. The later kinds' fields are part of format 1 now and published as not yet run (section 12);
    a network with several outputs names the one to read (section 9.1).
15. The MindGrab check is functional (the mask covers the made-up brain), since Voxa's menu does
    not copy Brainchop's eight-bit conform step (section 14).
