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
  made here use none.
- The Python environment of `tools/synthseg-to-onnx` (its `requirements.txt`). On NumPy 2,
  SynthSeg's own code needs the few one-line fixes that README speaks of.

## Running it

```
python make_golden.py PATH\TO\SynthSeg
python make_golden.py PATH\TO\SynthSeg prepare
```

The first makes every group of cases, the second only the group named. Each group is written
under `crates/voxa-segment/tests/golden/<group>/` of the repository this script is in. It
takes under a minute, most of it Python starting.

A second run writes the same bytes, as long as NumPy, SciPy and nibabel are the versions that
made the files before; every `case.json` says which those were. Each case has a seed of its
own, made from its group's name and its own, so adding a case changes no other. Commit what
the script writes together with the change to the script.

## How to read what it writes

Each case is a folder holding `case.json` and arrays. An array is raw little-endian 32-bit
floats with no header; `case.json` gives its size and repeats its order.

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

## Adding a group

A group is a function like `prepare_group(checkout, out)` and a line in `GROUPS` at the foot
of the script. Take random numbers from `seeded(group, case)`, write arrays with
`write_floats` and `case.json` with `write_json`, and say here what the new files hold.
