# 5. The volumes of a series are listed as series of their own

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 2026-10-14. Status: accepted.

## Context

Many MRI series hold more than one volume under one series identifier: the echoes of a
multi-echo scan, the diffusion weightings of a diffusion scan, the time points of a dynamic
or functional scan, the phases of a heart scan, a scan repeated. Every slice position is then
filled several times. Voxa used to see such a series as a stack with more than one image at a
position, which is not a volume, and showed it as a plain stack with no 3D view.

Everything Voxa does with a series (the slice stack, the planes, the 3D scene, measuring,
saved views, labels) assumes one image at each position. Teaching each of them about a fourth
dimension would touch all of them.

## Decision

- The index splits a series into volumes when every position is filled the same number of
  times, at least twice, and each volume is then a regular stack. The images at one position
  are put in order by what tells volumes apart: the temporal position, then the diffusion
  weighting, the echo time, the trigger time and the acquisition number, and last by the order
  the scanner numbered and stored them in.
- Each volume is listed as a series of its own, with the identifier of the series and its
  place after an `@`: `1.2.3@1`, `1.2.3@2`. A DICOM identifier holds only digits and dots, so
  these cannot clash with one. Its `VolumeSet` says which series it is of, its place, how many
  there are, and what sets it apart in words ("echo 80 ms", "b 1000"), from the first of those
  values that every image of a volume agrees on and that differs between volumes; it is empty
  when nothing does.
- The list shows one card for the set, with the number of volumes. A viewport shows one volume
  and says which; stepping to another keeps the slice, the view and the camera, since the
  volumes share their geometry.
- When Voxa chooses what to open first, a tie between volumes goes to the earlier one.
- Nothing is guessed when the counts are uneven or a volume would not be regular: the series
  stays one stack, with its reason, as before.

## Consequences

- Everything made for a series works on each volume unchanged, and is tested once.
- Saved views and annotations belong to a volume, not to the whole set: a ruler drawn on the
  first echo is not shown on the second.
- There is no view of all the volumes at once, no curve of a voxel's value across them and no
  difference between two of them. Each would be a feature of its own, built on the volumes as
  listed here.
- A series whose volumes differ in a value not on the list above is still split, with empty
  names: the volumes are numbered, not described.
