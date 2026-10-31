//! The help topics: short, plain pieces of help for people looking at their own scans, most
//! of them with no medical or technical training and often anxious about what a scan shows.
//!
//! Each says what Voxa does and where to find it, and says no more than Voxa knows. None
//! says what a scan shows: the wording test holds them to that, and each was read against the
//! risk register (`docs/quality/risk-register.md`) and the statement of what Voxa is for
//! (`docs/intended-use.md`). The names of buttons are written as the window writes them, which
//! a test checks.

/// A short piece of help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Topic {
    /// What the topic is known by in the code, which never changes: `measure`.
    pub id: &'static str,
    /// What the help and search call it. It is a heading of one line, so it is short.
    pub title: &'static str,
    /// Other words search finds it by. They are never drawn.
    pub words: &'static [&'static str],
    /// What it says, in a few plain sentences. A blank line starts a new paragraph.
    pub body: &'static str,
}

/// The topics, in the order the help lists them.
pub const TOPICS: [Topic; 7] = [
    Topic {
        id: "open",
        title: "Opening your scans",
        words: &[
            "open", "folder", "zip", "cd", "dvd", "disc", "drive", "dicomdir", "export",
            "download", "load", "drag", "drop", "start",
        ],
        body: "Choose Open at the top of the window, then Folder… to pick a folder, or Zip \
               file or DICOMDIR… to pick one of those. You can also drag a folder or a file \
               onto the window.\n\n\
               Scans from a hospital often come on a CD. Put it in, choose Folder… and pick \
               the CD's drive: Voxa looks inside every folder on it, so you do not need to \
               find the scans yourself. A download is often a zip file, which Voxa opens \
               without your unpacking it. Voxa never changes your files and sends nothing \
               anywhere.",
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
        body: "An export often holds many scans. They are listed down the left, each with a \
               plain name that Voxa makes from what the scanner wrote and what the file says, \
               and the scanner's own name in small type beneath it. If the two ever seem to \
               disagree, go by the scanner's own name. Voxa starts on the largest scan it can \
               show in full; that is only somewhere to begin, and says nothing about which \
               scan matters.\n\n\
               Click a card to show that scan, or use search to look for a name. Reports, \
               scanned pages and locating images are kept apart, under Other files at the \
               bottom of the list; a report opens as text, headed \"Shown as text. The \
               original report is the reference.\" A small amber mark on a scan's picture \
               means Voxa cannot show it in full: rest the pointer on the card to read why. \
               If the list says the export holds scans of more than one person, it groups \
               them as Person 1, Person 2 and so on, so do not compare scans of different \
               people as if they were one person's.",
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
               The View tab has a button for each setting: for a CT scan Brain, Soft tissue, \
               Lung, Bone and Auto, and for an MRI scan Auto, Soft and Hard. Try them: the \
               same scan looks very different with each, because only the drawing changes.\n\n\
               To set it by hand, change Window width (the span of values drawn from black to \
               white; a narrower span gives more contrast) and Window level (the value drawn \
               as mid-grey) in the View tab, or drag with the right mouse button over the \
               picture: across for width, down for level. The CT settings are made for the \
               CT scale of Hounsfield units, so they are greyed out, with the reason, when a \
               scan's values are not in it. A picture in colour, such as a scanned page, has \
               no brightness to set.",
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
        body: "The Measure tab (or the M key) has a Ruler for lengths, an Angle, a Region for \
               the values inside an ellipse you draw, and a Marker.\n\n\
               Millimetres come only from the scan's files, in one of three ways. When the \
               files give the size of a pixel and say where each slice lies in the patient, \
               lengths are in the patient's millimetres. When they give the size of a pixel \
               alone, a measurement is made within the picture and says \"from the file's \
               pixel spacing\"; it says nothing about the distance between slices. When they \
               give no pixel spacing, the scan is not measured in millimetres, and the tools \
               say why: \"Measuring needs a scan with a scale in its files.\" A spacing that \
               belongs to an X-ray detector, not the body, is never used.\n\n\
               A measurement is only as good as those numbers and the line you drew. It is \
               not a clinical result. If a scan has the amber mark because it is shown as a \
               plain stack, what you measure within one slice holds, but the distance between \
               slices is only a rough one.",
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
        body: "Left and right in a scan are the patient's, not yours. Hospitals usually show a \
               scan as if you faced the patient, so the patient's left is on the right of the \
               screen. Voxa starts that way and remembers your choice; to see it the other way \
               round, press Patient's left on the right in the View tab to turn it off.\n\n\
               Where the file says how the picture lies in the patient, a letter at each edge \
               says which way that edge faces: R and L for the patient's right and left, A for \
               the front, P for the back, S for the head and I for the feet. The letters \
               follow the picture when you mirror or turn it, so go by the letters, not by how \
               the picture looks. A picture without letters has no direction in its file. In \
               the 3D tab, the buttons A, P, L, R, S and I look at the scan from each of \
               those sides.",
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
        body: "Labels outline parts of the head on a scan, each part in its own colour. The \
               Labels tab says who made them before it lists any part: Voxa, with the model's \
               own check of its work, or \"Made by another program\", which Voxa shows as given \
               and cannot vouch for. You can open a file of labels with Open labels… in the \
               Labels tab, or have Voxa make them, on your own computer, with Label this \
               brain scan in the Tools tab.\n\n\
               Take a label as a guess that looks certain. A check that passes only means the \
               model is content with its own work; it does not make the labels right. If the \
               check fails, the tab says so first, and every part should be taken with \
               caution. A part's size is worked out from its label, so it is only as good as \
               the label. Do not read anything about anyone's health into the labels or the \
               sizes of the parts.",
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
               at scans on your own computer, for learning, teaching and research. It does \
               not diagnose, screen for or rule out anything, and its measurements and labels \
               are not clinical results.\n\n\
               Do not use it to decide anything about anyone's care, to read scans in a clinic \
               or hospital, or to plan a treatment or an operation. A report that came with a \
               scan was written by a qualified person who saw the whole of it and knew why it \
               was taken; Voxa knows neither. If you are worried about what a scan shows, or \
               about how you feel, ask a clinician. Do not wait for, or rely on, anything \
               Voxa shows.",
    },
];
