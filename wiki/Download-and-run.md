# Download and run

## Getting Volsa

1. Open the [Releases](https://github.com/Thenewmanator15/volsa/releases) page and, under
   **Assets** for the newest release, download `volsa-<version>-windows-x64.zip`.
2. Unzip it: right-click it, choose **Extract All…** and pick a folder of your own, such as one
   in Documents. Volsa has no installer. The zip holds one folder, `volsa-<version>`, with
   `volsa.exe`, a short `README.txt`, `INTENDED-USE.txt` (what Volsa is for and what it is not
   for), the licences and notices, `MODELS.md` (how to set up labelling), the changelog, and a
   `runtime` folder with ONNX Runtime, which labelling uses.
3. Start `volsa.exe`. Open a scan with **Open** at the top left and then **Folder…** or **Zip
   file or DICOMDIR…** (a folder of DICOM files, a zip of them, or a `DICOMDIR`), or drop it on
   the window.

## If Windows warns you

Volsa is not signed with a code-signing certificate, so the first time it starts Microsoft
Defender SmartScreen may say "Windows protected your PC" and give the publisher as unknown. Only
if the zip came from this repository's Releases page, choose **More info** and then
**Run anyway**.

## Checking the download

To be sure the zip is the one that was published, compare its SHA-256 with the one published
beside it on the Releases page, in the file whose name ends in `.sha256`. In PowerShell, in the
folder that holds the zip,

```powershell
Get-FileHash volsa-<version>-windows-x64.zip
```

prints it. `Get-FileHash` prints the hash in capital letters and the `.sha256` file has it in
small letters; the case does not matter, only the letters and digits. With the zip and its
`.sha256` file in one folder, these two lines, with the version put in, print `True` when the
two match:

```powershell
$zip = "volsa-<version>-windows-x64.zip"
(Get-FileHash $zip).Hash -eq (Get-Content "$zip.sha256").Split()[0]
```

If the two differ, do not run anything from that zip.

## From a command prompt

`volsa.exe PATH` opens a scan at once, and `volsa.exe PATH --labels FILE` opens it with labels
another program made for it. `volsa.exe --help` says the rest, in a small window.

## Getting round the window

Volsa opens at a size that fits your screen, and its window can be made as small as 640 by 480
at your display's scale. The window has a bar across the top (**Open**, the study's name and
date, the search box, help, the theme and a menu, **⋯**), the rail of scans down the left, the
pictures in the middle and, at the right, a panel of tabs: **View**, **Measure**, **3D**,
**Labels** and **Tools**. Along the bottom are the disclaimer and the notices. The search box,
which Ctrl+K or `/` opens, finds the actions by their names and by other words for them, and
also the parts of open labels, the scans of the export and the help topics. **?** opens the
help: seven short topics, then every key.

An export opens on its slices. The strip above the pictures arranges them: **Slices** (with one,
two or four viewports), **Slices + 3D**, **Planes** (the three planes and the 3D view, two by
two) or **3D** alone. **B** folds the rail of scans to a strip, and **P** the tab panel, as do
the **«** at the top of the rail and the **»** at the top of the panel; a click on a strip
opens it again. The rail starts folded in a window under 960 points across; under 760 both
fold, and no more than one is open at a time, so that the pictures keep their room. The edge of
the rail and the edge of the panel can be dragged to give either more of the width. **F11** is
full screen, and **Size** in the **⋯** menu draws the words and buttons larger or smaller, as do
Ctrl+= and Ctrl+- (Ctrl+0 puts them back).

Volsa remembers the window's size, the size of the words and buttons, how wide you dragged the
rail and the panel, which of them you folded, which tab is open and the theme (light, dark or
the system's). Full screen, which scans you opened, what you typed in the search box and whether
patient details are shown start afresh each time.

## More than scans

An export can hold more than scans. The rail lists locating images, scanned pages, reports and
other files under **Other files**, which a click opens. A report opens as text in the middle of
the window, headed "Shown as text. The original report is the reference.", and its dates are
written in words. A PDF stored in the export is not shown, since Volsa has no PDF reader and
starts no other program; **Save a copy as PDF…** writes a copy where you choose. Another kind of
file is named, with its kind in words, and its tags can be shown.

See also [[A closer look]], [[System requirements]] and [[Removing Volsa]].
