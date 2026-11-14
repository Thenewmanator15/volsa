Volsa
=====

Volsa is a viewer for MRI and CT scans, for 64-bit Windows. It opens a folder of
DICOM files, a zip of them or a DICOMDIR, and shows each series as slices, as
three planes and as a volume in 3D, on which lengths, angles and regions can
be measured. It can also show labels of the parts of a brain.

Volsa is not a medical device and is not for diagnostic use.
INTENDED-USE.txt, in this folder, says what it is for and what it is not for.


WHAT VOLSA IS FOR

Volsa is for looking at MRI and CT scans, and other DICOM files at a basic
level, on your own computer: your own scans, and scans used for learning,
teaching or research. It shows the images, lets you measure them and can show
labels made by a model, but it does not diagnose, screen for or rule out
anything, and its measurements and labels are not clinical results. Questions
about anyone's health belong with a qualified clinician. Volsa is not a medical
device and is not for diagnostic use.


WHAT YOU NEED

- 64-bit Windows 10 or 11.
- A graphics device with DirectX 12 or Vulkan. If Volsa cannot open its
  window, it says why in a message and closes.
- To label a brain scan, which is optional: the free Microsoft Visual C++
  2015-2022 x64 redistributable and a model folder you make yourself.
  MODELS.md, in this folder, says how.


STARTING VOLSA

1. Unzip the zip before you start Volsa: right-click it, choose Extract
   All, and pick a folder of your own, such as one in Documents. Volsa has
   no installer.
2. Open the folder volsa-<version> and start volsa.exe.
3. Open a scan with Open, at the top left of the window, or drop a folder,
   a zip or a DICOMDIR on the window.

Windows may warn you the first time. Volsa is not signed with a
code-signing certificate, so Microsoft Defender SmartScreen may say
"Windows protected your PC" and give the publisher as unknown. Only if you
trust where you got the zip, choose More info and then Run anyway.

To be sure the zip is the one that was published, compare its SHA-256 with
the one in the .sha256 file beside it on the Releases page. In PowerShell,
in the folder that holds the zip and that file, this prints True when the
two match:

    $zip = "volsa-<version>-windows-x64.zip"
    (Get-FileHash $zip).Hash -eq (Get-Content "$zip.sha256").Split()[0]

If they differ, do not run anything from that zip.

The question mark at the top of the window opens the help: short topics,
then every key.


WHAT VOLSA KEEPS ON THIS COMPUTER, AND REMOVING IT

Volsa makes no network connections and has no telemetry. It keeps what it
needs between runs in two folders of its own:

- %APPDATA%\Volsa\config holds the window's settings (preferences.json)
  and, in plugins, a copy of each plugin you added. Nothing in it is
  about any scan. Plugins are models made by others, added under Added
  by you in the Tools tab; the guide's Plugins page says more.
- %LOCALAPPDATA%\Volsa\data holds what comes of your scans: the views you
  saved, your measurements, your choices for labelling, labelling results
  kept to be shown again, and the log, volsa.log. Nothing in it names a
  patient, but a kept label file is a map of someone's head and a
  measurement holds whatever name you typed, so treat this folder as you
  would the scans.

Apart from these, Volsa writes a file only when you save one, in the place
you choose.

To remove Volsa, close it and delete the volsa-<version> folder you
unzipped, with the models folder in it, if you made one. Then type
%APPDATA%\Volsa into File Explorer's address bar and delete that folder,
and do the same for %LOCALAPPDATA%\Volsa.


MORE

The rest is in Volsa's user guide, https://thenewmanator15.github.io/volsa/ :
getting round the window, what patient details Volsa hides and
which it does not, what each saved file holds, labelling a brain scan, and
building Volsa yourself.

Report a fault at https://github.com/Thenewmanator15/volsa/issues . Never
post a scan, a scan export, a label file made from a scan, or a screenshot
that shows patient details: anyone can read an issue. A security problem
is reported privately, at
https://github.com/Thenewmanator15/volsa/security/advisories/new


THE FILES IN THIS FOLDER

    volsa.exe                 the program
    README.txt                this file
    INTENDED-USE.txt          what Volsa is for
    MODELS.md                 how to set up labelling
    CHANGELOG.md              what changed in each version
    LICENSE-MIT               Volsa's licences: either one, at your choice
    LICENSE-APACHE
    NOTICE                    Volsa's notice and SynthSeg's attribution
    THIRD-PARTY-NOTICES.txt   the licences of the libraries built into Volsa
    runtime\                  ONNX Runtime, with its own licence and notices
