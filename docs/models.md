# The files labelling needs

Since 0.5.0, Voxa labels a brain scan itself, with SynthSeg 2.0's networks run through ONNX
Runtime on this computer. No Python runs. For that it needs two things: a runtime, and a model
folder.
A download of Voxa from the Releases page comes with the runtime; nothing comes with the model
folder, which you make once. Voxa without them is Voxa as it was at 0.4.0: it starts and does
everything else, and the **Label this scan** button in the Parts tab is greyed and says which
of the two is missing.

**Voxa is not a medical device and is not for diagnostic use.** A label made by a model is a
guess that looks certain. What Voxa is for, and what it is not for, is in
[intended-use.md](intended-use.md) (`INTENDED-USE.txt` in a download of Voxa).

## What comes with Voxa, and what does not

- **The runtime comes in the download.** The zip on the Releases page holds a folder called
  `runtime` beside `voxa.exe`, with ONNX Runtime's `onnxruntime.dll` and
  `onnxruntime_providers_shared.dll` and ONNX Runtime's own `LICENSE` and
  `ThirdPartyNotices.txt`. ONNX Runtime is Microsoft's, under the MIT licence. Voxa looks in
  that folder first. `DirectML.dll` is not in it: Voxa uses the copy that is part of Windows
  ([below](#directml-from-windows)). A build of Voxa from source has no `runtime` folder, and
  the files are fetched by hand, as [the runtime](#the-runtime) says.
- **The model never comes with Voxa.** SynthSeg's code is Apache 2.0, but its trained weights
  come with FreeSurfer, under FreeSurfer's licence, which says the software is designed for
  research only and that clinical use is neither recommended nor advised
  ([below](#the-terms-of-the-models-weights)). Voxa ships none and
  fetches none: you get the files yourself, under the terms they come with, and turn them
  into a model folder on your own computer.

Voxa uses no network. Nothing here is downloaded by Voxa.

## What a person does

1. With a download of Voxa, the runtime is in place already. With a build of your own, put
   the runtime's files in a folder called `runtime` beside Voxa's executable (below).
2. Make the model folder once, with the converter, and call it `models` beside Voxa's
   executable (below).
3. Open a scan that has a 3D view, choose the Parts tab and press **Label this scan**.

If the folders are somewhere else, **Options**, beside the button, has **Folder of the
models…** and **Folder of the runtime…**. Voxa remembers what was chosen.

## The runtime

A download of Voxa has it. A build from source needs these files from Microsoft's ONNX
Runtime with DirectML, version 1.24.4, fetched by hand:

- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `DirectML.dll`, which may be left out: then Windows' own copy is used, as in a download

They are in Microsoft's `Microsoft.ML.OnnxRuntime.DirectML` package, under
`runtimes/win-x64/native`, and in the Python package `onnxruntime-directml` under
`onnxruntime/capi`. The zip a release offers carries both, taken from that package on nuget.org,
with the package's `LICENSE` and `ThirdPartyNotices.txt`. With a build of your own, put them in
a folder called `runtime` beside Voxa's executable, or anywhere and tell Voxa where. Voxa looks
beside its executable first and then in the folder named under **Options**.

The third file of that package, `DirectML.dll`, is not carried: without it in the folder, ONNX
Runtime uses the copy Windows has.

ONNX Runtime needs Microsoft's Visual C++ 2015-2022 x64 redistributable, which many computers
already have and Microsoft gives free ([below](#what-windows-needs-for-it)). The `voxa.exe` in a
download does not need it for anything else. If ONNX Runtime will not load because a library
it needs is missing or old, Voxa says that it needs the redistributable.

Voxa loads the runtime as it starts, when it checks whether this computer can label a scan,
once it has found a model folder; with no model folder it does not load it. A program can
hold only one runtime for as long as it lives. If you change the runtime folder
after Voxa has loaded the runtime, the change takes effect when Voxa is next started.

Any build of ONNX Runtime from 1.17 on has the interface Voxa asks for and loads; an older
one is refused with a sentence that says so. 1.24.4 is the one the networks were measured
with.

Microsoft's builds of ONNX Runtime report usage (not images) unless told not to. Voxa tells
the runtime not to, before it asks anything else of it.

1.24.4 was the newest ONNX Runtime found with DirectML. On Linux and macOS there is no
DirectML and the processor is used; this release has not been built or run there.

### What Windows needs for it

**The Microsoft Visual C++ Redistributable, x64.** ONNX Runtime is built with Microsoft's C++
compiler and needs its runtime libraries, which Windows does not always have. ONNX Runtime's
[installation page](https://onnxruntime.ai/docs/install/) says its Windows builds require the
Visual C++ 2019 runtime and recommends the latest. That is the free Visual C++ 2015-2022
Redistributable for x64. Microsoft's page
[Latest supported Visual C++ Redistributable downloads](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
now lists it as the Visual C++ v14 Redistributable, for Visual Studio 2017 to 2026, says that
programs built with Visual Studio 2015 can use it too, and offers it for x64 at
<https://aka.ms/vc14/vc_redist.x64.exe>. A computer may have it already, put there by another
program.

### DirectML from Windows

DirectML, which lets ONNX Runtime use a graphics card, is part of Windows. Microsoft's
[DirectML version history](https://learn.microsoft.com/en-us/windows/ai/directml/dml-version-history)
says it is distributed as a system component of Windows, in Windows 10 version 1903 (build
18362) and newer. Which version a Windows has depends on the Windows: that page lists
DirectML 1.0.0 as first in Windows 10 version 1903, 1.1.0 in Windows 10 version 2004, 1.6.0
in Windows 11 (21H2) and 1.8.0 in Windows 11 version 22H2. For the later versions it names
no Windows, only Microsoft's own package of DirectML (`Microsoft.AI.DirectML`). ONNX
Runtime's
[page on DirectML](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)
says it uses DirectML 1.15.2.

On the Windows 11 this was tried on, Windows' own `DirectML.dll` (1.15.5) was found and gave
the same answer as the copy in Microsoft's package. Windows 10 and other versions of Windows 11
were not tried, nor a Windows whose own copy is older than the one ONNX Runtime uses. If
labelling on a graphics card fails on such a Windows, Voxa moves on to the next device it can
use, in the end the processor, which is slower; putting `DirectML.dll` 1.15 or later, from
Microsoft's `Microsoft.AI.DirectML` package, in the `runtime` folder may let the card be used.

Microsoft says DirectML is in sustained engineering: it is still supported, and new work has
moved to Windows ML. DirectML comes under Microsoft's own licence, not an open-source one;
Voxa carries no copy of it.

## The model folder

1. Get a copy of [SynthSeg](https://github.com/BBillot/SynthSeg). Its `models` folder comes
   with the 1.0 model only.
2. Get the four 2.0 files and put them in that `models` folder: `synthseg_2.0.h5`,
   `synthseg_robust_2.0.h5`, `synthseg_parc_2.0.h5`, `synthseg_qc_2.0.h5`. They come with
   FreeSurfer, in the folder of its `mri_synthseg` command, under FreeSurfer's terms
   ([below](#the-terms-of-the-models-weights)).
3. Run the converter in `tools/synthseg-to-onnx`; its README says how. It needs Python and
   TensorFlow, once. Voxa never does. On NumPy 2 SynthSeg's own code needs a few one-line
   fixes, which the README names.
4. Tell Voxa where the folder is, or call it `models` and put it beside Voxa's executable.
   Voxa looks in the folder named under **Options** first and then beside its executable.

The folder holds six networks (`standard`, `robust_1`, `robust_2`, `robust_3`, `cortex` and
`scores`, about 370 MB together), `model.json`, and a `check` folder with a made-up head and
what SynthSeg's own program made of it, which Voxa's tests use and Voxa itself does not read.

### What the folder's list shows, and what it does not

`model.json` lists each network with its size and SHA-256, the SHA-256 of each `.h5` it was
made from, and what the converter's own check measured. Voxa reads a folder only if every
file is there with the size and SHA-256 listed, and says which check failed if one does not.

That shows a file is the one the list describes: not damaged, not swapped for another. It
does not show who made the folder. A model folder from someone else is as trustworthy as
they are.

## The terms of the model's weights

SynthSeg's code is under the Apache License 2.0. Its trained 2.0 weights, the four `.h5` files
above, carry no licence statement of their own. They are distributed with FreeSurfer: they
are in the `mri_synthseg` folder of [FreeSurfer's repository](https://github.com/freesurfer/freesurfer)
and come with its releases. FreeSurfer is under the
[FreeSurfer Software License Agreement](https://surfer.nmr.mgh.harvard.edu/fswiki/FreeSurferSoftwareLicense),
version 1.0 of February 2011, whose text is also
[`LICENSE.txt`](https://github.com/freesurfer/freesurfer/blob/dev/LICENSE.txt) in that
repository. It is given by The General Hospital Corporation of Boston, which it calls MGH, and
covers software and data downloaded from FreeSurfer. Downloading or using them is accepting
it.

What its Part B, the licence for what is downloaded, says, in short. This is a summary, not
legal advice; the agreement itself is what holds.

- **What it allows.** A royalty-free, non-exclusive licence to use, reproduce, make derivative
  works of, display and distribute the software and data, with the right to sublicense,
  including putting them into proprietary programs.
- **On what conditions.** Whoever passes on a copy of any of it passes on all of Part B's
  terms with it, prefaced by a sentence the agreement gives; keeps the attributions,
  copyright notices and licences that apply; and marks a changed version as changed, not as
  the original. They are asked to consider, but need not, sharing the source of their changes.
- **Research, not clinical use.** It says the software is "designed for research purposes
  only", that it has not been reviewed or approved by the US Food and Drug Administration or
  any other agency, and that clinical applications are neither recommended nor advised.
- **Commercial use.** It does not forbid it. It says that any commercialisation is at the sole
  risk of whoever undertakes it.
- **No warranty.** The software comes as it is, with no promise of support. MGH and the
  contributors disclaim every warranty and all liability, and the user takes on the liability
  for their own use and agrees to hold them harmless.
- **Names.** The names, logos and trademarks of MGH, of the contributors and of funders may not
  be used to endorse or promote a product without written permission.
- **Ending.** Use, copying or passing on that does not keep to it ends the rights it grants.

Part C says the agreement is governed by the laws of Massachusetts.

For Voxa this means: Voxa ships none of the weights and fetches none, so it passes on nothing
of FreeSurfer's; you get the files under the agreement yourself, and make the model folder on
your own computer. Whether, and on what terms, a model folder may be passed on
is the agreement's to say: Part B covers derivative works and passing them on. Labels Voxa makes
with the weights are under the agreement's caution as well as Voxa's own: designed for
research, with clinical use neither recommended nor advised.

## What a run needs of this computer

Memory is what limits a run, not time. Before a run Voxa works out what the scan needs and
goes only where the system can still promise it, trying in this order:

1. the whole scan on a graphics card with memory of its own, if it fits in what the system
   lets Voxa use there;
2. the whole scan on the processor, if the computer's ordinary memory can back it;
3. the scan cut to the head, on the card and then on the processor, where cutting makes it
   smaller.

Whether a run fits is worked out when the button is pressed, as the run starts. If none fits,
a notice says so, with what the run needs and what the computer has. Integrated
graphics, which share the computer's memory and were found slower than the processor, are
used only if you choose them under **Options**. A card is never allowed to spill into
ordinary memory by plan, since that is ten or more times slower.

The sums are in bytes for each voxel of the grid the networks are given (the scan at 1 mm,
each side padded up to a multiple of 32 and at least 128), with a quarter added for safety.
A GB below is a GiB, as the system shows memory.

| | On a card | On the processor |
|---|---|---|
| The networks that label | 1 400 bytes of the card | 700 bytes of ordinary memory (900 with the cortex areas) |
| The model's check of its own work | 8.2 GB of the card, whatever the scan | 3.1 GB of ordinary memory, whatever the scan |
| Voxa's own | 200 bytes of ordinary memory | 200 bytes of ordinary memory |

A run on a card also asks the system for ordinary memory, since Windows sets some aside behind
what a program puts on a card: 1 900 bytes a voxel, and 8.4 GB for the check. A run holds no
more than the largest of its networks at a time, with the chances it has found and, for a
moment, the answer that has just arrived: two sets of chances at most. If the check does not
fit on the card, or ordinary memory cannot back it there, that one network runs on the
processor and the result says so. The figures were measured on made-up volumes and rounded up;
the decision record, [adr/0006](adr/0006-voxa-runs-a-labelling-model.md), gives them.

For a made-up cube of 160 voxels a side, which is 4 096 000 voxels, the networks need 6.7 GB
of a card and the check 10.3 GB, and a run on a card asks the system for up to 11.5 GB of
ordinary memory; on the processor the whole run asks for up to 5.3 GB. A whole head scanned
at 1 mm, 176 x 256 x 256 voxels, is padded to 192 x 256 x 256 and its networks need 20.5 GB
of a card, so on a 16 GB card it goes to the processor or is cut to the head, and the result
says which. These are limits a run is held to, not what it was seen to use.

## What the result says

Every result carries its check and its history.

- **The model's check.** SynthSeg scores its own work in groups of structures. Voxa says the
  check failed when any group scores below 0.65, names the groups that did, and says that
  first, in the warning colour.
- **Who made the labels.** The caption says Voxa made them, with which model, on which
  device, on what day, and "Not for diagnostic use." The day is the date at Greenwich (UTC),
  which late in the evening west of Greenwich, or early in the morning east of it, is not the
  local one. Over the 3D view a failed check is said before who made the labels.
- **Notes.** A line for anything a reader should know: the slices are more than 3 mm apart,
  so it is a rough map; the run was on the processor, or on another card, because the card
  had no room or stopped answering; the check of the work ran on the processor; the scan was
  cut to the head.
- **The model.** Automatic picks the robust model when the slices are more than 3 mm apart
  and the standard model otherwise, and says which it picked and why. The cortex areas
  divide the cortex into 34 areas a side and are on unless you switch them off.

While a run is on, its stage and a **Stop** button are shown in the Parts tab, and only
there; everything else in Voxa keeps working. The 3D view may be slow while the card is busy.
A run belongs to the scan it was started on: if the scene moves to another series, or
another export is opened, the run is stopped and its result dropped, even one that had just
finished. Only one run goes at a time, and labels a run made are drawn only once the Parts
tab lists them.

## What is kept

A result is kept on this computer, in the folder the system gives Voxa that stays on it
(`%LOCALAPPDATA%\Voxa\data` on Windows), so that it can be shown again when the same scan is
opened. It is a label file and a table of names filed under a hash of the series, and holds
nothing that says whose scan it was. The
Parts tab then offers **Show**, **Make again** and **Delete**. **Save as** writes the label
file and its table wherever you choose, for other programs; the file's name always ends in
`.nii.gz`, which is added if you leave it out. **Keep results on this computer**,
under **Options**, switches keeping off.

If the graphics device is lost, which a driver reset can do, the interface cannot be given
another. Voxa says so once, in the window and in a message box, and asks to be started again.
Nothing made is lost: the work in hand goes on, and annotations, saved views and kept results
are written by Voxa's workers and do not depend on the card.

## What Voxa does differently from SynthSeg's own program

Three things, and nothing else: the preparing and tidying steps are SynthSeg's, step for step,
and each is held against arrays that SynthSeg's own Python made.

- **Volumes are counted voxels.** A part's volume in Voxa is its voxels times the size of
  one, as for any labels Voxa shows. SynthSeg's own table adds up probabilities, so the two
  differ a little.
- **The scan may be cut to the head when memory is short**, and never otherwise. The result
  says so when it was. A cut lays the head elsewhere in the box the networks are given, and
  that alone moves their answer: on a made-up head, about one labelled voxel in a hundred came
  out otherwise, along the borders between structures and all through the head, and
  SynthSeg's own program does the same when the head lies elsewhere in its box. Neither
  answer is the truer.
- **The cortex network's answer is reduced to areas inside the file.** The labels are the
  same; there is no table of soft volumes for the areas.

SynthSeg's own program stops on a stack of 10 or fewer slices, and on one that is 10 voxels or
fewer from bottom to top at 1 mm; Voxa refuses the same, with a sentence. The localiser series
of an ordinary export are in this range.

## How it was checked

`cargo xtask check` holds every preparing and tidying step to arrays that SynthSeg's own Python
made from small made-up volumes, with no runtime and no model folder on the machine.
`cargo xtask check-models`, on a computer that has them, runs the real networks on the
converter's made-up head. The standard model on the processor gives SynthSeg's label at every
voxel. Every other pairing of model and device gives at least 99.99% of voxels the same, a
Dice of at least 0.999 for every structure and scores within 0.001. The acceptance record,
[acceptance-0.5.0.md](acceptance-0.5.0.md), says what was run for this release and what was
not.
