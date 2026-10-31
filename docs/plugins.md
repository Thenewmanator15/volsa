# Plugins: models you add yourself

Voxa can run a model that someone else made, if you add it. A plugin is a folder
holding the model's network as an ONNX file and a `plugin.json` that tells Voxa how to prepare a
scan for it and what its answer means. This page is for people who make such a folder, and for
anyone who wants to know exactly what Voxa does with one.

**A plugin is not part of Voxa.** Voxa has not checked what any plugin does, recommends none and
links to none. Whatever a plugin gives is shown under "Added by you, not part of Voxa" and is
not for diagnostic use. Add plugins only from sources you trust: a network file is a program of
a kind, and Voxa runs it with your own rights ([SECURITY.md](../SECURITY.md), "Plugins").

Voxa never downloads a plugin. Its licence is its maker's, in the maker's own words; whether you
may use it, and for what, is between you and them.

## What a person does

1. In the Parts tab, under **Added by you**, press **Add a plugin…** and choose the folder.
2. Voxa checks the folder (below). If a check fails, one sentence says which, and nothing is
   copied.
3. A card shows Voxa's own words first, then what the plugin says of itself, then every setting
   Voxa will prepare a scan with, in plain words. Press **Add** or **Cancel**. A plugin with the
   same `id` as one already added names the version it replaces.
4. Voxa copies the listed files into its own folder and runs the network once on a made-up
   input, on the processor and on each graphics card with memory of its own, to measure what it
   needs. A plugin that cannot run on the processor is not added.
5. Open a scan, and press **Run** in the plugin's entry. A scan of a modality the plugin does
   not list is refused. When the scan's description names a contrast the plugin does not list,
   Voxa says so and runs only if you press **Run anyway**; the result then says it was run after
   a warning. When the description names no contrast, the result says "Contrast not confirmed".

The result opens as labels, with the banner first wherever it is shown or written: the Parts
tab, the captions of the plane and 3D views, the kept files, a table saved with **Save as**, and
the parts exported as CSV. **Show result** opens the result kept for the scan again, **Delete
result** deletes it, and **Remove** (asked first) deletes Voxa's copy of the plugin; results it
made are kept.

## The folder

A plugin folder holds `plugin.json` and the files it lists. Nothing else in it is read or
copied. Voxa copies the listed files into a folder of its own, beside the results it keeps,
named by the first 16 hexadecimal digits of the SHA-256 of `plugin.json`, and checks the copy
again. The folder you added it from is never touched.

## `plugin.json`, format 1

JSON, at most 1 MiB. Every setting that changes the answer must be stated: there are no
defaults for them, and a field missing, unknown, or holding a value Voxa does not know is
refused with the field named. "Printable" text has no control or format character (no line
break, no right-to-left mark).

| Field | What it holds | Checked |
|---|---|---|
| `format` | `1` | a format Voxa knows |
| `kind` | `"label-map"`; the later kinds `"number"`, `"value-map"`, `"alignment"` are read and checked but **not run by this version yet** | a kind Voxa has a routine for |
| `id` | an identity of your choosing that stays the same across versions, e.g. `example.skull-strip` | 3 to 64 of `a-z`, `0-9`, `.` and `-` |
| `name`, `version` | shown | printable, at most 60 and 20 characters |
| `made_by`, `source`, `licence` | shown; `source` as text, never as a link | printable, at most 80, 200 and 200 characters |
| `about` | one sentence of what it does | printable, at most 300 characters |
| `limits` | up to 8 sentences you want shown with every result | printable, each at most 300 characters |
| `files` | `[{ "name", "bytes", "sha256" }]` | plain names; each at most 2 GiB, all together at most 4 GiB |
| `network` | the name of the `.onnx` file among `files` | listed |
| `accepts.modality` | DICOM modality codes, e.g. `["MR"]` | at least one |
| `accepts.contrast` | `"any"`, or a list of the words below, e.g. `["T1"]` | words Voxa knows |
| `accepts.body_part` | shown only, e.g. `"head"` | printable, at most 60 characters |
| `accepts.thickest_mm` | optional: the widest voxel spacing the model was made for | a number above 0 |
| `input` | how a scan is prepared ([the menu](#preparation-the-menu)) | every field present and valid |
| `output` | what the network gives and what it means ([below](#what-the-network-gives-output)) | every field present and valid |

The contrast words are `T1`, `3D T1`, `T2`, `PD`, `FLAIR`, `T2*`, `Diffusion`, `ADC map`, `SWI`,
`STIR` and `Angiography (TOF)` (or `TOF`). Voxa reads the scan's own series description for
them as the series list does: "3D T1" counts as T1, and a description that names two contrasts
that cannot both be true counts as saying none.

### An example

Made up for this page; the sum is not a real file's.

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
  "files": [{ "name": "strip.onnx", "bytes": 612345, "sha256": "…64 hexadecimal digits…" }],
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

## Preparation: the menu

A scan is turned into what the network sees in these steps, each chosen by `input`. Voxa never
rotates a scan and never writes any of its values to disk.

### The grid: `spacing_mm`, `axes`, `layout`

- `axes`: three letters, one from each of L/R, A/P and S/I, e.g. `RAS` or `LIA`. Letter *n* is
  the direction in the patient that the *n*-th spatial dimension of the input points to as its
  index grows. The spatial dimensions are in the order of the tensor's shape, the last changing
  fastest, as ONNX stores tensors. Voxa takes the scan's own axes, gives each letter the axis
  whose direction is nearest to it, and turns that axis round where it runs the other way: the
  grid stays along the scan's own axes, as SynthSeg's does.
- `spacing_mm`: the distance between voxels along each spatial dimension, in the tensor's
  order. Resampling along each of the scan's own axes is linear.
- `layout`: `"channels-first"`, an input of `[1, 1, d0, d1, d2]`; or `"channels-last"`,
  `[1, d0, d1, d2, 1]`. The output has the same layout.

### Fitting: `fit`

Every input the network is given has a fixed size, so the memory measured once holds for every
scan.

- `{ "whole": [n0, n1, n2], "outside": … }`: one box of that size, centred on the centre of the
  scan's grid. A scan larger than the box along an axis is cut to it, and the result says how
  many millimetres were left out; a smaller one is surrounded with the `outside` value.
- `{ "blocks": { "size": [b0, b1, b2], "step": [s0, s1, s2] }, "outside": … }`: the whole grid
  is covered by blocks of that size, `step` voxels apart (each step from 1 to the block's size),
  the last block of each row laid against the far end, as nnU-Net lays its blocks. Each voxel
  takes its label from the block whose centre is nearest to it, the earlier block when two are
  as near. nnU-Net instead adds the blocks' scores with a Gaussian weight, so near the edges
  between blocks the two can differ; a result made in blocks says so.
- `outside`: `"lowest"`, the scan's lowest value before scaling (so a CT is surrounded by air),
  or `"zero"`, 0 after scaling.

### Brightness: `scaling`

- `{ "kind": "percentiles", "low": p, "high": q, "over": … }`: the value at the `p`-th
  percentile becomes 0 and the `q`-th becomes 1, everything below and above held at 0 and 1.
- `{ "kind": "z-score", "voxels": "all" | "above-zero", "over": … }`: the mean is taken away and
  the result divided by the standard deviation, both of the voxels named.
- `{ "kind": "fixed", "clip": [a, b], "subtract": m, "divide": s }`: values held within
  `[a, b]`, then `(x − m) / s`. A CT's fixed z-score, or a fixed range (subtract `a`, divide by
  `b − a`), is written this way.
- `{ "kind": "none" }`: the scan's own values, after the file's rescale; for a model that scales
  inside itself.
- `over`, required for percentiles and z-score: `"scan"`, every voxel of the scan resampled to
  the plugin's spacing and turned, before any cut or surroundings; or `"grid"`, the fitted whole
  box, surroundings included. `"grid"` is allowed only with `whole`.

How exact the figures are: means and standard deviations are summed in 64-bit numbers.
Percentiles are read from a histogram of 65 536 equal bins between the lowest and the highest
resampled value, interpolating within the bin, so each is within 1/65 536 of the range of its
exact value. (SynthSeg's own labelling keeps its exact percentiles; this is for plugins only.) A
scan whose values are all the same is refused: there is nothing in it for a model to see.

## What the network gives: `output`

- `"gives": "scores"` with `"channels": [...]`: one channel per entry; each voxel takes the
  channel with the largest score, the first when several are equal, and that channel's label
  from `channels` (a label of 0 is nothing).
- `"gives": "chance"` with `"label"` and `"threshold"`: one channel; a voxel takes `label` where
  the number is above `threshold`.
- `"gives": "labels"`: a label for each voxel, as 8-, 32- or 64-bit integers. A label that is
  neither 0 nor named stops the run: "The plugin gave labels its description does not name."
- `"from"`: the name of the network output to read; required only when the network has more
  than one.
- `"keep_largest_piece"`: `true` keeps only the largest connected piece of all labelled voxels
  together, `false` keeps every piece. Required.
- `"names"`: for each label that can appear (1 to 65 535), its `id`, `name`, `side` (`"left"`,
  `"right"` or `"midline"`) and `group`, and optionally a `colour` of three numbers from 0 to
  255. Every label the network can give must be named, and no two entries may share an `id`, or
  a name and side.

Every number a network gives must be finite, and every answer of the size expected; anything
else stops the run with a sentence. The labels are put back on the scan as SynthSeg's are: the
label grid lies at the plugin's spacing along the scan's own axes, never resampled to the scan's
grid. A voxel outside a whole box has no label. An answer with nothing labelled says so.

### The later kinds, not run by this version yet

Their fields are part of format 1, so a plugin written now will not need changing when they are
built. This version reads and checks them, then refuses the kind with "This version of Voxa
cannot run plugins that give one number yet." (or a value map, or an alignment).

- `"kind": "number"`: the network gives one 32-bit number. `output` has `name`, `decimals` (0 to
  6), and optionally `unit`, `range` (`[low, high]`, where its maker says it means something)
  and `from`. Whole box only.
- `"kind": "value-map"`: one channel of 32-bit numbers on the grid. `output` has `range` (the
  range to draw), and optionally `unit` and `from`.
- `"kind": "alignment"`: two scans, the fixed one first, each prepared by the same `input` onto
  its own box. `output` has `"gives": "field"` with `"unit": "mm" | "voxels"` (displacements of
  shape `[1, 3, d0, d1, d2]`), or `"gives": "matrix"` (4 × 4); and optionally `from`. Whole box
  only.

## The checks

When a folder is offered, in this order; the first that fails is said in a sentence, and nothing
is copied:

1. The folder exists and holds `plugin.json`, no longer than 1 MiB.
2. `format` is known, and `kind` is one this version runs.
3. Every field above is present and valid.
4. Every listed file has a plain name (not empty, no separator, no `..`, no `:`, no NUL, and no
   two names that differ only in case), is a regular file and not a link, and has the size and
   SHA-256 listed.
5. The network file passes the reading below.
6. The network's image input and the output read from it fit `input` and `output`: element
   types, the number of dimensions, every fixed dimension equal to the description's, and every
   free dimension one Voxa will fix by name.

Voxa reads the ONNX file's structure itself, skipping over its weights, and refuses it when:

- it imports any operator set other than the standard one (`""` or `ai.onnx`), ONNX Runtime's
  own `com.microsoft` included, or a version newer than the runtime Voxa uses (IR 13, opset 25);
- any node, in the main graph or in a subgraph at any depth, names another domain;
- it holds local functions;
- any tensor's data lies outside the file somewhere other than a listed file, or past that
  file's end;
- subgraphs nest more than 16 deep, there are more than 100 000 nodes, or a length runs past
  the end of the file.

A network's own metadata, `doc_string` and producer are never shown and never followed.

Before every run, the helper process that runs the network checks the network file's SHA-256
again before it opens it, and `plugin.json` must still have the sum its folder is named by. A
plugin whose files changed after it was added says: "This plugin's files have changed since it
was added. Remove it and add it again."

## How a plugin runs

Networks run in a helper, a second copy of `voxa.exe` started with `--plugin-helper`, never in
Voxa itself. The helper is held by Windows in a job object that ends it when Voxa ends or when
**Stop** is pressed, lets it start no program of its own, and caps its memory at what it was
measured to need with a quarter added. Scan values pass to it through pipes, a block at a time,
and its labels come back the same way; nothing is written to disk. If it fails or ends early,
Voxa says so and goes on.

The memory a plugin needs is measured once, when it is added, and again on a card or a runtime
it was not measured with, before that first run. A run goes where **Runs on** in labelling's
**Options** says, and only where the measured need fits; a plugin run and a labelling run never
run at the same time.

## How to check a plugin you made

- Run your model's own program and Voxa on the same scans you are allowed to use, and compare
  the labels where they are both defined. Near the edges between blocks they may differ if your
  program blends blocks.
- Try a scan stored the other way round, and a tilted one: a label on the wrong side means
  `axes` is wrong.
- Try a scan with thicker slices than your model was made for, and say so in `limits` and
  `accepts.thickest_mm`.
- Never put a real scan, or anything made from one, in a plugin folder or in a report about one.
