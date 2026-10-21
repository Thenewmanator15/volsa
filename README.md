# Voxa

Voxa is a desktop viewer for MRI scan exports, written in Rust. It opens a folder, a zip or a
`DICOMDIR`, lists the studies and series inside and says what kind each is, and shows any series as a stack of slices with
correct orientation, contrast controls, overlays, tag inspection and cine playback; shows the
series being looked at as a 3D volume that can be turned, cut open and saved as a picture;
shows axial, coronal and sagittal planes through any point of it, on which lengths, angles and
regions can be measured and points marked; and shows the parts of the head that have been
labelled on a scan, over the planes and as solids in 3D, with a list of them and their
volumes.

Voxa shows two kinds of labels, and says which is which wherever it draws them. Labels made by
another program are read from a file and shown as made elsewhere; Voxa does not vouch for
them. Labels of the structures of a brain, and of the areas of its cortex, Voxa makes itself
on this computer when asked, with SynthSeg 2.0's networks run through ONNX Runtime and, on
Windows, DirectML. A label made by a model is a guess that looks certain, so each one comes
with the model's own check of its work, which is said first when it fails. Voxa ships no
network and no runtime: [docs/models.md](docs/models.md) says what to provide and why.

**Voxa is not a medical device and is not for diagnostic use.**

![Voxa showing a made-up test study in three planes and in 3D, with measurements](docs/images/voxa-phantom-planes.png)

The pictures show a made-up study that Voxa's own example writes, not a scan of anyone. Above:
a ball in its axial, sagittal and coronal planes and in 3D, with a ruler, an angle, a region and
a marker measured on it, and the slice through its middle beside them. Below: the same ball
turned and cut open in the 3D scene.

![Voxa showing the same study in 3D, cut open, with the slice shown beside it](docs/images/voxa-phantom-3d.png)

![Voxa showing the made-up study with made-up labels of its parts, over the planes and as solids](docs/images/voxa-phantom-labels.png)

Last: the same ball with the made-up labels that come with it, tinted over the three planes
and drawn as solids in 3D, with the list of its parts. The left half of its shell is selected,
and the list sets it against the right half. The two halves were drawn equal: the 2% between
them is where the labels' own coarse voxels happen to fall, which is the kind of difference
Voxa says it cannot tell from noise.

![Voxa listing a series of each kind in the made-up study, showing the second of two volumes of one](docs/images/voxa-phantom-kinds.png)

An export holds more than plain stacks of slices, and the list says what each series is. Here
the made-up study also holds a series of two volumes of the ball, taken at two echo times,
listed once and shown on its second volume, as the toolbar and the viewport say; three
locating images in three planes; and a picture in colour, which is shown in its own colours.

Status: version 0.5.0, which finishes the fourth milestone: 0.4.0 showed labels from files, and
0.5.0 labels a brain scan inside Voxa. It also reads more kinds of MRI export. What it does and
what it does not yet do are in the [changelog](CHANGELOG.md); how it was checked is in
[docs/acceptance-0.5.0.md](docs/acceptance-0.5.0.md), and for the reading of more kinds of
export in [docs/acceptance-any-mri.md](docs/acceptance-any-mri.md). The design and the plan for
every milestone are in [docs/superpowers](docs/superpowers), and why labelling is built as it
is in [docs/adr/0006-voxa-runs-a-labelling-model.md](docs/adr/0006-voxa-runs-a-labelling-model.md).

## Build and run

Voxa needs the Rust toolchain named in `rust-toolchain.toml`; `rustup` installs it on first use.

```bash
cargo run --release -p voxa-app -- path/to/export.zip
```

To show labels made for that scan by another program, name the label file too, or open it from
the Parts tab, or drop it on the window:

```bash
cargo run --release -p voxa-app -- path/to/export.zip --labels path/to/labels.nii.gz
```

To have Voxa label a brain scan itself, open the scan, choose the Parts tab and press **Label
this scan**. That needs two things Voxa does not come with: the three files of ONNX Runtime
with DirectML in a `runtime` folder beside the executable, and a model folder made from
SynthSeg's files in a `models` folder beside it (or the folders named under **Options**).
[docs/models.md](docs/models.md) says where they come from and how to make them. Without them
Voxa starts and does everything else, and the button says what is missing.

## Check your work

```bash
cargo xtask check
```

runs, in order: formatting, lints, the 400-line limit on source files, every test, licence and
advisory checks (`cargo deny`), and the documentation build. It stops at the first failure.
`cargo deny` is installed with `cargo install --locked cargo-deny`.

```bash
cargo xtask coverage
```

measures line coverage of `voxa-core`, `voxa-dicom`, `voxa-volume`, `voxa-labels`,
`voxa-models` and `voxa-segment` and fails below 80% on any of them. The two files that need
the runtime or a graphics card, `runtime.rs` and `cards.rs`, are left out of the count. It
needs `cargo install --locked cargo-llvm-cov`.

```bash
VOXA_MODELS=path/to/model-folder VOXA_RUNTIME=path/to/runtime-folder cargo xtask check-models
```

holds the labelling of a made-up head against SynthSeg's own answers, on the processor and on
each graphics card, with the real networks. It runs the tests that the other two leave out,
which need a model folder and the runtime; without both variables it does nothing and says so.
`cargo xtask check` passes with neither on the machine.

## Layout

| Crate | Purpose |
|---|---|
| `voxa-core` | Data model, typed coordinate spaces, errors |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing, pixel decoding |
| `voxa-volume` | Volumes from series, sampling and lines of sight, planes through a volume, measuring, histograms and automatic window/level |
| `voxa-labels` | Label volumes (NIfTI) and their tables of names, read and written; which part lies at a point |
| `voxa-models` | The networks: finding and checking a model folder, running a network through ONNX Runtime, the graphics cards and what a run needs of the computer |
| `voxa-segment` | SynthSeg's own steps around its networks: preparing a scan, the steps between networks, tidying the answer into labels with the model's verdict |
| `voxa-render` | GPU drawing of slices, of volumes, of planes through them and of labelled parts; the 3D camera |
| `voxa-app` | The application: interface, state, background work |
| `xtask` | Project tasks (`check`, `coverage`, `check-models`) |

Beside the crates, `tools/synthseg-to-onnx` turns SynthSeg's files into a model folder (Python,
run once by a person; cargo neither builds nor runs it), `tools/golden` makes the arrays that
SynthSeg's own program gives for small made-up volumes, which the tests hold Voxa's steps to,
and `tools/fixtures` makes a small made-up JPEG 2000 picture for the decoding test.

Crates depend only downward: core, then dicom, volume, labels and models, then segment and
render, then app. See [docs/architecture.md](docs/architecture.md) and the decision records in
[docs/adr](docs/adr).

## Privacy

Voxa makes no network connections and collects nothing. Patient names and identifiers are hidden
by default in the interface and are never written to logs. Scan data never belongs in this
repository: tests build synthetic images.

ONNX Runtime, which runs the labelling networks, reports usage (not images) in Microsoft's
Windows builds unless told not to. Voxa tells it not to before it asks anything else of it. A
labelling result is kept on this computer, in the folder the system gives Voxa for its
settings, so that it can be shown again when the same scan is opened; it is a label file and a
table of names filed under a hash of the series, and holds nothing that says whose scan it
was. **Options** has a switch that keeps nothing, and **Delete** removes a kept result.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Contributions are accepted under the same terms.

Voxa is built on other people's libraries, each under its own licence; `cargo deny check` holds
them to the permissive licences listed in [deny.toml](deny.toml). This software is based in part
on the work of the Independent JPEG Group.

Labelling uses three things that are not in this repository, each under its own licence:

- **ONNX Runtime** (Microsoft), which runs the networks, under the MIT licence. Voxa loads it
  from files beside the executable through the `ort` crate (MIT or Apache-2.0); it is not built
  into Voxa.
- **DirectML** (Microsoft), which lets ONNX Runtime use a graphics card on Windows, under
  Microsoft's own licence: it may be passed on inside an application for use on Windows, it is
  not open source, and its notices must stay with it. Windows 11 has a copy of its own, and
  Voxa carries none.
- **SynthSeg 2.0** (Billot et al.), the model. Its code is Apache 2.0. Its trained networks
  carry no licence statement of their own and are distributed with FreeSurfer, under
  FreeSurfer's terms, so Voxa ships none and fetches none: you get SynthSeg's files under
  the terms they come with and turn them into a model folder on your own computer.

The typefaces in the interface come with the `egui` library: Ubuntu Light (Ubuntu Font Licence
1.0), Hack (MIT) and Noto Emoji (SIL Open Font Licence 1.1). No other font files are in this
repository.

## Trying it without a scan

```
cargo run -p voxa-app --example write_phantom -- phantom
cargo run -p voxa-app -- phantom
cargo run -p voxa-app -- phantom --labels phantom/labels/labels.nii.gz
```

The first command writes a small made-up study (a ball, brighter towards the patient's left, with
a bead near the front, with a series of two volumes of it at two echo times, three locating
images and a picture in colour) to the folder `phantom`; the second opens it. The third opens it with the
made-up labels the first command wrote beside it: the ball's shell in two halves, its core and
the bead. Voxa then says it skipped two files, which are the labels and their table: they are
not part of the scan. Setting the environment
variable `VOXA_CAPTURE` to a `.png` path makes Voxa save one picture of its own window and close,
which is how the pictures in the documentation are made. `VOXA_CAPTURE_SERIES` names the series
to show by words from its description, and the word `stepped` in `VOXA_CAPTURE_SHOW` shows the
next volume of a series that holds several.

The ball has no brain in it, so there is nothing for the labelling model to find. For something
with a brain's shape that is nobody's, the model folder's `check` folder holds a made-up head,
painted from a label map that comes with SynthSeg, and `write_phantom` can write it as a study:

```
cargo run -p voxa-app --example write_phantom -- head path/to/model-folder/check head
cargo run -p voxa-app --example write_phantom -- head path/to/model-folder/check head-whole whole
```

The second puts the head in the middle of a grid of air as large as a scan of a whole head
(176 x 256 x 256 voxels). With `VOXA_MODELS` and `VOXA_RUNTIME` set as above,

```
cargo run --release -p voxa-app --example check_labelling -- head [standard|robust] [no-areas]
```

labels the largest series that has a 3D view, with no window, on every place Voxa would try
(each discrete graphics card, then the processor, whole and then cut to the head) and prints
for each the number of voxels the networks were given, the place, the seconds, the number of
parts and whether the model's own check passed, and nothing else.
