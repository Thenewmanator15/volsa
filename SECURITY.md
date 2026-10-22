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
  in its own settings folder. A file that makes Voxa run code, read or write outside the places
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
  says it does not: the log, the settings folder, or a saved picture.

These are not for this form:

- How well a label matches the anatomy. Voxa is not a medical device and is not for diagnostic
  use. A label made by a model is a guess, and a label that is wrong is an ordinary issue.
- A fault in ONNX Runtime or DirectML themselves, which goes to Microsoft, or in a library Voxa
  is built on, which goes to that library. If it reaches Voxa, tell us too.

## Never include a real scan

A report must never include a real scan, a scan export, a label file made from a scan, a
screenshot that shows patient details, or tag values, not even privately. Make a file that
shows the fault instead. Voxa's tests build DICOM files with `voxa_dicom::testkit`, and

```
cargo run -p voxa-app --example write_phantom -- phantom
```

writes a made-up study to start from. If the fault shows only with a real scan, describe the
file's shape (how it is laid out, how large it is, which tags it has) and never its values.
