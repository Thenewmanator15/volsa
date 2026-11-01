# Security

Volsa opens files that come from elsewhere, and to label a scan it loads a library from a folder.
A fault in either can matter, so please report a vulnerability privately, where it can be fixed
before it is widely known.

## How to report one

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and choose
**Report a vulnerability**, or go straight to [the form](../../security/advisories/new). Only
you and the people who look after Volsa see the report, and the conversation about it stays
there. Do not open a public issue, discussion or pull request about it.

Say which version of Volsa and which Windows, what you did, what happened and what you expected,
and, if you can, include a made-up file that makes it happen (see below).

## What counts

Volsa runs on a person's own computer, makes no network connections, and opens what it is given.
The faults that matter are those where something Volsa is handed makes it do what it should not.

- **The files Volsa reads, which may come from anyone.** DICOM files, folders and zip files of
  them, `DICOMDIR` files, NIfTI label files (`.nii`, `.nii.gz`) and their tables of names
  (`.json`, `.ctbl`), a model folder (`model.json` and its networks), and the files Volsa keeps
  in its own two folders. A file that makes Volsa run code, read or write outside the places
  it should, or take memory or time without end counts. A file that only makes Volsa stop or
  show something wrong is a fault too; if you are sure it does no more than that, an ordinary
  issue with a made-up file is welcome, and if you are not sure, report it here.
- **The runtime library.** To label a scan, Volsa loads ONNX Runtime (`onnxruntime.dll`, which
  in turn loads `onnxruntime_providers_shared.dll` and `DirectML.dll`) from a folder called
  `runtime` beside `volsa.exe`, or from the folder named under **Options** in the Tools tab. A
  way to make Volsa load a library from anywhere else, or one the person did not put there,
  counts. Volsa runs whatever library is in a folder the person names, with the person's own
  rights: that is how it is meant to work, so name only a folder that holds Microsoft's files.
- **Anything that sends data off the computer**, or that writes a patient's details where Volsa
  says it does not: the log, the folders where Volsa keeps things, or a saved picture.
- **The spill file.** Past its memory budget Volsa writes the numbers of a large series' slabs
  to `cache/spill-<random number>.volsa` in its local folder, opened by Volsa alone and deleted
  when the series gives way, when Volsa closes and, if a stop left one, at the next start. It
  holds pixel values and nothing else: no name, no tag. A way to make Volsa leave one behind
  while it runs no more, write anything else into it, put it anywhere else, or read another
  program's file in its place counts.

These are not for this form:

- How well a label matches the anatomy. Volsa is not a medical device and is not for diagnostic
  use. A label made by a model is a guess, and a label that is wrong is an ordinary issue.
- A fault in ONNX Runtime or DirectML themselves, which goes to Microsoft, or in a library Volsa
  is built on, which goes to that library. If it reaches Volsa, tell us too.

## Reading reports

Security note S-UI-01. An export can hold structured reports (DICOM SR) and PDFs stored in
DICOM, and both come from anywhere. Volsa shows a report as plain text and saves a PDF as the
bytes it holds, and does nothing else with either.

- **Limits.** What Volsa turns into text is limited: a report is read no deeper than 16
  sections, into no more than 5,000 lines and 1,000,000 characters, whichever comes first.
  Where it goes past one, Volsa stops reading, keeps what it has, and says the report was cut
  short, so the lines shown are always the start of the report, in order. A line cut at the
  character limit ends in an ellipsis. The original report is the reference. These limits
  apply to Volsa's own reading of the report, after the DICOM library has read the file; see
  the last point.
- **Text only.** What a report says is turned into text. A link, a reference to an image or
  another object, or a waveform is never followed or fetched. Volsa writes a line that says
  what the item was and that it is not shown, so that nothing is left out without a word.
- **No PDF is drawn or opened.** Volsa has no PDF reader in it and starts no other program for
  a PDF. Its bytes are only written, whole, to the place the person chooses.
- **Nothing is executed.** A report or a PDF is read as data. No script, macro or embedded
  file in either is run, and none is written anywhere except the copy of the PDF the person
  asks for.
- **A damaged file is an error for that file.** A report or PDF that cannot be read is
  refused with an error, and so is one that makes the DICOM library panic, and Volsa goes on.
  A fault that stops the whole program is not caught; the one known, a file nested too
  deeply, is refused before the library reads it (see Files nested too deeply).
- **What the limits do not cover.** The DICOM library reads the whole file before Volsa walks
  the report, as it does for every file Volsa opens, so a file Volsa is given is still read
  into memory up to the size limit for one file (4 GiB). How deeply a file nests its
  sequences is checked before the DICOM library reads it, as for every file Volsa opens; see
  Files nested too deeply. A file that stops Volsa, or that makes the library run out of
  memory or time, is a fault to report here.

## Files nested too deeply

Security note S-DICOM-01. A DICOM file can hold sequences of items, and sequences inside those
items, as deep as it likes. The DICOM library Volsa reads with (dicom-rs) builds each level by
calling itself once more, with no limit of its own, so a damaged or crafted file nested deeply
enough would use up the stack of the thread reading it. That stops Volsa at once, and nothing
can catch it. Each level takes about 20 bytes of file, so a file of a few tens of kilobytes was
enough, and a deflated file could be smaller still.

- **The check.** Before the library is given a file, Volsa walks the same bytes with the
  library's own reader of one piece at a time, which keeps its place in memory rather than on
  the stack, set up as the library sets up its own read, a deflated file inflated as the
  library would inflate it. A file whose sequences nest more than 64 deep is refused unread.
  Real files nest far less: structured reports, the deepest, some ten to twenty.
- **Everywhere a file is read.** The check is made everywhere Volsa hands the library a file:
  reading headers while an export is indexed, reading what a document is (a report or a PDF)
  while it is listed, and opening a file whole to show its pictures, list its tags, read a
  report or save a PDF. Nesting after the pixels, which a header read never reaches, is
  checked when the file is opened whole.
- **What the person sees.** A refused file is named, with the reason "nested too deeply to read
  safely", in a warning that some files were not read, and the rest of the export opens. A file
  opened on its own says it was not read, and why. The log records only how many.
- **A second line.** Every thread that reads files has a 16 MiB stack, eight times the usual.
  Reading a file nested 64 deep takes at most about half a megabyte of it.

A file that still stops Volsa, for example through a way of nesting this check does not see, or
that makes the library run out of memory or time, is a fault to report here.

## Never include a real scan

A report must never include a real scan, a scan export, a label file made from a scan, a
screenshot that shows patient details, or tag values, not even privately. Make a file that
shows the fault instead. Volsa's tests build DICOM files with `volsa_dicom::testkit`, and

```
cargo run -p volsa-app --example write_phantom -- phantom
```

writes a made-up study to start from. If the fault shows only with a real scan, describe the
file's shape (how it is laid out, how large it is, which tags it has) and never its values.
