# synthseg-to-onnx

Turns SynthSeg 2.0's files into a model folder that Volsa reads. It is a tool for a person to
run once; it is no part of Volsa, and cargo neither builds, runs nor tests it.

**It ships nothing.** No weights are in this repository. You bring SynthSeg's files, and what
this writes stays on your computer.

Volsa is not a medical device and is not for diagnostic use, and neither is anything made here.

## What it needs

- A copy of [SynthSeg](https://github.com/BBillot/SynthSeg) with the four 2.0 files in its
  `models` folder: `synthseg_2.0.h5`, `synthseg_robust_2.0.h5`, `synthseg_parc_2.0.h5` and
  `synthseg_qc_2.0.h5`. `docs/models.md` says where they come from.
- Python 3.13 with the packages in `requirements.txt`, in an environment of its own.
- On NumPy 2, SynthSeg's own code needs four one-line fixes, made by hand in your copy of
  SynthSeg; without them the converter stops inside SynthSeg with a NumPy error. In two
  functions SynthSeg uses what `np.where` gives, an array, where a single number belongs, and
  each fix takes the first index of it, `np.where(...)[0][0]`:
  - `get_flip_indices` in `SynthSeg/predict.py`, three lines: where it fills `lr_indices`,
    and the two lines that fill `flip_indices` for a left and for a right label;
  - `align_volume_to_ref` in `ext/lab2im/edit_volumes.py`, one line: `swapped_axis_idx`.

  They are proposed to SynthSeg as
  [pull request 127](https://github.com/BBillot/SynthSeg/pull/127), which shows each line.
  Nothing else of SynthSeg's needs changing: as it imports SynthSeg, the converter itself
  serves Keras 2 from `tf_keras` and puts back, where NumPy no longer has them, the old names
  `np.int`, `np.float`, `np.bool` and `np.object` that SynthSeg still uses. It changes none of
  SynthSeg's files.
- Windows, to check the networks on graphics cards. Elsewhere the check runs on the processor
  only.

## Running it

```
python convert.py PATH\TO\SynthSeg PATH\TO\model-folder
```

It takes a few minutes. It writes, into the model folder:

- `standard.onnx`, `robust_1.onnx`, `robust_2.onnx`, `robust_3.onnx`, `cortex.onnx`,
  `scores.onnx`: the six networks;
- `check/`: a made-up head and what SynthSeg's own program made of it, for Volsa's own tests;
- `model.json`, last: the list Volsa checks the folder against.

## What it does

1. **Converts** each network on its own with tf2onnx (opset 17), built as SynthSeg's
   `build_model` builds it and loaded by layer name. A layer the `.h5` does not name is an
   error, not a silent gap.
2. **Tidies** each graph (`graphs.py`): every upsampling step, which tf2onnx writes as three
   rounds of work on six-dimensional tensors, becomes one `Resize`. The standard network's
   blur of its answer (SynthSeg's Gaussian blur with a sigma of 0.5, done inside its
   `build_model`) is written into the file as three one-axis convolutions, one along each
   axis, with SynthSeg's own one-axis kernels (`graphs.add_blur`; converter version 2). As one
   convolution with the 3 x 3 x 3 kernel the runtime asked the processor for about 3 000
   bytes a voxel more; this way it asks for nothing more, and the labels are the same.
3. **Makes each network take and give only what Volsa needs.** A network whose answer feeds
   another gives a byte for each voxel, not thirty-three probabilities; a map of classes goes
   in as bytes and is spread inside the file; the last network of each model gives its
   probabilities in two halves, because DirectML hands back no more than about 2 GiB at once.
4. **Checks** (`check.py`) the converted networks against SynthSeg's own program on a made-up
   head, on the processor and on every graphics card DirectML offers. `model.json` is written
   only if:
   - the standard model on the processor gives SynthSeg's label at every voxel;
   - every other model and device agrees at 99.99% of voxels or more, with Dice of at least
     0.999 for every structure, and scores within 0.001;
   - the cortex network's byte answer gives the same labels as SynthSeg's own use of its
     probabilities.

The made-up head is painted from the sample label map that comes with SynthSeg: one grey
level for each kind of tissue, blurred a little, shaded and made noisy. It is nobody's scan.

## What it does not show

That the labels are right. The check shows that the converted networks say what SynthSeg
says. Whether SynthSeg is right about a given scan is another question, and the model's own
scores, which Volsa shows, are only the model's opinion of its own work.

## The files

| File | What it holds |
|---|---|
| `convert.py` | The command: SynthSeg's lists, the six networks, `model.json` |
| `graphs.py` | The changes made to what tf2onnx writes |
| `check.py` | The made-up head, SynthSeg's own answer, the converted networks' answer, the marks |
