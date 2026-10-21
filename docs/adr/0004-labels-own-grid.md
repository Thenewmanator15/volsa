# 4. Labels keep their own grid, and Voxa does not make them

Date: 2026-10-10. Status: accepted. Its fifth point, that Voxa does not run a labelling model,
was replaced in 0.5.0 by [record 0006](0006-voxa-runs-a-labelling-model.md); the rest stands.

## Context

A label volume says, for each of its voxels, which part of the head that voxel belongs to. It
is made by another program, often on a grid of its own: coarser or finer than the scan it was
made from, in another voxel order, sometimes covering more or less of the head. The usual way
to show labels over a scan is to resample them onto the scan's grid once and then treat the
two as one.

Resampling labels changes them. A part's edge moves by up to half a voxel, small parts lose or
gain a noticeable share of their volume, and the volume Voxa would report is no longer the one
the labelling program wrote. Resampling onto each series of a study in turn would give each
series a different copy.

Labels also carry a risk that pictures of a scan do not: a coloured outline looks as certain
on a poor scan as on a good one, and a viewer that draws it can be taken to vouch for it.

## Decision

- A label volume is read as it is and kept on its own grid, with its own transform from voxel
  to patient. It is never resampled. `voxa-labels` reads it; nothing in `voxa-volume` or
  `voxa-dicom` knows about labels.
- Labels and scan meet only in patient space. A plane view and the 3D scene each ask, for a
  point of the patient, which part lies there: the nearest label voxel, never a blend of
  neighbours, since a number that stands for a part cannot be averaged.
- One copy goes on the graphics device: the part numbers as a 3D texture, and a small table
  of how each part looks. The plane views and the Parts look both read that copy.
- A part's volume is its count of voxels times the size of one of the label volume's own
  voxels. Voxa works out nothing else about a part but the difference between a left and a
  right, and says of that difference that it cannot tell how much is noise.
- Voxa does not run a labelling model, and ships none. It shows labels made elsewhere, says
  so wherever they are drawn, and passes on what the labels' own table says about how each
  part was made and whether the labelling program's own check passed. *(Replaced by record
  0006: from 0.5.0 Voxa does run one, and still ships none. Labels from a file are still
  shown as made elsewhere.)*
- Labels belong to the study they were opened on and are shown on its series only. A label
  file does not say which scan it was made from, so labels whose box lies nowhere near the
  series are refused, and that is the only check there is.

## Consequences

- What is drawn and what is counted are the labelling program's voxels, exactly. On a scan
  finer than the labels the edges of parts are visibly stepped; that is how coarse the
  labels are, and it is left to be seen.
- A label set costs its own memory on the device (two bytes a voxel) whatever the size of
  the scan, and nothing is recomputed when another series of the study is shown.
- Labels can cover more than a series does: over a thin slab, whole-head labels are drawn
  whole in 3D and only where the slab is in the planes.
- Labels made from another scan of the same head, taken in the same place in the scanner,
  are not told apart from the right ones. Nothing in the files could tell them apart.
- Running a model inside Voxa was a separate decision with its own questions of licence and
  trust; nothing here depends on it. It is record 0006.
