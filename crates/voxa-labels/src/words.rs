//! What is said of a label set in words: a volume, how far a name can be trusted, and what
//! the labelling model made of its own work.

use crate::table::{Trust, Verdict};

/// A volume as it is written: millilitres to one decimal place, or whole cubic millimetres
/// below one millilitre. Nothing for a volume that is not a number.
pub fn volume_in_words(ml: f64) -> String {
    if !ml.is_finite() {
        String::new()
    } else if ml >= 1.0 {
        format!("{ml:.1} ml")
    } else {
        format!("{:.0} mm³", ml * 1000.0)
    }
}

/// The word for how far a part's name can be trusted.
pub fn trust_in_words(trust: &Trust) -> String {
    let word = match trust {
        Trust::Model => "model",
        Trust::Approximate(_) => "approximate",
        Trust::NotNamed => "not named",
        Trust::NotSaid => "not said",
    };
    word.to_owned()
}

/// A labelling model's check of its own result, in a sentence or two. A failed check names
/// what scored low; a passed one says whose opinion it is.
pub fn verdict_in_words(verdict: &Verdict) -> String {
    let fails = verdict.fail_below;
    if verdict.passed && verdict.low.is_empty() {
        return format!(
            "The labelling model's own check passed: nothing scored below {fails}. That is the \
             model's opinion of its own work, not a check against anything else."
        );
    }
    let scored = |(name, score): &(String, f64)| format!("{name} ({score})");
    let low: Vec<String> = verdict.low.iter().map(scored).collect();
    let which = if low.is_empty() {
        String::new()
    } else {
        format!(" for {}", low.join(", "))
    };
    format!(
        "The labelling model's own check failed{which}; below {fails} fails. Take every part \
         here with caution: labels look as neat on a poor scan as on a good one."
    )
}
