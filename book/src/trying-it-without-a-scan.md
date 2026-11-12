# Trying it without a scan

Volsa's own example writes made-up studies, which are nobody's scans. From the source (see
[Building from source](building-from-source.md)):

```
cargo run -p volsa-app --example write_phantom -- phantom
cargo run -p volsa-app -- phantom
cargo run -p volsa-app -- phantom --labels phantom/labels/labels.nii.gz
```

The first command writes a small made-up study (a ball, brighter towards the patient's left, with
a bead near the front, with a series of two volumes of it at two echo times, three locating
images and a picture in colour) to the folder `phantom`; the second opens it. The third opens it
with the made-up labels the first command wrote beside it: the ball's shell in two halves, its
core and the bead. Volsa then says that two files are not scans or documents it lists, which are
the labels and their table: they are not part of the scan. The pictures on [A closer look](a-closer-look.md) are
of this study.

## Larger made-up studies

```
cargo run --release -p volsa-app --example write_phantom -- ct some/ct [SLICES]
cargo run --release -p volsa-app --example write_phantom -- body some/body [STATIONS]
```

write a made-up body from the thighs to the head, for trying how Volsa copes with very large
scans: `ct` as one CT series of thin slices (512 x 512, 2,000 slices unless a number is given),
`body` as a whole-body MR study taken in overlapping stations (five unless a number is given),
each a series, beside the stations joined into one.

## A made-up head, for labelling

The ball has no brain in it, so there is nothing for the labelling model to find. For something
with a brain's shape that is nobody's, the model folder's `check` folder holds a made-up head,
painted from a label map that comes with SynthSeg, and `write_phantom` can write it as a study:

```
cargo run -p volsa-app --example write_phantom -- head path/to/model-folder/check head
cargo run -p volsa-app --example write_phantom -- head path/to/model-folder/check head-whole whole
```

The second puts the head in the middle of a grid of air as large as a scan of a whole head
(176 x 256 x 256 voxels). With `VOLSA_MODELS` and `VOLSA_RUNTIME` set as on
[Building from source](building-from-source.md),

```
cargo run --release -p volsa-app --example check_labelling -- head [standard|robust] [no-areas]
```

labels the largest series that has a 3D view, with no window, on every place Volsa would try
(each discrete graphics card, then the processor, whole and then cut to the head) and prints
for each the number of voxels the networks were given, the place, the seconds, the number of
parts and whether the model's own check passed, and nothing else.

## Pictures of the window

Setting the environment variable `VOLSA_CAPTURE` to a `.png` path makes Volsa save one picture of
its own window and close, which is how the pictures in the documentation are made.

- `VOLSA_CAPTURE_SIZE` sets the window's size in points, such as `900x600`. A picture's window is
  otherwise the size asked for, or 1440 by 900 points.
- `VOLSA_CAPTURE_ZOOM` sets the size the interface is drawn at, 1 being its usual size, so that
  one display can show what another would: on a display of 1440 by 900 points at 200%, `0.75`
  with `VOLSA_CAPTURE_SIZE=1920x1010` shows the window as a screen of 1920 by 1080 pixels at 100%
  would, and `0.375` with `3840x2090` as a 4K one would.
- `VOLSA_CAPTURE_SERIES` names the series to show by words from its description.
- `VOLSA_CAPTURE_SHOW` names what else to have in the picture, separated by commas: `stepped`
  shows the next volume of a series that holds several, `keys` the list of keys, `nolist` folds
  the rail of scans, `other` opens the other files in it, `light` draws the window in the light
  theme, in place of the dark one every picture is otherwise made in, `preview` takes the 3D
  view's preview of a long series rather than waiting for full detail, and `fitted` fits the
  window to the screen as Volsa does when it starts, in place of the size asked for. Others
  name a tab (`tab-view` and its kin), an arrangement of the pictures (`planes`, `3d`,
  `slices3d`), a window or list open over them (`search`, `notices`) and a pose of the 3D scene;
  the whole list is in `crates/volsa-app/src/ui/capture.rs`.
- `VOLSA_ADAPTER=software` has the window draw with the system's software graphics, as on a
  computer with no usable graphics card.
