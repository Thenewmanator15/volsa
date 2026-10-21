# Voxa

Voxa is a desktop viewer for MRI scan exports, written in Rust. It opens a folder, a zip or a
`DICOMDIR`, lists the studies and series inside and says what kind each is, and shows any series as a stack of slices with
correct orientation, contrast controls, overlays, tag inspection and cine playback; shows the
series being looked at as a 3D volume that can be turned, cut open and saved as a picture;
shows axial, coronal and sagittal planes through any point of it, on which lengths, angles and
regions can be measured and points marked; and shows the parts of the head that another program
has labelled on a scan, over the planes and as solids in 3D, with a list of them and their
volumes. Voxa shows labels; it does not make them.

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

Status: version 0.4.0, the fourth milestone (Labelled parts, from files). What it does and what
it does not yet do are in the [changelog](CHANGELOG.md); how it was checked is in
[docs/acceptance-0.4.0.md](docs/acceptance-0.4.0.md). The design and the plan for every
milestone are in [docs/superpowers](docs/superpowers). Work since 0.4.0, on reading more kinds
of MRI export, is listed under Unreleased in the changelog, and how it was checked is in
[docs/acceptance-any-mri.md](docs/acceptance-any-mri.md).

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

measures line coverage of `voxa-core`, `voxa-dicom`, `voxa-volume` and `voxa-labels` and fails
below 80% on any of them. It needs `cargo install --locked cargo-llvm-cov`.

## Layout

| Crate | Purpose |
|---|---|
| `voxa-core` | Data model, typed coordinate spaces, errors |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing, pixel decoding |
| `voxa-volume` | Volumes from series, sampling and lines of sight, planes through a volume, measuring, histograms and automatic window/level |
| `voxa-labels` | Label volumes (NIfTI) and their tables of names; which part lies at a point |
| `voxa-render` | GPU drawing of slices, of volumes, of planes through them and of labelled parts; the 3D camera |
| `voxa-app` | The application: interface, state, background work |
| `xtask` | Project tasks (`check`, `coverage`) |

Crates depend only downward: core, then dicom, volume and labels, then render, then app. See
[docs/architecture.md](docs/architecture.md) and the decision records in [docs/adr](docs/adr).

## Privacy

Voxa makes no network connections and collects nothing. Patient names and identifiers are hidden
by default in the interface and are never written to logs. Scan data never belongs in this
repository: tests build synthetic images.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Contributions are accepted under the same terms.

Voxa is built on other people's libraries, each under its own licence; `cargo deny check` holds
them to the permissive licences listed in [deny.toml](deny.toml). This software is based in part
on the work of the Independent JPEG Group.

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
