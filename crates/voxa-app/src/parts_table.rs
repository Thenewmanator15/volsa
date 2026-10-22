//! The labelled parts written out as a table for another program, and the two sides of a
//! part set against each other. Both say only what the label file and its table held:
//! nothing of the scan, and nothing Voxa worked out but a difference between two numbers.

use voxa_labels::{Side, Trust, difference_percent, pairs, trust_in_words, volume_in_words};

use crate::csv::{field, not_a_formula};
use crate::labels::Labels;

/// The file name offered when the parts are saved. It says nothing of the scan.
pub const PARTS_NAME: &str = "voxa-parts";
/// The columns of the table: what each part is and how large, then on every line which model
/// made the labels, how the model's own check of them went, and the notice that Voxa is not
/// for diagnostic use.
const HEADER: &str = "group,name,side,volume_ml,voxels,trust,how,model,model_check,notice";
/// What the `model` column says of labels that Voxa did not make.
const MADE_ELSEWHERE: &str = "made by another program";
/// What is said after any difference between the two sides of a part.
pub const NOISE: &str =
    "Voxa cannot tell how much of a difference like this is noise in the labelling.";

/// Which model made the labels, as the `model` column says it: the model Voxa ran, or that
/// another program made them.
fn model_of(labels: &Labels) -> String {
    match &labels.made {
        Some(made) if !made.model.is_empty() => made.model.clone(),
        Some(_) => "not said".to_owned(),
        None => MADE_ELSEWHERE.to_owned(),
    }
}

/// How the labelling model's own check of its work went, as the `model_check` column says it:
/// `passed` only when nothing scored below the line, as the Parts tab says it; `failed`; or
/// `not given` for labels that carry no check.
fn check_of(labels: &Labels) -> &'static str {
    match &labels.verdict {
        Some(verdict) if verdict.passed && verdict.low.is_empty() => "passed",
        Some(_) => "failed",
        None => "not given",
    }
}

/// The parts as a table any spreadsheet opens: one line of column names, then a line for
/// each part, hidden or not, in the order of the list. Fields are separated by commas as
/// RFC 4180 has it and lines end CR LF. `side` is empty for a part on the midline or with no
/// side said; `how` is what the labels' table says of a part that is approximate. The last
/// three columns are the same on every line, so that each line carries them wherever it is
/// copied, sorted or filtered: `model`, which model made the labels; `model_check`, whether
/// the model's own check of them failed; and `notice`, which holds `notice`, the sentence
/// that this is not for diagnostic use. A name that a spreadsheet would take for a formula is
/// written with an apostrophe before it.
pub fn parts_csv(labels: &Labels, notice: &str) -> String {
    let mut table = format!("{HEADER}\r\n");
    let model = field(&not_a_formula(&model_of(labels)));
    let notice = field(&not_a_formula(notice));
    let about = format!("{model},{},{notice}", check_of(labels));
    for part in &labels.parts {
        let side = match part.side {
            Side::Left => "left",
            Side::Right => "right",
            Side::Midline => "",
        };
        let how = match &part.trust {
            Trust::Approximate(how) => how.as_str(),
            _ => "",
        };
        let line = [
            field(&not_a_formula(&part.group)),
            field(&not_a_formula(&part.name)),
            side.to_owned(),
            format!("{:.3}", part.ml),
            part.voxels.to_string(),
            trust_in_words(&part.trust),
            field(&not_a_formula(how)),
            about.clone(),
        ];
        table.push_str(&line.join(","));
        table.push_str("\r\n");
    }
    table
}

/// The two sides of a part set against each other, for the part at this place in the list:
/// the volume of each, and how much larger or smaller the left is, as a percentage of the
/// mean of the two, which is how asymmetry is usually given. `None` for a part that has one
/// side only, or a side with nothing in it.
pub fn sides_compared(labels: &Labels, part: usize) -> Option<String> {
    let both = pairs(&labels.parts);
    let pair = both
        .iter()
        .find(|pair| pair.left == Some(part) || pair.right == Some(part))?;
    let (left, right) = (
        labels.parts.get(pair.left?)?,
        labels.parts.get(pair.right?)?,
    );
    if left.voxels == 0 || right.voxels == 0 {
        return None;
    }
    let apart = difference_percent(left.ml, right.ml)?.round();
    let sizes = format!(
        "Left {}, right {}",
        volume_in_words(left.ml),
        volume_in_words(right.ml)
    );
    if apart == 0.0 {
        return Some(format!("{sizes}: the same size."));
    }
    let which = if apart < 0.0 { "smaller" } else { "larger" };
    let by = apart.abs();
    Some(format!(
        "{sizes}: the left is {which} by {by}% of their mean. {NOISE}"
    ))
}

#[cfg(test)]
mod tests {
    use voxa_labels::Part;

    use super::*;
    use crate::labels::{LabelsStatus, PartShown};

    const NOTICE: &str = "Voxa is not a medical device and is not for diagnostic use.";

    /// A part of voxels a millimetre each way.
    fn part(name: &str, group: &str, side: Side, trust: Trust, voxels: u32) -> Part {
        Part {
            name: name.to_owned(),
            group: group.to_owned(),
            side,
            colour: [200, 120, 40],
            trust,
            voxels: u64::from(voxels),
            ml: f64::from(voxels) / 1000.0,
        }
    }

    fn labels_of(parts: Vec<Part>) -> Labels {
        let shown = PartShown {
            shown: true,
            opacity: 1.0,
        };
        Labels {
            status: LabelsStatus::Ready,
            shown: vec![shown; parts.len()],
            parts,
            ..Labels::default()
        }
    }

    fn labels() -> Labels {
        let rules = Trust::Approximate("Filled in by rules, not found.".to_owned());
        let deep = "Deep brain";
        labels_of(vec![
            part("hippocampus", deep, Side::Left, Trust::Model, 4000),
            part("hippocampus", deep, Side::Right, Trust::Model, 6000),
            part(
                "brain stem",
                "Brainstem",
                Side::Midline,
                Trust::Model,
                21_000,
            ),
            part("skull, \"outer\" table", "Bone", Side::Midline, rules, 12),
            part("=SUM(A1)", "", Side::Midline, Trust::NotNamed, 1),
            part("amygdala", deep, Side::Left, Trust::NotSaid, 1500),
            part("amygdala", deep, Side::Right, Trust::NotSaid, 1500),
        ])
    }

    #[test]
    fn the_table_has_a_line_for_every_part_hidden_or_not() {
        let mut labels = labels();
        labels.shown[1].shown = false;
        let table = parts_csv(&labels, NOTICE);
        let lines: Vec<&str> = table.split("\r\n").collect();
        let elsewhere = format!("made by another program,not given,{NOTICE}");
        let expected = [
            "group,name,side,volume_ml,voxels,trust,how,model,model_check,notice".to_owned(),
            format!("Deep brain,hippocampus,left,4.000,4000,model,,{elsewhere}"),
            format!("Deep brain,hippocampus,right,6.000,6000,model,,{elsewhere}"),
            format!("Brainstem,brain stem,,21.000,21000,model,,{elsewhere}"),
            format!(
                "Bone,\"skull, \"\"outer\"\" table\",,0.012,12,approximate,\"Filled in by rules, \
                 not found.\",{elsewhere}"
            ),
            format!(",'=SUM(A1),,0.001,1,not named,,{elsewhere}"),
            format!("Deep brain,amygdala,left,1.500,1500,not said,,{elsewhere}"),
            format!("Deep brain,amygdala,right,1.500,1500,not said,,{elsewhere}"),
            String::new(),
        ];
        assert_eq!(lines, expected);
        assert!(
            !table.replace("\r\n", "").contains(['\r', '\n']),
            "every line ends CR LF"
        );
    }

    #[test]
    fn the_table_holds_nothing_but_what_the_labels_held() {
        let mut labels = labels();
        labels.title = "A made-up head".to_owned();
        labels.study = Some(voxa_core::StudyId("1.2.3.4".to_owned()));
        let table = parts_csv(&labels, NOTICE);
        assert!(
            !table.contains("made-up") && !table.contains("1.2.3.4"),
            "{table}"
        );
        let empty = parts_csv(&Labels::default(), NOTICE);
        assert_eq!(
            empty,
            "group,name,side,volume_ml,voxels,trust,how,model,model_check,notice\r\n"
        );
        assert_eq!(
            PARTS_NAME, "voxa-parts",
            "the name offered says nothing of the scan"
        );
    }

    /// Two parts labelled by Voxa with a model, which judged its own work.
    fn made_by_voxa(passed: bool, low: Vec<(String, f64)>) -> Labels {
        let deep = "Deep brain";
        let mut labels = labels_of(vec![
            part("hippocampus", deep, Side::Left, Trust::Model, 4000),
            part("hippocampus", deep, Side::Right, Trust::Model, 6000),
        ]);
        labels.made = Some(voxa_labels::Made {
            by: "Voxa 0.5.0".to_owned(),
            model: "SynthSeg 2.0, robust, with cortex areas".to_owned(),
            device: "Made-up card".to_owned(),
            ..voxa_labels::Made::default()
        });
        labels.verdict = Some(voxa_labels::Verdict {
            passed,
            fail_below: 0.65,
            low,
        });
        labels
    }

    #[test]
    fn labels_voxa_made_say_on_every_line_which_model_made_them_and_how_its_check_went() {
        let failed = made_by_voxa(false, vec![("cortex".to_owned(), 0.52)]);
        let model = "\"SynthSeg 2.0, robust, with cortex areas\"";
        let expected = [
            "group,name,side,volume_ml,voxels,trust,how,model,model_check,notice".to_owned(),
            format!("Deep brain,hippocampus,left,4.000,4000,model,,{model},failed,{NOTICE}"),
            format!("Deep brain,hippocampus,right,6.000,6000,model,,{model},failed,{NOTICE}"),
            String::new(),
        ];
        let table = parts_csv(&failed, NOTICE);
        assert_eq!(table.split("\r\n").collect::<Vec<_>>(), expected);
        let passed = parts_csv(&made_by_voxa(true, Vec::new()), NOTICE);
        assert_eq!(passed.matches(",passed,").count(), 2, "{passed}");
        // A check that passed with something below the line is said, as on screen, to fail.
        let mixed = parts_csv(
            &made_by_voxa(true, vec![("cortex".to_owned(), 0.6)]),
            NOTICE,
        );
        assert_eq!(mixed.matches(",failed,").count(), 2, "{mixed}");
        let mut unnamed = made_by_voxa(true, Vec::new());
        unnamed.made = Some(voxa_labels::Made::default());
        unnamed.verdict = None;
        let unnamed = parts_csv(&unnamed, NOTICE);
        assert_eq!(
            unnamed.matches(",not said,not given,").count(),
            2,
            "{unnamed}"
        );
    }

    #[test]
    fn a_part_with_two_sides_is_set_left_against_right() {
        let labels = labels();
        let smaller =
            format!("Left 4.0 ml, right 6.0 ml: the left is smaller by 40% of their mean. {NOISE}");
        assert_eq!(sides_compared(&labels, 0), Some(smaller.clone()));
        assert_eq!(
            sides_compared(&labels, 1),
            Some(smaller),
            "whichever side is selected"
        );
        let same = "Left 1.5 ml, right 1.5 ml: the same size.";
        assert_eq!(sides_compared(&labels, 5).as_deref(), Some(same));
        assert!(NOISE.contains("cannot tell") && NOISE.contains("noise in the labelling"));
    }

    #[test]
    fn the_larger_side_is_said_to_be_larger() {
        let deep = "Deep brain";
        let labels = labels_of(vec![
            part("hippocampus", deep, Side::Right, Trust::Model, 4000),
            part("hippocampus", deep, Side::Left, Trust::Model, 6000),
        ]);
        let larger =
            format!("Left 6.0 ml, right 4.0 ml: the left is larger by 40% of their mean. {NOISE}");
        assert_eq!(sides_compared(&labels, 0), Some(larger));
    }

    #[test]
    fn a_part_with_one_side_or_an_empty_side_is_not_compared() {
        let labels = labels();
        assert_eq!(sides_compared(&labels, 2), None, "on the midline");
        assert_eq!(sides_compared(&labels, 40), None, "no such part");
        let one = labels_of(vec![part(
            "eyeball",
            "Eyes",
            Side::Left,
            Trust::Model,
            6000,
        )]);
        assert_eq!(
            sides_compared(&one, 0),
            None,
            "its other side is not labelled"
        );
        let empty = labels_of(vec![
            part("eyeball", "Eyes", Side::Left, Trust::Model, 6000),
            part("eyeball", "Eyes", Side::Right, Trust::Model, 0),
        ]);
        assert_eq!(sides_compared(&empty, 0), None, "nothing to set it against");
    }
}
