# Building from source

This page is for people who want to build Volsa or work on it.
[CONTRIBUTING.md](https://github.com/Thenewmanator15/volsa/blob/main/CONTRIBUTING.md) says how
changes are made.

## Build and run

Volsa needs the Rust toolchain named in `rust-toolchain.toml`; `rustup` installs it on first use.
On Windows the build also gives `volsa.exe` its icon and the details Explorer shows, with the
resource compiler (`rc.exe`) of the Windows SDK that the Visual C++ build tools Rust uses come
with; without it the program is built without them, and cargo warns.

```bash
cargo run --release -p volsa-app -- path/to/export.zip
```

To show labels made for that scan by another program, name the label file too, or open it from
the Labels tab, or drop it on the window:

```bash
cargo run --release -p volsa-app -- path/to/export.zip --labels path/to/labels.nii.gz
```

To label a brain scan in a build of your own, put the `runtime` and `models` folders beside the
executable the build makes, or name them under **Options**, as
[docs/models.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/models.md) says.

## The release zip

```bash
cargo xtask dist
```

builds the release program and makes `target/dist/volsa-<version>-windows-x64.zip`, the zip the
Releases page offers, with the folder it holds beside it. The program is built with Microsoft's
C runtime inside it, so that it needs no Visual C++ redistributable of its own, and the task
checks that it imports none, and that it records no path of the computer it was built on.
`cargo xtask dist --runtime FOLDER` puts ONNX Runtime's two libraries from that folder in the
zip's `runtime` folder, with its `LICENSE` and `ThirdPartyNotices.txt`; a folder without them is
refused, unless `--local` is given for a package that is not for release, whose zip's name then
ends `-windows-x64-local.zip`. A tag that starts with `v` has GitHub make a draft release with
that zip, its SHA-256 and the version's section of the changelog
(`.github/workflows/release.yml`), with ONNX Runtime taken from Microsoft's package on nuget.org.

## Check your work

```bash
cargo xtask check
```

runs, in order: formatting, lints, the 400-line limit on source files, that
`THIRD-PARTY-NOTICES.txt` is current (`cargo xtask notices` rewrites it), every test, licence and
advisory checks (`cargo deny`), and the documentation build. Every test program runs even when
one fails, so all failures are reported together; any other failing step stops it there.
`cargo deny` is installed with `cargo install --locked cargo-deny`. Among the tests, the words
of the README, the zip's `README.txt` and these wiki pages are held to the rule the window's
words are held to (no word that reads as a verdict on anyone's body), the statement of what
Volsa is for is held to `docs/intended-use.md`, every link between wiki pages must lead to a
page in `wiki/`, and every link into the repository to a file in it.

```bash
cargo xtask coverage
```

measures line coverage of `volsa-core`, `volsa-dicom`, `volsa-volume`, `volsa-labels`,
`volsa-models` and `volsa-segment` and fails below 80% on any of them. The two files that need
the runtime or a graphics card, `runtime.rs` and `cards.rs`, are left out of the count. It
needs `cargo install --locked cargo-llvm-cov`.

```bash
VOLSA_MODELS=path/to/model-folder VOLSA_RUNTIME=path/to/runtime-folder cargo xtask check-models
```

holds the labelling of a made-up head against SynthSeg's own answers, on the processor and on
each graphics card, with the real networks. It runs the tests that the other two leave out,
which need a model folder and the runtime; without both variables it does nothing and says so.
`cargo xtask check` needs neither on the machine. It passes in full: the `zip` crate is at
version 9, past three advisories about extracting archives (Volsa never extracts one), and
`deny.toml` sets aside three notices that crates Volsa uses only for accessibility on Linux are
no longer maintained, each with its reason. GitHub runs it on Windows and on Linux.

## Layout

| Crate | Purpose |
|---|---|
| `volsa-core` | Data model, typed coordinate spaces, errors |
| `volsa-dicom` | Sources (folder, zip, DICOMDIR), header indexing, pixel decoding |
| `volsa-volume` | Volumes from series, sampling and lines of sight, planes through a volume, measuring, histograms and automatic window/level |
| `volsa-labels` | Label volumes (NIfTI) and their tables of names, read and written; which part lies at a point |
| `volsa-models` | The networks: looking for and checking a model folder, running a network through ONNX Runtime, the graphics cards and what a run needs of the computer |
| `volsa-segment` | SynthSeg's own steps around its networks: preparing a scan, the steps between networks, tidying the answer into labels with the model's verdict |
| `volsa-render` | GPU drawing of slices, of volumes, of planes through them and of labelled parts; the 3D camera |
| `volsa-app` | The application, `volsa.exe`: interface, state, background work |
| `xtask` | Project tasks (`check`, `coverage`, `check-models`, `dist`) |

Beside the crates, `tools/synthseg-to-onnx` turns SynthSeg's files into a model folder (Python,
run once by a person; cargo neither builds nor runs it), `tools/golden` makes the arrays that
SynthSeg's own program gives for small made-up volumes, which the tests hold Volsa's steps to,
and `tools/fixtures` makes a small made-up JPEG 2000 picture for the decoding test. `wiki/`
holds these pages.

Crates depend only downward: core, then dicom, volume, labels and models, then segment and
render, then app. See
[docs/architecture.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/architecture.md)
and the decision records in [docs/adr](https://github.com/Thenewmanator15/volsa/tree/main/docs/adr).

Scan data never belongs in this repository: tests build synthetic images. To try Volsa with no
scan of your own, see [[Trying it without a scan]].
