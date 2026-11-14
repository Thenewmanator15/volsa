# What Volsa keeps on your computer

Volsa makes no network connections. It sends nothing anywhere, collects nothing and has no
telemetry. What it keeps between runs stays on this computer, in two folders of its own.

## Settings

In `%APPDATA%\Volsa\config`:

| File | What it holds |
|---|---|
| `preferences.json` | The window's size, the size of the interface, how wide the rail of scans and the tab panel are, whether you folded either, which tab is open, the theme, how many viewports, the **Patient's left on the right** and **Link the views** switches, and how labels are drawn over the planes. Nothing about any scan. |
| `plugins` | A copy of each [plugin](plugins.md) you added: its model and its `plugin.json`, made by someone else. Nothing about any scan. |

`%APPDATA%` is the roaming part of your Windows profile, which a managed network may copy to
its servers, so it holds nothing about any scan.

## What comes of your scans, and the log

In `%LOCALAPPDATA%\Volsa\data`, which stays on this computer:

| File | What it holds |
|---|---|
| `views.json` | The views of the 3D scene you saved, for each series. |
| `annotations.json` | Your markers, rulers, angles and regions for each series, with the names you gave them. |
| `places.json` | Where you left each series you changed the slice, the window or the view of: the slice, the window if you set one yourself, and the zoom, move, flips and turns; and for each study, which series was last in the first viewport. Numbers only, for at most 500 series and 500 studies; the oldest are dropped. No date or time. |
| `choices.json` | What you chose for labelling under **Options**, with the two folders you named, if any. |
| `labels\` | Labelling results kept to be shown again: for each series, a label file and the table of its parts' names. |
| `volsa.log` | The log, below. |

Each series, and each study in `places.json`, is filed under a 16-digit hash of its identifier,
not under the identifier, and nothing in these files names a patient. Someone who holds the
same scan could work out its hash, and so tell that it was opened here. Still, a kept label file is a map of someone's head and
an annotation holds whatever name you typed, so treat this folder as you would the scans.
Labelling results are kept unless you switch off **Keep results on this computer** under
**Options** in the Tools tab, and **Delete**, beside a kept result, removes it.

While a very large scan is open, slabs of it past Volsa's memory budget wait in a file in this
folder that only Volsa can open. The file is deleted when the scan is closed, when Volsa closes,
and at the next start after a stop.

## The log

`%LOCALAPPDATA%\Volsa\data\volsa.log` is for working out why something went wrong. It says when
Volsa started, its version and the system, which graphics device it draws with, how many
studies, series and skipped files an export held, the messages Volsa showed (a message whose
words may name a file is logged by its kind alone), and where a crash happened. It never holds a
patient's name, identifier or date, a value from a file, or the name of any of your files.

A crash is logged by the place in the source code where it happened: a file and line in Volsa's
own code or in a library's. For a library that place can be a path on the computer Volsa was
built on, which for the program in a download is not yours, so its log holds no path on your
computer; for a Volsa you built yourself, it can be a path on your own. Once the log passes 1 MB
it is cut back to its newest 512 KB the next time Volsa starts. The **About** window shows where
it is, with a button that copies the path.

## The files you save

Apart from these folders, Volsa writes a file only when you save one, in the place you choose: a
picture or a turntable of the 3D scene (which holds the picture and nothing else from the scan),
measurements, the list of parts, labels, or a copy of a PDF from the export.

Every table Volsa saves (CSV) has a column, `notice`, that says "Volsa is not a medical device
and is not for diagnostic use." on every line, so the sentence goes wherever a line is copied,
sorted or filtered. The table of measurements ends with one more column, `scale`: `patient`
where the files say where the slices lie, `pixel spacing` where the millimetres come from the
picture's own pixel spacing alone. Measurements saved as JSON say the same in `scale` and carry
the sentence as `notice`. The table of parts also says on every line which model made the labels
(`model`: the model Volsa ran, "made by another program", or "not said" when the labels name no
model) and how the model's own check of its work went (`model_check`: `passed`, `failed`, or
`not given`).

What Volsa hides of a patient, and what it does not, is on [Patient details and privacy](patient-details-and-privacy.md). To
delete these folders, see [Removing Volsa](removing-volsa.md).
