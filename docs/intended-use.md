# What Volsa is for

Volsa is for looking at MRI and CT scans, and other DICOM files at a basic level, on your own
computer: your own scans, and scans used for learning, teaching or research. It shows the
images, lets you measure them and can show labels made by a model, but it does not diagnose,
screen for or rule out anything, and its measurements and labels are not clinical results.
Questions about anyone's health belong with a qualified clinician. Volsa is not a medical device
and is not for diagnostic use.

## What it is not for

- **Deciding anything about anyone's care.** Not a diagnosis, a screening, a second opinion
  or a reason to wait. A scan's own report is written by a qualified person who has seen the
  whole of it, with the reason it was taken; Volsa knows neither.
- **Clinical work.** Volsa has not been made, tested or reviewed as a medical device, and no
  regulator has looked at it. It is not for reading scans in a clinic or a hospital, or for
  planning a treatment or an operation.
- **Measurements that something rests on.** A length, an angle, an area or a volume in Volsa is
  worked out from the numbers in the files. It is as good as those numbers, and as the line
  or region you drew. Nobody has checked it against a clinical standard.
- **Taking a model's labels as facts.** A label made by a model is a guess that looks
  certain. Volsa says which labels a model made, shows the model's own check of its work and
  says first when that check fails; a check that passes does not make the labels right.
- **Anything urgent.** If you are worried about what a scan shows, or about how you feel,
  ask a clinician. Do not wait for, or rely on, anything Volsa shows.

Volsa comes with no warranty, as its licences say: LICENSE-MIT and LICENSE-APACHE, at the top
of the source and beside this file in a download of Volsa.

## The labelling model's terms

Volsa labels a brain scan with SynthSeg 2.0's networks, which you make from SynthSeg's files
on your own computer; Volsa ships none and fetches none. Their trained weights come with
FreeSurfer, under the FreeSurfer Software License Agreement. That agreement says the software
is designed for research purposes only, that it has not been reviewed or approved by the US
Food and Drug Administration or any other agency, and that clinical applications are neither
recommended nor advised. Labels made with those weights come under the same caution.
docs/models.md in the source (MODELS.md in a download of Volsa) says more, under "The terms
of the model's weights", with where the agreement can be read.
