//! The `input` part of a description: the plugin's grid, the size of what the network is
//! given, what lies outside the scan, and how the brightness is scaled.

use serde_json::Value;
use voxa_segment::Toward;

use super::fields::Obj;
use super::{DescribeError, Fit, Input, Layout, Outside, Over, Scaling, Voxels};

/// The longest side of a box or a block, in voxels.
const LONGEST_SIDE: u64 = 1024;

/// Reads `input`.
pub(super) fn input(top: &Obj) -> Result<Input, DescribeError> {
    let input = top.object("input")?;
    let spacing_mm = spacing(&input)?;
    let axes = Toward::code(input.word("axes")?).ok_or_else(|| {
        let why = "must be three letters, one each of L or R, A or P, and S or I";
        input.wrong("axes", why)
    })?;
    let layout = match input.word("layout")? {
        "channels-first" => Layout::ChannelsFirst,
        "channels-last" => Layout::ChannelsLast,
        _ => return Err(input.wrong("layout", "is not one Voxa knows")),
    };
    let (fit, outside) = fit(&input)?;
    let scaling = scaling(&input, fit, outside)?;
    input.only(&["spacing_mm", "axes", "layout", "fit", "scaling"])?;
    Ok(Input {
        spacing_mm,
        axes,
        layout,
        fit,
        outside,
        scaling,
    })
}

/// Three numbers above 0.
fn spacing(input: &Obj) -> Result<[f64; 3], DescribeError> {
    let wrong = || input.wrong("spacing_mm", "must be three numbers above 0");
    let numbers = three(input.value("spacing_mm")?, |value| {
        value.as_f64().filter(|mm| mm.is_finite() && *mm > 0.0)
    });
    numbers.ok_or_else(wrong)
}

/// Three values of a list of exactly three, each read by `read`.
fn three<T: Copy>(value: &Value, read: impl Fn(&Value) -> Option<T>) -> Option<[T; 3]> {
    match value.as_array()?.as_slice() {
        [a, b, c] => Some([read(a)?, read(b)?, read(c)?]),
        _ => None,
    }
}

/// Three sides of a box or a block: whole numbers from 1 to 1024.
fn sides(group: &Obj, key: &str) -> Result<[usize; 3], DescribeError> {
    let side = |value: &Value| {
        let side = value
            .as_u64()
            .filter(|side| (1..=LONGEST_SIDE).contains(side))?;
        usize::try_from(side).ok()
    };
    three(group.value(key)?, side)
        .ok_or_else(|| group.wrong(key, "must be three whole numbers from 1 to 1024"))
}

/// Reads `fit`: a whole box or blocks, and what lies outside the scan.
fn fit(input: &Obj) -> Result<(Fit, Outside), DescribeError> {
    let group = input.object("fit")?;
    let fit = match (group.get("whole"), group.get("blocks")) {
        (Some(_), None) => Fit::Whole(sides(&group, "whole")?),
        (None, Some(_)) => {
            let blocks = group.object("blocks")?;
            let size = sides(&blocks, "size")?;
            let step = sides(&blocks, "step")?;
            if step.iter().zip(&size).any(|(step, size)| step > size) {
                return Err(blocks.wrong("step", "must be from 1 to the block's size"));
            }
            blocks.only(&["size", "step"])?;
            Fit::Blocks { size, step }
        }
        _ => {
            let why = "must hold either whole or blocks";
            return Err(DescribeError::Field {
                field: input.named("fit"),
                why,
            });
        }
    };
    let outside = match group.word("outside")? {
        "lowest" => Outside::Lowest,
        "zero" => Outside::Zero,
        _ => return Err(group.wrong("outside", "is not one Voxa knows")),
    };
    group.only(&["whole", "blocks", "outside"])?;
    Ok((fit, outside))
}

/// Reads `scaling`.
fn scaling(input: &Obj, fit: Fit, outside: Outside) -> Result<Scaling, DescribeError> {
    let group = input.object("scaling")?;
    let (scaling, keys): (Scaling, &[&str]) = match group.word("kind")? {
        "percentiles" => {
            let (low, high) = (group.number("low")?, group.number("high")?);
            if !(0.0 <= low && low < high && high <= 100.0) {
                let field = input.named("scaling");
                let why = "must have 0 <= low < high <= 100";
                return Err(DescribeError::Field { field, why });
            }
            let over = over(&group, fit, outside)?;
            let scaling = Scaling::Percentiles { low, high, over };
            (scaling, &["kind", "low", "high", "over"])
        }
        "z-score" => {
            let voxels = match group.word("voxels")? {
                "all" => Voxels::All,
                "above-zero" => Voxels::AboveZero,
                _ => return Err(group.wrong("voxels", "is not one Voxa knows")),
            };
            let over = over(&group, fit, outside)?;
            (
                Scaling::ZScore { voxels, over },
                &["kind", "voxels", "over"],
            )
        }
        "fixed" => (fixed(&group)?, &["kind", "clip", "subtract", "divide"]),
        "none" => (Scaling::None, &["kind"]),
        _ => return Err(group.wrong("kind", "is not one Voxa knows")),
    };
    group.only(keys)?;
    Ok(scaling)
}

/// Reads `over`: grid statistics need a whole box with the lowest value outside it.
fn over(group: &Obj, fit: Fit, outside: Outside) -> Result<Over, DescribeError> {
    match group.word("over")? {
        "scan" => Ok(Over::Scan),
        "grid" if matches!(fit, Fit::Whole(_)) && outside == Outside::Lowest => Ok(Over::Grid),
        "grid" => {
            let why = "can be grid only with a whole box and the lowest value outside";
            Err(group.wrong("over", why))
        }
        _ => Err(group.wrong("over", "is not one Voxa knows")),
    }
}

/// Reads a fixed scaling: a clip from low to high, and a divisor that is not 0.
fn fixed(group: &Obj) -> Result<Scaling, DescribeError> {
    let clip = pair(group, "clip")?;
    let subtract = group.number("subtract")?;
    let divide = group.number("divide")?;
    if divide == 0.0 {
        return Err(group.wrong("divide", "must not be 0"));
    }
    Ok(Scaling::Fixed {
        clip,
        subtract,
        divide,
    })
}

/// Two finite numbers, the first no more than the second.
pub(super) fn pair(group: &Obj, key: &str) -> Result<[f64; 2], DescribeError> {
    let wrong = || group.wrong(key, "must be two numbers, the first below the second");
    let list = group.list(key)?;
    let numbers: Option<Vec<f64>> = list
        .iter()
        .map(|value| value.as_f64().filter(|number| number.is_finite()))
        .collect();
    match numbers.as_deref() {
        Some(&[low, high]) if low < high => Ok([low, high]),
        _ => Err(wrong()),
    }
}
