//! A plugin's settings and what it accepts and gives, in plain words, as its card and its
//! entry show them.

use voxa_plugins::{
    Accepts, Contrasts, Fit, Gives, Input, LabelOutput, Output, Outside, Over, Scaling, Stage,
    Voxels,
};
use voxa_segment::Toward;

/// A number as it is written: to one decimal place, none when that place is nought.
fn number(value: f64) -> String {
    let written = format!("{value:.1}");
    match written.strip_suffix(".0") {
        Some(whole) => whole.to_owned(),
        None => written,
    }
}

/// Three numbers joined by times signs.
fn three<T: Copy>(values: [T; 3], each: impl Fn(T) -> String) -> String {
    values.map(each).join(" × ")
}

/// A percentile as an ordinal: 2nd, 98th, 2.5th.
fn ordinal(value: f64) -> String {
    let written = number(value);
    let whole = written.parse::<u64>().ok();
    let suffix = match whole.map(|whole| (whole % 10, whole % 100)) {
        Some((_, 11..=13)) | None => "th",
        Some((1, _)) => "st",
        Some((2, _)) => "nd",
        Some((3, _)) => "rd",
        Some(_) => "th",
    };
    format!("{written}{suffix}")
}

/// A direction as a word.
fn toward(toward: Toward) -> &'static str {
    match toward {
        Toward::Left => "left",
        Toward::Right => "right",
        Toward::Anterior => "front",
        Toward::Posterior => "back",
        Toward::Superior => "top",
        Toward::Inferior => "feet",
    }
}

/// Over which voxels, as words.
fn over(over: Over) -> &'static str {
    match over {
        Over::Scan => "the scan",
        Over::Grid => "the fitted box",
    }
}

/// Every preparation setting of a label-map plugin, and what its answer means, a sentence each.
pub fn settings_lines(input: &Input, output: &LabelOutput) -> Vec<String> {
    let mut lines = vec![
        format!("Resampled to {} mm.", three(input.spacing_mm, number)),
        format!("Turned towards the {}.", input.axes.map(toward).join(", ")),
        match input.fit {
            Fit::Whole(size) => format!(
                "Fitted to {} voxels, centred on the scan.",
                three(size, |side| side.to_string())
            ),
            Fit::Blocks { size, step } => format!(
                "Cut into blocks of {} voxels, {} apart.",
                three(size, |side| side.to_string()),
                three(step, |side| side.to_string())
            ),
        },
        match input.outside {
            Outside::Lowest => "Outside the scan: the scan's lowest value.".to_owned(),
            Outside::Zero => "Outside the scan: 0 after scaling.".to_owned(),
        },
        scaling(input.scaling),
    ];
    lines.push(match &output.gives {
        Gives::Scores { channels } => {
            format!(
                "Answer: the label of the highest of {} scores.",
                channels.len()
            )
        }
        Gives::Chance { label, threshold } => format!(
            "Answer: label {label} where the chance is above {}.",
            number(f64::from(*threshold))
        ),
        Gives::Labels => "Answer: labels as the network gives them.".to_owned(),
    });
    lines.push(if output.keep_largest_piece {
        "Only the largest connected piece is kept.".to_owned()
    } else {
        "Every labelled piece is kept.".to_owned()
    });
    lines
}

/// The scaling, as a sentence.
fn scaling(scaling: Scaling) -> String {
    match scaling {
        Scaling::Percentiles {
            low,
            high,
            over: of,
        } => format!(
            "Brightness: the {} to the {} percentile of {} becomes 0 to 1.",
            ordinal(low),
            ordinal(high),
            over(of)
        ),
        Scaling::ZScore { voxels, over: of } => format!(
            "Brightness: less the mean, divided by the standard deviation, of {} of {}.",
            match voxels {
                Voxels::All => "every voxel",
                Voxels::AboveZero => "the voxels above 0",
            },
            over(of)
        ),
        Scaling::Fixed {
            clip: [low, high],
            subtract,
            divide,
        } => format!(
            "Brightness: held from {} to {}, less {}, divided by {}.",
            number(low),
            number(high),
            number(subtract),
            number(divide)
        ),
        Scaling::None => "Brightness: as stored; the model scales it itself.".to_owned(),
    }
}

/// What scans a plugin accepts, as a phrase.
pub(super) fn accepts(accepts: &Accepts) -> String {
    let contrast = match &accepts.contrast {
        Contrasts::Any => "any contrast".to_owned(),
        Contrasts::Only(list) => list
            .iter()
            .map(|contrast| contrast.word())
            .collect::<Vec<_>>()
            .join(" or "),
    };
    let mut words = format!("{} scans, {contrast}", accepts.modality.join(" or "));
    if !accepts.body_part.is_empty() {
        words = format!("{words}, of the {}", accepts.body_part);
    }
    words
}

/// What a plugin gives, as a phrase.
pub(super) fn returns(output: &Output) -> String {
    match output {
        Output::Labels(labels) => match labels.names.len() {
            1 => "Labels: 1 named part.".to_owned(),
            many => format!("Labels: {many} named parts."),
        },
        Output::Number(number) => format!("One number: {}.", number.name),
        Output::ValueMap(_) => "A value map.".to_owned(),
        Output::Alignment(_) => "An alignment of two scans.".to_owned(),
    }
}

/// How far a run has got, as a sentence.
pub(super) fn stage(stage: Stage) -> String {
    match stage {
        Stage::Preparing => "Preparing the scan.".to_owned(),
        Stage::Running { block: 1, of: 1 } => "Running.".to_owned(),
        Stage::Running { block, of } => format!("Running block {block} of {of}."),
        Stage::Finishing => "Finishing.".to_owned(),
    }
}
