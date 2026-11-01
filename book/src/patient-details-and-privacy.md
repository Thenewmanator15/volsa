# Patient details and privacy

Volsa makes no network connections. It sends nothing anywhere, collects nothing and has no
telemetry. ONNX Runtime, which runs the labelling networks, reports usage (not images) in
Microsoft's Windows builds unless told not to; Volsa tells it not to before it asks anything
else of it.

## Patient details are hidden

Each time Volsa starts, the patient's name and identifier are left out of the rail of scans, the
viewports, the titles of documents and reports, and Volsa's notices. In a report's text they are
shown as `[hidden]`, and a report item that holds a name shows as "Name hidden".

The **DICOM tags** window (**Show the file's tags**, in the menu of a scan's card in the rail, or
found by the search box) hides, and cannot find by searching, the values of these tags, and only
these:

- every tag in the patient group, 0010;
- every private tag, which is any in a group of odd number, since what a maker keeps there is
  not said;
- and this list: the accession number; the institution's name, address and department; the
  referring physician's name, address and telephone numbers; the physicians of record; the
  performing, reading, requesting and scheduled performing physicians; the operators; the
  station's name, the performed station's name and the performed location; the device's serial
  number; the admission ID; the placer order number; the requested procedure ID; and the
  scheduled and performed procedure step IDs.

The window says how many values it has hidden. A sequence, a tag that holds other tags, is shown
only as the number of items in it. **Show patient details** in the View tab shows the hidden
values until you switch it off or close Volsa.

## What may still show

**A name written some other way inside a report's own text** (with an initial, a title, another
spelling, or a surname alone) cannot be found reliably and may show; an identifier under four
letters or digits is not looked for, so that it cannot change a number. When Volsa has replaced
something in a report it says so under the report's heading.

**Every other value stays shown**, and some of them can still identify a scan or lead to the
person in it: dates and times, unique identifiers (UIDs), the study ID, and free-text comments
and descriptions among them. The study's date and description are in the top bar and in each
viewport's corner either way. Take care with a screenshot of the **DICOM tags** window, a report
or a viewport, even with **Show patient details** off.

A file is named in Volsa's notices by its own name, without the folders it lies in, since a
folder can be named for the patient.

What Volsa keeps on this computer is on [What Volsa keeps on your computer](what-volsa-keeps-on-your-computer.md). When asking for
help, see [Getting help and reporting a fault](getting-help-and-reporting-a-fault.md) for what never to post.
