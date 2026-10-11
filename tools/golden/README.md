# golden

Makes the arrays Voxa's tests are held against, with SynthSeg's own Python. Voxa does in Rust
what SynthSeg does around its networks, and "the same as SynthSeg" has to mean something that
can be checked: these arrays are what SynthSeg's own functions make of small made-up volumes,
and the tests of `voxa-segment` compare Voxa's answer with them.

It is a tool for a person to run when a case is added or changed; it is no part of Voxa, and
cargo neither builds, runs nor tests it. Nothing here comes from a scan: every volume is made
up from a seed.

Voxa is not a medical device and is not for diagnostic use, and neither is anything made here.

## What it needs

- A copy of [SynthSeg](https://github.com/BBillot/SynthSeg). No weights are needed: the steps
  made here use none, and no network is run.
- The Python environment of `tools/synthseg-to-onnx` (its `requirements.txt`). On NumPy 2,
  SynthSeg's own code needs the few one-line fixes that README speaks of.

## Running it

```
python make_golden.py PATH\TO\SynthSeg
python make_golden.py PATH\TO\SynthSeg prepare
python make_golden.py PATH\TO\SynthSeg box
python make_golden.py PATH\TO\SynthSeg tidy
```

The first makes every group of cases, the others only the group named. Each group is written
under `crates/voxa-segment/tests/golden/<group>/` of the repository this script is in. It
takes under a minute, most of it Python starting.

A second run writes the same bytes, as long as the libraries are the versions that made the
files before; every `case.json` says which those were (NumPy, SciPy and nibabel for
`prepare`; NumPy and TensorFlow for `box`; NumPy and SciPy for `tidy`). Each case has a seed
of its own, made from its group's name and its own, so adding a case changes no other.
Commit what the script writes together with the change to the script.

## How to read what it writes

Each case is a folder holding `case.json` and arrays. An array has no header: a file named
`.f32` is raw little-endian 32-bit floats, one named `.i32` is little-endian 32-bit whole
numbers, and one named `.u8` is one byte a number. `case.json` gives its size and repeats its
order.

- **A volume** (`volume.f32`, `resampled.f32`) is in the order Voxa holds one, which is also
  a NIfTI file's: columns fastest, then rows, then slices. With `dims` as
  `[cols, rows, slices]`, the value at a column, row and slice is
  `values[(slice * rows + row) * cols + col]`.
- **An image** (`image.f32`) is in the order a network takes one: `[x, y, z]` with z fastest,
  x running to the patient's right, y to the front and z to the top. With `size` as
  `[size_x, size_y, size_z]`, the value at x, y, z is `values[(x * size_y + y) * size_z + z]`.
  It is stored without its padding, to keep the files small: the network's image is `padded`
  voxels along each axis, zeros everywhere but in a block of `size` that begins
  `padding_before` voxels in. The script checks that this is so before it writes.
- **A transform** (`to_patient`) takes a voxel (column, row, slice) to the patient in
  left-posterior-superior millimetres, as Voxa holds it: `columns` is where a step along the
  columns, the rows and the slices leads, and `origin` where voxel (0, 0, 0) lies. SynthSeg
  works in right-anterior-superior: it was given the same transform with the x and y rows
  negated, and nothing else differs. The transform a case starts from holds 32-bit floats,
  as the NIfTI file SynthSeg read held them. Take each number as a 32-bit float and you have
  SynthSeg's transform to the last bit, which matters: one bit can change how many samples
  SynthSeg takes. The transform of a resampled volume is SynthSeg's result, in double
  precision.

## The group `prepare`

What SynthSeg makes of a volume before a network sees it: resampled to 1 mm, its axes turned,
its brightness scaled, its sides padded. Each volume is blobs and noise, under 40 voxels a
side. It is written to a NIfTI file in a temporary folder and given to SynthSeg's own
`preprocess` (in `SynthSeg/predict_synthseg.py`), called as SynthSeg's `predict` calls it:
nothing cropped, sides padded to at least 128.

Every case has eleven or more slices as stored, and eleven or more voxels from bottom to top
once it is at 1 mm and turned. On fewer of either, `preprocess` stops (it takes a last side of
ten or fewer for the channels of a flat picture), so no case can be made of such a volume.
Voxa refuses one for the same reason.

| Case | What it is |
|---|---|
| `iso` | 1 mm, stored as SynthSeg wants it |
| `sagittal` | 1 mm, another order of axes, the slices running to the left |
| `coronal` | 1 mm, another order of axes, the slices running to the back |
| `tilted` | 1 mm, turned 10 degrees about x |
| `thick` | 1 x 1 x 4 mm, stored as a scanner stores axial slices |
| `fine` | 0.75 mm, columns and rows changed over |
| `uneven` | 0.28125 x 1.08 x 2.7 mm, tilted |

`uneven` is not one of a scanner's sizes. It is there for two habits of SynthSeg's code that
the round numbers of the others do not show. Along its columns SynthSeg takes one sample more
than the spacing times the count, rounded up; the script stops if that is no longer so. And
its rows are a little over 1 mm apart, where SynthSeg does not blur and a blur made by
mistake would show.

In each case folder:

| File | What it holds |
|---|---|
| `volume.f32` | The made-up volume |
| `resampled.f32` | Only where SynthSeg resamples: the volume at 1 mm along its own axes, from SynthSeg's `resample_volume` |
| `image.f32` | What `preprocess` gives, without its padding |
| `case.json` | `volume` and `resampled`: `dims` and `to_patient`. `image`: `size`, `padded`, `padding_before`, and how the axes were turned: `order` (for each of x, y and z, the volume's axis it runs along: 0 columns, 1 rows, 2 slices) and `turned_round` (whether it runs the other way) |

`turns.json` holds twelve made-up transforms with no volume, and for each the `order` and
`turned_round` SynthSeg gives: unequal steps, slanted axes, and several where two of
SynthSeg's choices fall on the same axis at first and its code mends them.

`order` is the answer of SynthSeg's `get_ras_axes`. `turned_round` is read off the transform
as SynthSeg's `align_volume_to_ref` reads it, and the script checks both against what that
function does to a block of numbers.

## The group `box`

What SynthSeg cuts from a map of top structures for its scores network: a box of one fixed
size, laid so that what was found lies in its middle. Each map is given to SynthSeg's own
layer `MakeShape` (in `SynthSeg/predict_synthseg.py`), fed as SynthSeg's `build_model` feeds
it, for a box of 8 voxels a side. SynthSeg's own box is 224 a side; the rule is the same and
the files are small.

A map holds, for each voxel, the place of its top structure among 33: a number from 0 to 32.
Outside the blob a map is place 0 with a sprinkling of place 24. Those are the two places
the layer leaves out when it looks for where the structures lie, so they must not move the
box; they are still copied into it. The blob is a ball of the other places.

| Case | What it is |
|---|---|
| `middle` | A blob in the middle of the grid. The box is cut out round it, with one voxel more before the blob than after it where the room to share is odd |
| `corner` | A blob in a corner: at the start of x and at the end of y and z. The box runs out of the grid on three sides and is 0 there |
| `longer` | A blob longer than the box along x and z. The box keeps the blob's middle, with one voxel more cut from its end than from its start where what hangs over is odd |
| `none` | No blob. The box is the start of the grid along x and z. Along y the grid is shorter than the box and lies in its middle |

In each case folder:

| File | What it holds |
|---|---|
| `places.u8` | The made-up map, in the order of an image: `[x, y, z]` with z fastest |
| `box.u8` | What `MakeShape` gives: a cube of places, z fastest, 0 where the box lies outside the grid |
| `case.json` | `places`: the grid's `size` along x, y and z, and how many `structures` a place is one of. `box`: its `side` |

SynthSeg turns each place of the box into its score class after the cut. The cases stop
before that, so the box holds places. The script stops if a case no longer shows what it is
there for.

## The group `tidy`

What SynthSeg makes of its networks' answer: the label of each voxel, on the grid as the
volume stores it. Each case is the chance of each of SynthSeg's 33 structures at each voxel
of a small padded grid, made up, and given to SynthSeg's own `postprocess` (in
`SynthSeg/predict_synthseg.py`), called as its `predict` calls it, with SynthSeg's own
lists. SynthSeg pads an image to 128 voxels a side at least; these grids are 8 to 16 a side.
The rule is the same and the files are small.

`postprocess` goes one of two ways. SynthSeg's own program (`SynthSeg_predict.py`) tells it
`fast` whenever the robust networks run, and not otherwise. The first four cases are made
with `fast` off, as for the standard network. The three whose names begin `robust_` are made
with it on: the padding is taken off before the largest piece of everything is looked for,
no piece of a class is looked for, and every chance but the first structure's that is not
above 0.2 is put at nought. `case.json` says which way a case was made (`fast`).

| Case | What it is |
|---|---|
| `pieces` | A block of white matter with other structures in it, each of a class of its own, so each shows one rule. Of two pieces of a class the smaller goes. Pieces that touch at a corner or along an edge are two. Of two as large, the first met is kept, going x by x, then y, then z. A voxel joins two pieces of a class when the class's chance there is over 0.25, whatever is likelier there; a chance of exactly 0.25 joins nothing. Two structures of one class are one piece where they touch. The lowest class is left alone. Of two chances that are equal the first wins |
| `speck` | A blob with specks and blobs round it, in faint noise. What is not joined to the largest piece of everything goes. A voxel joins when its chances add to more than 0.25, though none of them is over it; one whose chances add to exactly 0.25 does not. A piece joined to the blob only through the padding stays: the pieces are found before the padding is taken off. A blob that goes does not count when the pieces of its class are found afterwards. Stored as sagittal slices |
| `cortex` | White matter with cortex on both sides, and an area for every voxel. The cortex takes the labels of the areas, and nothing else does, nor a speck of cortex that went. The image is a box cut from a larger grid, stored as coronal slices: outside the box every label is 0 |
| `close` | Chances chosen to fall on the edge of SynthSeg's arithmetic. One voxel's chances add to more than 0.25 only in the order NumPy adds them, and another's only when added some other way: what lies beyond the first stays, and beyond the second goes. Two chances one step apart become equal when divided by their voxel's sum, so the first wins; two others stay apart. A voxel of zeros is nothing. And one voxel holds a chance below nought, which no network gives, so that its chances add to nought: divided, those that were nought are no numbers, and the first of them wins |
| `robust_pieces` | Made with `fast`. A blob with a structure in two pieces: both stay, where the standard way would lose the smaller. More of the blob's own structure joined to it only through the padding goes, where the standard way would keep it. A speck goes. A voxel whose chances add to 0.375 with none over 0.125 joins a piece to the blob and is itself nothing, having no chance over 0.2; one whose chances add to exactly 0.25 joins nothing. Stored as sagittal slices |
| `robust_marks` | Made with `fast`. Voxels that turn on the mark of 0.2: a chance of 0.22, the only one over the mark, wins over nothing at 0.21; a chance of exactly 0.2, as single precision has it, goes, and the next number above it stays; where no chance is over 0.2 the voxel is nothing. And, as in `close`, sums that are over 0.25 only in NumPy's order or only otherwise, and two chances one step apart that become equal when divided by their sum, and two that do not |
| `robust_cortex` | Made with `fast`. White matter with cortex on both sides and an area for every voxel, a speck of cortex that goes, and two voxels of left cortex apart from the rest of their class, which stay and take their areas where the standard way would lose them. A box cut from a larger grid, stored as coronal slices |

In each case folder:

| File | What it holds |
|---|---|
| `chances.f32` | The made-up chances on the padded grid: `[x, y, z, structure]`, the structure fastest. With `padded` as `[padded_x, padded_y, padded_z]`, the chance of a structure at x, y, z is `values[((x * padded_y + y) * padded_z + z) * 33 + structure]` |
| `areas.u8` | Only in `cortex` and `robust_cortex`: the area at each voxel of the padded grid as Voxa's cortex network gives it, z fastest. It is 1 plus the place of the largest of the chances of areas 1 to 68. SynthSeg was given the chances themselves, made up for every voxel, and the script stops if the two do not come to the same labels |
| `labels.i32` | SynthSeg's label at each voxel of the grid as stored, in the order of a volume: columns fastest |
| `case.json` | `fast`: whether SynthSeg's tidying was told so. `chances`: the `padded` size and how many `structures`. `back`: how to come back from the image to the grid, as Voxa remembers it: `order` and `turned_round` as in `prepare`, `padding_before` (the voxels of padding before the image along x, y and z), `size` (the image's size without its padding), `cut_from` (where the image began in the turned grid; all zero when nothing was cut) and `grid_dims` (the grid's columns, rows and slices). `labels`: the same `dims` |

Beside the cases:

- `lists.json` holds SynthSeg's own lists, as its `predict` prepares them: the `labels` of
  the 33 structures in the networks' order, the `classes` its tidying gives them, the
  `area_labels` of the cortex network's 69 answers, and the two `cortex` labels.
- `sums.json` holds six lists of made-up 32-bit floats (7, 9, 15, 33, 130 and 255 of them),
  each with its `sum` as NumPy adds along the last axis of an array, which is how SynthSeg
  adds a voxel's chances. Under eight numbers are added in turn. Up to 128 are added in eight
  lanes, every eighth number to a lane; the lanes are added in pairs, and what is left over
  is added in turn. A longer list is halved at a multiple of eight, and its halves are added.
  The order matters: a sum can fall on either side of 0.25 by it. Each list is one whose sum
  comes out differently when a part of that rule is broken.

Every block of a scene says what SynthSeg must make of it, and the script stops if SynthSeg
makes anything else, or if NumPy no longer adds by the rule above. So a case cannot quietly
stop showing what it is there for.

## Adding a group

A group is a function like `prepare_group(checkout, out)` and a line in `GROUPS` at the foot
of the script. Take random numbers from `seeded(group, case)`, write arrays with
`write_floats` or `write_bytes` and `case.json` with `write_json`, and say here what the new
files hold.
