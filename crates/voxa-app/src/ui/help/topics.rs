//! The help topics: short, plain pieces of help for people looking at their own scans, most
//! of them with no medical or technical training and often anxious about what a scan shows.
//!
//! Each is at most six short sentences. It says what Voxa does and where to find it, and says
//! no more than Voxa knows; a word of the trade is explained in the sentence that uses it, or
//! left out. None says what a scan shows: the wording test holds them to that, and each was
//! read against the risk register (`docs/quality/risk-register.md`) and the statement of what
//! Voxa is for (`docs/intended-use.md`). A button, box or tab is named as the window draws it,
//! which a test checks against what the window draws.

/// A short piece of help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Topic {
    /// What the topic is known by in the code, which never changes: `measure`.
    pub id: &'static str,
    /// What the help and search call it. It is a heading of one line, so it is short.
    pub title: &'static str,
    /// Other words search finds it by. They are never drawn.
    pub words: &'static [&'static str],
    /// What it says, in at most six short sentences. A blank line starts a new paragraph.
    pub body: &'static str,
}

/// The topics, in the order the help lists them.
pub const TOPICS: [Topic; 7] = [
    Topic {
        id: "open",
        title: "Opening your scans",
        words: &[
            "open", "folder", "zip", "cd", "dvd", "disc", "drive", "dicom", "dicomdir", "export",
            "download", "load", "drag", "drop", "start",
        ],
        body: "Choose Open at the top of the window, then Folder… to pick a folder, or Zip \
               file or DICOMDIR… to pick a zip file or a DICOMDIR (a file that lists the scans \
               on a disc). You can also drag a folder or a file onto the window.\n\n\
               Scans from a hospital often come on a CD, usually in a folder named DICOM, \
               beside a file named DICOMDIR. Put the CD in, choose Folder… and pick the CD's \
               drive: Voxa looks inside every folder on it, so you need not find the scans \
               yourself. A download is often a zip file, which Voxa opens without your \
               unpacking it. Voxa never changes your files and sends nothing anywhere.",
    },
    Topic {
        id: "find",
        title: "Choosing your scan",
        words: &[
            "find",
            "which",
            "scan",
            "scans",
            "list",
            "names",
            "series",
            "other files",
            "reports",
            "pages",
            "people",
            "person",
            "card",
            "mark",
            "plain name",
        ],
        body: "A hospital's copy often holds many scans, listed down the left as cards that \
               you click to show. Each card has a plain name that Voxa makes from what the \
               scanner wrote, and the scanner's own name beneath it; if the two seem to \
               disagree, go by the scanner's own. Voxa starts on the largest scan it can show \
               in full, which says nothing about which scan matters.\n\n\
               Reports, scanned pages and pictures taken to plan other scans wait under \
               Other files, and a report opens as text that says the original report is the \
               reference. A small amber mark on a scan's picture means Voxa cannot show it in \
               full: rest the pointer on the card to read why. If the list groups scans as \
               Person 1, Person 2 and so on, they belong to different people: do not compare \
               them as if they were one person's.",
    },
    Topic {
        id: "brightness",
        title: "Brightness and contrast",
        words: &[
            "brightness",
            "contrast",
            "window",
            "level",
            "width",
            "dark",
            "bright",
            "light",
            "grey",
            "preset",
            "hounsfield",
            "ct",
            "mri",
        ],
        body: "Brightness and contrast change how the picture is drawn, not the scan itself. \
               The View tab has a button for each setting: Brain, Soft tissue, Lung, Bone and \
               Auto for a CT scan, and Auto, Soft and Hard for an MRI scan.\n\n\
               To set it by hand, change the box marked W, the width: how wide a range of the \
               scan's numbers runs from black to white, so a narrower range gives more \
               contrast. Change the box marked L, the level, to choose which number is drawn \
               as middle grey, or drag with the right mouse button over the picture: across \
               changes the width, down changes the level. The CT buttons suit CT scans \
               measured in Hounsfield units, a standard scale of density, so they are greyed \
               out, with the reason, when a scan's numbers are not in that scale. A picture \
               in colour, such as a scanned page, has no brightness to set.",
    },
    Topic {
        id: "measure",
        title: "Measuring and millimetres",
        words: &[
            "measure",
            "ruler",
            "length",
            "distance",
            "angle",
            "region",
            "marker",
            "millimetres",
            "mm",
            "scale",
            "spacing",
            "size",
            "pixel spacing",
            "units",
        ],
        body: "The Measure tab (or the M key) has a Ruler for lengths, an Angle, a Region \
               that measures inside an ellipse you draw, and a Marker.\n\n\
               Millimetres come only from the scan's files: when they give the size of each \
               dot, or pixel, of the picture (the pixel spacing) and say where each slice lies \
               in the patient, lengths are in the patient's millimetres. When they give the \
               pixel spacing alone, a measurement stays within one picture and says \"from \
               the file's pixel spacing\", and it says nothing about the distance between \
               slices. When they give no pixel spacing, or the slices are not all the same \
               size or do not all face the same way, the scan is not measured in millimetres, \
               and the tools say why.\n\n\
               A scan with the amber mark may be only a stack of slices, not a regular 3D \
               volume; where it can be measured at all, a length within one slice holds but \
               the distance between slices is only rough. A measurement is only as good as \
               those numbers and the line you drew, and it is not a clinical result.",
    },
    Topic {
        id: "sides",
        title: "Telling left from right",
        words: &[
            "left",
            "right",
            "side",
            "sides",
            "orientation",
            "radiological",
            "neurological",
            "mirror",
            "flip",
            "letters",
            "front",
            "back",
        ],
        body: "Left and right in a scan are the patient's, not yours. Hospitals usually show \
               a scan as if you faced the patient, so the patient's left is on the right of \
               the screen, and Voxa starts that way. Press Patient's left on the right in the \
               View tab to turn it off and see the other way round; Voxa remembers your \
               choice.\n\n\
               Where the file says how the picture lies in the patient, each edge carries a \
               letter saying which way it faces; a picture without letters has no direction \
               in its file. R is the patient's right, L their left, A the front, P the back, S \
               the head and I the feet; they follow the picture when you mirror or turn it. \
               In the 3D tab, the buttons \"A front\", \"P back\", \"L left\", \"R right\", \
               \"S top\" and \"I bottom\" look at the scan from each of those sides.",
    },
    Topic {
        id: "labels",
        title: "Labels, and who made them",
        words: &[
            "labels",
            "label",
            "segmentation",
            "parts",
            "model",
            "synthseg",
            "brain",
            "colours",
            "check",
            "made by",
            "nifti",
        ],
        body: "Labels outline parts of the head in colour; Voxa makes them with a model, a \
               computer program that has learned to do this, when you press Label this scan \
               in the Tools tab. You can also open labels another program made, with Open \
               labels… in the Labels tab; they are marked \"Made by another program\" and \
               Voxa shows them as given.\n\n\
               A label is a guess that looks certain. The Labels tab shows the model's own \
               check of its work, but a check that passes is only the model's opinion of its \
               own work and does not make the labels right. If the check fails, the tab says \
               so first, and every part should be taken with caution. A part's size is worked \
               out from its label, so it is only as good as the label, and nothing about \
               anyone's health should be read into either.",
    },
    Topic {
        id: "not-for",
        title: "What Voxa is not for",
        words: &[
            "not for",
            "safe",
            "safety",
            "medical",
            "device",
            "clinical",
            "doctor",
            "clinician",
            "diagnosis",
            "diagnose",
            "diagnostic",
            "disclaimer",
            "warning",
            "worried",
            "urgent",
            "intended use",
            "treatment",
        ],
        body: "Voxa is not a medical device and is not for diagnostic use. It is for looking \
               at scans on your own computer, for learning, teaching and research, and it \
               does not diagnose, screen for or rule out anything.\n\n\
               Its measurements and labels are not clinical results, so do not use it to \
               decide anything about anyone's care, to read scans in a clinic or hospital, or \
               to plan a treatment or an operation. A report that came with a scan is meant \
               to be written by a qualified person who saw the whole of it and knew why it was \
               taken, and Voxa knows neither. If you are worried about what a scan shows, or \
               about how you feel, ask your doctor or another clinician, such as a nurse. Do \
               not wait for, or rely on, anything Voxa shows.",
    },
];
