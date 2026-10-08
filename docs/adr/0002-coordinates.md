# 2. Typed coordinate spaces, DICOM LPS

Date: 2026-10-05. Status: accepted.

## Context

A viewer handles the same position in several coordinate systems: the patient (millimetres), the
image grid (column, row, slice) and the screen (pixels). Mixing them up produces images that look
right and are wrong, the most dangerous being a left-right flip.

## Decision

- Patient coordinates follow DICOM: left-posterior-superior, millimetres. +x is the patient's
  left, +y the back, +z the head. Files in other conventions are converted on reading.
- Voxel coordinates are (column, row, slice), in that order, counted from zero at voxel centres.
- `Point<S>` and `Affine<From, To>` carry their space as a type parameter (`Patient`, `Voxel`,
  `Screen`). Applying a transform to a point in the wrong space does not compile.
- Display convention (radiological or neurological) is a property of the view, never of the data.

## Consequences

- Every conversion is an explicit, named transform that can be tested alone.
- A little more typing at call sites; raw arrays appear only at the edges (file reading, GPU
  upload).
- Orientation letters are derived from the transform, so a tilted or flipped acquisition is
  labelled from its own geometry and not from an assumption about the scanner.
