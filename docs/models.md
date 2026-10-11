# The files labelling needs

This page is written ahead of the feature. Voxa 0.4.0 shows labels made elsewhere and does
not make any. From 0.5.0 it will label a brain scan itself with SynthSeg 2.0, and for that it
needs two things it does not come with: a runtime, and a model folder. Voxa without them is
Voxa as it is now.

**Voxa is not a medical device and is not for diagnostic use.** A label made by a model is a
guess that looks certain.

## Why Voxa comes with neither

- **The model.** SynthSeg's code is Apache 2.0, but its trained weights carry no licence
  statement of their own. They are distributed with FreeSurfer, under FreeSurfer's terms.
  Until that is settled Voxa ships no weights and fetches none: you get the files yourself,
  under the terms they come with, and turn them into a model folder on your own computer.
- **The runtime.** ONNX Runtime is MIT, and DirectML comes under Microsoft's own licence,
  which lets it be passed on inside an application for use on Windows but is not open
  source. Keeping both beside Voxa, not inside it, keeps Voxa one executable under its own
  two licences that needs nothing else for everything it does today.

Voxa uses no network. Nothing here is downloaded by Voxa.

## The runtime

Three files from Microsoft's ONNX Runtime with DirectML, version 1.24.4:

- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `DirectML.dll`

They are in Microsoft's `Microsoft.ML.OnnxRuntime.DirectML` package, and in the Python
package `onnxruntime-directml` under `onnxruntime/capi`. Put them in a folder called
`runtime` beside Voxa's executable, or anywhere and tell Voxa where.

On the Windows 11 this was tried on, Windows' own `DirectML.dll` (1.15.5) was found and gave
the same answer when the file was left out. An older Windows was not tried.

Microsoft's builds of ONNX Runtime report usage (not images) unless told not to. Voxa tells
the runtime not to, before it asks anything else of it.

DirectML is frozen: Microsoft supports it and ships it with Windows, and adds nothing new to
it. 1.24.4 was the newest ONNX Runtime found with DirectML when this was written.

## The model folder

1. Get a copy of [SynthSeg](https://github.com/BBillot/SynthSeg). Its `models` folder comes
   with the 1.0 model only.
2. Get the four 2.0 files and put them in that `models` folder: `synthseg_2.0.h5`,
   `synthseg_robust_2.0.h5`, `synthseg_parc_2.0.h5`, `synthseg_qc_2.0.h5`. They come with
   FreeSurfer, in the folder of its `mri_synthseg` command, under FreeSurfer's terms.
3. Run the converter in `tools/synthseg-to-onnx`; its README says how. It needs Python and
   TensorFlow, once. Voxa never does.
4. Tell Voxa where the folder is, or call it `models` and put it beside Voxa's executable.

### What the folder's list shows, and what it does not

`model.json` lists each network with its size and SHA-256, the SHA-256 of each `.h5` it was
made from, and what the converter's own check measured. Voxa reads a folder only if every
file is there with the size and SHA-256 listed.

That shows a file is the one the list describes: not damaged, not swapped for another. It
does not show who made the folder. A model folder from someone else is as trustworthy as
they are.

## What Voxa does differently from SynthSeg's own program

Three things, and nothing else: the preparing and tidying steps are SynthSeg's, step for step.

- **Volumes are counted voxels.** A part's volume in Voxa is its voxels times the size of
  one, as for any labels Voxa shows. SynthSeg's own table adds up probabilities, so the two
  differ a little.
- **The scan may be cut to the head when memory is short**, and never otherwise. The result
  says so when it was.
- **The cortex network's answer is reduced to areas inside the file.** The labels are the
  same; there is no table of soft volumes for the areas.
