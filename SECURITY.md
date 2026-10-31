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
  (`.json`, `.ctbl`), a model folder (`model.json` and its networks), a plugin folder
  (`plugin.json` and its network, [below](#plugins)), and the files Voxa keeps
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

## Plugins

A plugin is a folder a person adds: a `plugin.json` and an ONNX network someone else made
([docs/plugins.md](docs/plugins.md)). Its files can come from anywhere, so they count as files
Voxa reads, above, and so does anything that makes Voxa run a plugin network other than the one
the person added.

- **S-PL-01.** Voxa accepts only standard ONNX operators, reads the network file's structure
  itself, with limits, before ONNX Runtime sees it, and runs it in a helper process
  (`voxa.exe --plugin-helper`) held in a Windows job object that ends it with Voxa, lets it
  start no process, and caps its memory. The helper has the person's own rights: it is not a
  sandbox. Add plugins only from sources you trust.
- **S-PL-02.** Names in a description never lead outside its folder: only plain names are
  taken, links are not followed, and Voxa's copy lives in a folder named by a sum, never by
  anything the description says.
- **S-PL-03.** The helper checks the network file's SHA-256 before it opens it, on every run.
- **S-PL-04.** A description's words are shown as plain text inside the "Added by you" frame,
  never as links and never as Voxa's own words.
- **S-PL-05.** Scan values pass between Voxa and its helper through pipes only; none is written
  to disk.
- **S-PL-06.** The helper's runtime has its telemetry turned off before anything else, as
  Voxa's own is.

A fault in a plugin's model itself, such as wrong labels, is its maker's and not for this form.
A network file that gets past the checks above and makes the helper, or Voxa, do what it should
not is.

## Never include a real scan

A report must never include a real scan, a scan export, a label file made from a scan, a
screenshot that shows patient details, or tag values, not even privately. Make a file that
shows the fault instead. Voxa's tests build DICOM files with `voxa_dicom::testkit`, and

```
cargo run -p voxa-app --example write_phantom -- phantom
```

writes a made-up study to start from. If the fault shows only with a real scan, describe the
file's shape (how it is laid out, how large it is, which tags it has) and never its values.
