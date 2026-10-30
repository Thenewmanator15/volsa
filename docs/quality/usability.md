# Usability file

How Voxa is made easy and safe to use, and the evidence. Kept in the spirit of IEC 62366-1.
Started with the redesign of the window (design:
`docs/superpowers/specs/2026-10-30-voxa-ui-redesign-design.md`).

Never record anything from a participant's scan here: no names, dates, descriptions,
pictures or counts. Write what the person did and said about Voxa, not about their body.

## 1. Use specification

- **Intended use**: see `docs/intended-use.md`. Looking at scans on your own computer: your
  own, and scans for learning, teaching and research. Never diagnosis.
- **Users**: (1) people viewing their own scans, most with no medical or technical
  training, often anxious about what a scan shows; (2) students; (3) researchers.
- **Where**: a Windows computer at home, in a library or an office; a laptop screen as small
  as 1366×768, often with the interface scaled; a mouse, a touchpad, or the keyboard alone;
  sometimes a screen reader.
- **What they bring**: an export from a hospital (a CD's folder, a zip, a DICOMDIR), which
  may hold several studies, scanned paperwork and reports beside the scans.

## 2. Parts of the window that bear on safety

- Orientation: the letters at each edge, and the radiological or neurological convention.
- Scale: the scale bar, and where measurements get their millimetres from.
- Values: the window, the value under the pointer and its unit.
- Labels: who made them and the model's own check.
- Which scan and whose: the scan's name and description; patient details when shown.
- The disclaimer.

The risk register (`risk-register.md`) holds what could go wrong with each.

## 3. Problems seen in 0.6.0

From captures of 0.6.0 with made-up studies (2026-10-27, 2026-10-28 and 2026-10-30):

- U-01. About forty controls in three rows across the top, not grouped by task.
- U-02. Scanned paperwork listed, and opened, before the scans.
- U-03. The measurements window opens over the list of series.
- U-04. Warnings such as "! several planes" sit in the list of series.
- U-05. The list of labelled parts is long, with no overview.
- U-06. Opens on the 3D view; the slices are in a narrow column.
- U-07. Measurement labels in 3D drawn over one another.
- U-08. In an 800×600 window the bar wraps onto four lines.
- U-09. Measuring only on the planes and in 3D, not on the slices a newcomer sees first.

## 4. Use scenarios to test

- S-01. Open an export and find the brain scan. *(success test, part 1)*
- S-02. Measure a length on it. *(success test, part 2)*
- S-03. Make the picture brighter or darker.
- S-04. Find the report among the other files and read it.
- S-05. Tell the patient's left from their right.
- S-06. Label a brain scan with Voxa, and say who made the labels.
- S-07. Do S-01 and S-02 with the keyboard alone.

## 5. Test script

For each participant, on their own computer, with Voxa's zip and either their own export or
a made-up study handed to them as a folder (written beforehand with
`cargo run -p voxa-app --example write_phantom`):

1. Say: "This is a viewer for scans. Please think aloud. I can't help, but there are no
   wrong answers: if something is hard, that is Voxa's fault."
2. Ask them to open the export (S-01). Start a timer when Voxa's window first shows.
3. "Find the scan of the brain." Note the time, and every place they looked first.
4. "Measure something on it: anything you like." Note the time, and how they found the tool.
5. Then S-03 to S-06 as time allows; S-07 for a keyboard user.
6. Ask: what was hardest; what they expected to find and did not; what they thought any
   number or label meant.
7. Record below: the date, the participant as "P1", "P2" (kind only: own scan, student,
   researcher), the version, each task's time and outcome (done unaided, done with a hint,
   not done), and what they did and said. Nothing from their scan.

**Pass**: the success test (S-01 and S-02) done unaided within a minute.

## 6. Results

### Formative: 0.6.0 preview (*owner*, 2026-10-30)

Three to five people, during the redesign. None recorded yet.

### Summative: the redesign

None recorded yet.
