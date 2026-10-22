Voxa
====

Voxa is a viewer for MRI scans, for 64-bit Windows. It opens a folder of
DICOM files, a zip of them or a DICOMDIR, lists the studies and series in
it, and shows each series as slices, as three planes and as a volume in 3D,
on which lengths, angles and regions can be measured. It can also show
labels of the parts of a brain, made by a model on this computer.

Voxa is not a medical device and is not for diagnostic use.


WHAT VOXA IS FOR

Voxa is for looking at MRI scans on your own computer: your own scans, and
scans used for learning, teaching or research. It shows the images, lets
you measure them and can show labels made by a model, but it does not
diagnose, screen for or rule out anything, and its measurements and labels
are not clinical results. Questions about anyone's health belong with a
qualified clinician. Voxa is not a medical device and is not for diagnostic
use.


WHAT YOU NEED

- 64-bit Windows 10 or 11.
- A graphics device with DirectX 12 or Vulkan. If Voxa cannot open its
  window, it says why in a message and closes.
- To label a brain scan, which is optional: see LABELLING A BRAIN SCAN.

Voxa has been tried on Windows 11, with an NVIDIA graphics card and with
Intel integrated graphics. Windows 10 has not been tried.


STARTING VOXA

1. Unzip the zip before you start Voxa: right-click it, choose Extract
   All, and pick a folder of your own, such as one in Documents. Voxa has
   no installer.
2. Open the folder voxa-<version> and start voxa.exe.
3. Open a scan with Folder... or File... in the toolbar, or drop a folder,
   a zip or a DICOMDIR on the window.

Windows may warn you the first time. Voxa is not signed with a
code-signing certificate, so Microsoft Defender SmartScreen may say
"Windows protected your PC" and give the publisher as unknown. Only if you
trust where you got the zip, choose More info and then Run anyway.

To be sure the zip is the one that was published, compare its SHA-256
with the one published beside it on the Releases page, in the file whose
name ends in .sha256. In PowerShell, in the folder that holds the zip,

    Get-FileHash voxa-<version>-windows-x64.zip

prints it. If the two differ, do not run anything from that zip.

From a command prompt, voxa.exe PATH opens a scan at once, and
voxa.exe PATH --labels FILE opens it with labels another program made for
it. voxa.exe --help says the rest, in a small window.


PRIVACY

Voxa makes no network connections. It sends nothing anywhere, collects
nothing and has no telemetry. ONNX Runtime, which runs the labelling
networks, would report usage (not images) to Microsoft unless told not
to; Voxa tells it not to before it asks anything else of it.

Patient details are hidden. Each time Voxa starts, the patient's name and
identifier are left out of the viewports, and the tag browser hides every
value in the patient group. Patient details, in the toolbar, shows them
until you switch it off or close Voxa. The study's date and description
are shown either way.


WHAT VOXA KEEPS ON THIS COMPUTER, AND REMOVING IT

Voxa keeps what it needs between runs in two folders of its own:

    %APPDATA%\Voxa\config      its settings, the views and measurements
                               you saved, your choices for labelling, and
                               labelling results kept to be shown again
    %LOCALAPPDATA%\Voxa\data   voxa.log, for finding out what went wrong;
                               it never names a patient or a file

A kept labelling result is a map of someone's head, and a measurement
holds whatever name you typed, so treat these folders as you would the
scans. Apart from them, Voxa writes a file only when you save one, in the
place you choose.

To remove Voxa, close it and delete the voxa-<version> folder you
unzipped. Then type %APPDATA%\Voxa into File Explorer's address bar and
delete that folder, and do the same for %LOCALAPPDATA%\Voxa.


LABELLING A BRAIN SCAN

Voxa can label the structures of a brain on a scan, with SynthSeg 2.0's
networks run on this computer through ONNX Runtime. A label made by a
model is a guess that looks certain. To label a scan, open it, choose the
Parts tab and press Label this scan. That needs three things:

1. ONNX Runtime, which is in the runtime folder beside voxa.exe:
   Microsoft's onnxruntime.dll and onnxruntime_providers_shared.dll, with
   their LICENSE and ThirdPartyNotices.txt. A graphics card is used
   through DirectML, which Windows has; Voxa does not carry it.
2. The free Microsoft Visual C++ 2015-2022 x64 redistributable, which ONNX
   Runtime needs (Voxa itself does not). Many computers have it already.
   If yours does not, Voxa says so, and Microsoft offers it, as the X64
   one, at https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist
3. A model folder, which Voxa does not come with and never downloads. You
   make it once, from your own copy of SynthSeg 2.0's files, and put it
   beside voxa.exe as a folder called models. MODELS.md, beside voxa.exe,
   says where the files come from, how to make the folder, and how much
   memory a run needs.

Without them Voxa does everything else, and the button says what is
missing.


GETTING HELP

Report a fault, or ask for something Voxa does not do, at
https://github.com/Thenewmanator15/voxa/issues

Never post a scan, a scan export, a label file made from a scan, or a
screenshot or export that shows patient details, and never paste tag
values: anyone can read an issue. Describe what you saw in words. Even
with Patient details off, each viewport's corner shows the study's date
and description.

Please say which version of Voxa (the About window shows it), which
Windows and which graphics device, and include voxa.log, which the About
window locates. It holds nothing about a patient, but read it before you
post it.

A security problem is reported privately, never in an issue, at
https://github.com/Thenewmanator15/voxa/security/advisories/new


THE FILES IN THIS FOLDER

    voxa.exe                  the program
    README.txt                this file
    INTENDED-USE.txt          what Voxa is for
    MODELS.md                 how to set up labelling
    CHANGELOG.md              what changed in each version
    LICENSE-MIT               Voxa's licences: either one, at your choice
    LICENSE-APACHE
    NOTICE                    Voxa's notice and SynthSeg's attribution
    THIRD-PARTY-NOTICES.txt   the licences of the libraries built into Voxa
    runtime\                  ONNX Runtime, with its own licence and notices
