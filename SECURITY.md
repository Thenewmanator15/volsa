# Security

Voxa opens files that come from elsewhere, and to label a scan it loads a library from a folder.
A fault in either can matter, so please report a vulnerability privately, where it can be fixed
before it is widely known.

## How to report one

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and choose
**Report a vulnerability**, or go straight to [the form](../../security/advisories/new). Only
you and the people who look after Voxa see the report, and the conversation about it stays
there. Do not open a public issue, discussion or pull request about it.

Say which version of Voxa and which Windows, what you did, what happened and what you expected,
and, if you can, include a made-up file that makes it happen (see below).

## What counts

Voxa runs on a person's own computer, makes no network connections, and opens what it is given.
The faults that matter are those where something Voxa is handed makes it do what it should not.

- **The files Voxa reads, which may come from anyone.** DICOM files, folders and zip files of
  them, `DICOMDIR` files, NIfTI label files (`.nii`, `.nii.gz`) and their tables of names
  (`.json`, `.ctbl`), a model folder (`model.json` and its networks), and the files Voxa keeps
  in its own two folders. A file that makes Voxa run code, read or write outside the places
  it should, or take memory or time without end counts. A file that only makes Voxa stop or
  show something wrong is a fault too; if you are sure it does no more than that, an ordinary
  issue with a made-up file is welcome, and if you are not sure, report it here.
- **The runtime library.** To label a scan, Voxa loads ONNX Runtime (`onnxruntime.dll`, which
  in turn loads `onnxruntime_providers_shared.dll` and `DirectML.dll`) from a folder called
  `runtime` beside `voxa.exe`, or from the folder named under **Options** in the Parts tab. A
  way to make Voxa load a library from anywhere else, or one the person did not put there,
  counts. Voxa runs whatever library is in a folder the person names, with the person's own
  rights: that is how it is meant to work, so name only a folder that holds Microsoft's files.
- **Anything that sends data off the computer**, or that writes a patient's details where Voxa
  says it does not: the log, the folders where Voxa keeps things, or a saved picture.

These are not for this form:

- How well a label matches the anatomy. Voxa is not a medical device and is not for diagnostic
  use. A label made by a model is a guess, and a label that is wrong is an ordinary issue.
- A fault in ONNX Runtime or DirectML themselves, which goes to Microsoft, or in a library Voxa
  is built on, which goes to that library. If it reaches Voxa, tell us too.

## Reading reports

Security note S-UI-01. An export can hold structured reports (DICOM SR) and PDFs stored in
DICOM, and both come from anywhere. Voxa shows a report as plain text and saves a PDF as the
bytes it holds, and does nothing else with either.

- **Limits.** A report is read no deeper than 16 sections, into no more than 5,000 lines and
  1,000,000 characters, whichever comes first. Where it goes past one, Voxa stops reading, keeps
  what it has, and says the report was cut short, so the lines shown are always the start of
  the report, whole and in order. The original report is the reference.
- **Text only.** What a report says is turned into text. A link, a reference to an image or
  another object, or a waveform is never followed or fetched. Voxa writes a line that says
  what the item was and that it is not shown, so that nothing is left out without a word.
- **No PDF is drawn or opened.** Voxa has no PDF reader in it and starts no other program for
  a PDF. Its bytes are only written, whole, to the place the person chooses.
- **Nothing is executed.** A report or a PDF is read as data. No script, macro or embedded
  file in either is run, and none is written anywhere except the copy of the PDF the person
  asks for.
- **A damaged file is an error for that file.** A report or PDF that cannot be read, or that
  makes the DICOM library fail, is refused with an error, and Voxa goes on.
- **What the limits do not cover.** The DICOM library reads the whole file before Voxa walks
  the report, as it does for every file Voxa opens, so a file Voxa is given is still read
  into memory up to the size limit for one file (4 GiB). A report that tries to make that
  library run out of memory or time is a fault to report here.

## Never include a real scan

A report must never include a real scan, a scan export, a label file made from a scan, a
screenshot that shows patient details, or tag values, not even privately. Make a file that
shows the fault instead. Voxa's tests build DICOM files with `voxa_dicom::testkit`, and

```
cargo run -p voxa-app --example write_phantom -- phantom
```

writes a made-up study to start from. If the fault shows only with a real scan, describe the
file's shape (how it is laid out, how large it is, which tags it has) and never its values.
