Voxa
====

Voxa is a viewer for MRI and CT scans, for 64-bit Windows. It opens a folder of
DICOM files, a zip of them or a DICOMDIR, lists the studies and series in
it, and shows each series as slices, as three planes and as a volume in 3D,
on which lengths, angles and regions can be measured. It can also show
labels of the parts of a brain, made by a model on this computer.

Voxa is not a medical device and is not for diagnostic use.


WHAT VOXA IS FOR

Voxa is for looking at MRI and CT scans, and other DICOM files at a basic
level, on your own computer: your own scans, and scans used for learning,
teaching or research. It shows the images, lets you measure them and can show
labels made by a model, but it does not diagnose, screen for or rule out
anything, and its measurements and labels are not clinical results. Questions
about anyone's health belong with a qualified clinician. Voxa is not a medical
device and is not for diagnostic use.


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
3. Open a scan with Open, at the top left of the window, and then
   Folder... or Zip file or DICOMDIR... (a folder of DICOM files, a zip of
   them, or a DICOMDIR), or drop a folder, a zip or a DICOMDIR on the window.

The window has a bar across the top (Open, the study's name and date, the
search box, help, the theme and a menu, marked with three dots), a column
of scans down the left (the rail), the pictures in the middle and, at the
right, a panel of tabs: View, Measure, 3D, Labels and Tools. Along the
bottom are the notice that Voxa is not a medical device and any messages
from Voxa. The search box, which Ctrl+K or / opens, finds the actions by
name and by other words for them, and also the parts of open labels, the
scans of the export and the help topics. The question mark opens the help:
seven short topics, then every key.

Windows may warn you the first time. Voxa is not signed with a
code-signing certificate, so Microsoft Defender SmartScreen may say
"Windows protected your PC" and give the publisher as unknown. Only if you
trust where you got the zip, choose More info and then Run anyway.

To be sure the zip is the one that was published, compare its SHA-256
with the one published beside it on the Releases page, in the file whose
name ends in .sha256. In PowerShell, in the folder that holds the zip,

    Get-FileHash voxa-<version>-windows-x64.zip

prints it. Get-FileHash prints the hash in capital letters and the .sha256
file has it in small letters; the case does not matter, only the letters
and digits. With the zip and its .sha256 file in one folder, these two
lines, with the version put in, print True when the two match:

    $zip = "voxa-<version>-windows-x64.zip"
    (Get-FileHash $zip).Hash -eq (Get-Content "$zip.sha256").Split()[0]

If the two differ, do not run anything from that zip.

From a command prompt, voxa.exe PATH opens a scan at once, and
voxa.exe PATH --labels FILE opens it with labels another program made for
it. voxa.exe --help says the rest, in a small window.


PRIVACY

Voxa makes no network connections. It sends nothing anywhere, collects
nothing and has no telemetry. ONNX Runtime, which runs the labelling
networks, would report usage (not images) to Microsoft unless told not
to; Voxa tells it not to before it asks anything else of it.

Patient details are hidden. Each time Voxa starts, the patient's name and
identifier are left out of the rail of scans, the viewports, the titles of
documents and reports, and Voxa's messages. In a report's text they are
shown as [hidden], and a report item that holds a name shows as "Name
hidden". The DICOM tags window (Show the file's tags, in the menu of a
scan's card in the rail, or found by the search box) hides, and cannot find
by searching, the values of these tags, and only these:

- every tag in the patient group, 0010;
- every private tag, which is any in a group of odd number, since what a
  maker keeps there is not said;
- and this list: the accession number; the institution's name, address
  and department; the referring physician's name, address and telephone
  numbers; the physicians of record; the performing, reading, requesting
  and scheduled performing physicians; the operators; the station's
  name, the performed station's name and the performed location; the
  device's serial number; the admission ID; the placer order number; the
  requested procedure ID; and the scheduled and performed procedure step
  IDs.

The window says how many values it has hidden. A sequence, a tag that
holds other tags, is shown only as the number of items in it. Show patient
details, in the View tab, shows the hidden values until you switch it off
or close Voxa.

A name written some other way inside a report's own words (with an
initial, a title, another spelling, or a surname alone) cannot be found
reliably and may show, and an identifier shorter than four letters or
digits is not looked for, so that it cannot change a number. When Voxa has
replaced something in a report it says so under the report's heading.

Every other value stays shown, and some of them can still identify a
scan or lead to the person in it: dates and times, unique identifiers
(UIDs), the study ID, and free-text comments and descriptions among them.
The study's date and description are in the top bar and in each
viewport's corner either way. Take care with a screenshot of the DICOM tags
window, a report or a viewport, even with Show patient details off.


WHAT VOXA KEEPS ON THIS COMPUTER, AND REMOVING IT

Voxa keeps what it needs between runs in two folders of its own.

%APPDATA%\Voxa\config holds one file, preferences.json, and nothing about
any scan: the window's size, the size of the interface, how wide the rail
of scans and the tab panel are, whether you folded either, which tab is
open, the theme, how many viewports, the "Patient's left on the right"
and "Link the views" switches, and how labels are drawn over the planes.
%APPDATA% is the roaming part of your Windows profile, which a managed
network may copy to its servers, so Voxa keeps nothing else there.

%LOCALAPPDATA%\Voxa\data, which stays on this computer, holds what comes
of your scans, and the log:

    views.json         the views of the 3D scene you saved
    annotations.json   your markers, rulers, angles and regions, with the
                       names you gave them
    choices.json       your choices for labelling, with the two folders
                       you named, if any
    labels\            labelling results kept to be shown again: for
                       each series, a label file and the table of its
                       parts' names
    voxa.log           the log, below

Each series is filed under a 16-digit hash of its identifier, not under
the identifier, and nothing in these files names a patient. Still, a kept
label file is a map of someone's head and an annotation holds whatever
name you typed, so treat this folder as you would the scans. Labelling
results are kept unless you switch off Keep results on this computer,
under Options in the Tools tab, and Delete, beside a kept result, removes
it.

A version of Voxa before 0.6.0 kept the views, annotations, choices and
labelling results in %APPDATA%\Voxa\config. The first time Voxa 0.6.0 or
later starts, it moves them to %LOCALAPPDATA%\Voxa\data, each file whole.
A file that is already there is not written over, and the old one is left
where it was. The log says what was moved and what was left.

The log, voxa.log, is for finding out why something went wrong. It says
when Voxa started, its version and the system, which graphics device it
draws with, how many studies, series and skipped files an export held,
the messages Voxa showed (one whose words may name a file is logged by
its kind alone), and where a crash happened. It never holds a patient's
name, identifier or date, a value from a file, the name of any of your
files, or a path on your computer. A crash is logged by the place in the
source code where it happened: a file and line in Voxa's own code or in
a library's, which for a library can be a path on the computer that
built this release. Once the log passes 1 MB it is cut back to its newest
512 KB the next time Voxa starts. The About window shows where it is.

Apart from these folders, Voxa writes a file only when you save one, in
the place you choose: a picture or a turntable of the 3D scene (which holds
the picture and nothing else from the scan), measurements, the list of
parts, labels, or a copy of a PDF from the export.

A table Voxa saves (CSV) has a column, notice, that says "Voxa is not a
medical device and is not for diagnostic use." on every line, so that the
sentence goes wherever a line is copied, sorted or filtered. The table of
measurements ends with one more column, scale: "patient" where the files
say where the slices lie, "pixel spacing" where the millimetres come from
the picture's own pixel spacing alone. Measurements saved as JSON say the
same in scale and carry the sentence as notice.

Voxa 0.6.0 reads annotations.json only as it wrote it. When a measurement
is in a picture's own pixel spacing the file is written in a newer form,
which 0.6.0 does not read: it shows no measurements and, the first time it
saves, sets the file aside as annotations.json.unread, which no version
reads back. A file with only measurements in the patient's millimetres is
written as 0.6.0 wrote it.

To remove Voxa, close it and delete the voxa-<version> folder you
unzipped, with the models folder in it, if you made one. Then type
%APPDATA%\Voxa into File Explorer's address bar and delete that folder,
and do the same for %LOCALAPPDATA%\Voxa.


LABELLING A BRAIN SCAN

Voxa can label the structures of a brain on a scan, with SynthSeg 2.0's
networks run on this computer through ONNX Runtime. A label made by a
model is a guess that looks certain. To label a scan, open it, choose the
Tools tab and press Label this scan. That needs three things:

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
with Show patient details off, the top bar and each viewport's corner show
the study's date and description, and a report may hold a name written some
way Voxa cannot find.

Please say which version of Voxa (the About window shows it), which
Windows and which graphics device, and include voxa.log, which the About
window locates. It holds nothing about a patient and none of your files'
names, but read it before you post it.

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
