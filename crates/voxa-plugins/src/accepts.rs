//! Whether a plugin is run on a scan, and what is said on the result (design 5.3, decided by
//! the owner): a scan of a modality the plugin does not list is refused, since Voxa knows a
//! scan's modality for certain; a scan whose description names a contrast the plugin does not
//! accept is warned of, and run only when the person confirms, the mismatch stamped on the
//! result; a scan whose description names no contrast is run, stamped "contrast not
//! confirmed". Voxa reads a contrast only from the description's words.

use crate::{Accepts, Contrast, Contrasts};

mod words;

#[cfg(test)]
mod tests;

/// The stamp on a result run on a scan whose description names no contrast.
pub const UNSAID: &str = "Contrast not confirmed: the scan's description does not say.";

/// What is done with a request to run a plugin on a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Not run: the sentence says why.
    Refuse(String),
    /// Run only if the person confirms after the warning; the stamp goes on the result.
    Warn {
        /// What the person is told before.
        warning: String,
        /// What the result carries after.
        stamp: String,
    },
    /// Run, the note, if any, going on the result.
    Run {
        /// What the result carries.
        note: Option<String>,
    },
}

/// The contrasts a series' description names, by the redesign's reading of it.
pub fn contrasts_named(description: &str) -> Vec<Contrast> {
    words::weightings_in(description)
        .into_iter()
        .filter_map(Contrast::of_word)
        .collect()
}

/// What is done with a request to run a plugin that `accepts` these scans on a scan of this
/// DICOM `modality` and series `description`.
pub fn decide(modality: &str, description: &str, accepts: &Accepts) -> Decision {
    let code = modality.trim().to_ascii_uppercase();
    let wanted = accepts.modality.join(" or ");
    if code.is_empty() {
        return Decision::Refuse(format!(
            "This plugin is for {wanted} scans; this scan does not say what kind it is."
        ));
    }
    if !accepts.modality.contains(&code) {
        return Decision::Refuse(format!(
            "This plugin is for {wanted} scans; this scan is {code}."
        ));
    }
    let Contrasts::Only(accepted) = &accepts.contrast else {
        return Decision::Run { note: None };
    };
    let named = contrasts_named(description);
    if named.is_empty() {
        return Decision::Run {
            note: Some(UNSAID.to_owned()),
        };
    }
    let takes = |contrast: &&Contrast| {
        accepted.contains(contrast)
            || (**contrast == Contrast::T1In3d && accepted.contains(&Contrast::T1))
    };
    if let Some(matched) = named.iter().find(takes) {
        let note = format!(
            "The scan's description says {}, as this plugin expects.",
            matched.word()
        );
        return Decision::Run { note: Some(note) };
    }
    let expects: Vec<&str> = accepted.iter().map(|contrast| contrast.word()).collect();
    let says: Vec<&str> = named.iter().map(|contrast| contrast.word()).collect();
    let (expects, says) = (expects.join(" or "), says.join(" and "));
    Decision::Warn {
        warning: format!("This plugin expects {expects}; this scan's description says {says}."),
        stamp: format!(
            "Run after a warning: this plugin expects {expects}; this scan's description says \
             {says}."
        ),
    }
}

/// The note on a result run on a scan whose voxels are further apart than the plugin says it
/// was made for, naming both distances; `None` otherwise.
pub fn spacing_note(spacing_mm: [f64; 3], thickest_mm: Option<f64>) -> Option<String> {
    let thickest = thickest_mm?;
    let widest = spacing_mm.iter().copied().fold(0.0, f64::max);
    (widest > thickest).then(|| {
        format!(
            "The voxels of this scan are {} mm apart; this plugin states it was made for up to {} \
             mm.",
            in_mm(widest),
            in_mm(thickest)
        )
    })
}

/// A distance in millimetres as it is written: to one decimal place, none when it is nought.
pub(crate) fn in_mm(mm: f64) -> String {
    let written = format!("{mm:.1}");
    match written.strip_suffix(".0") {
        Some(whole) => whole.to_owned(),
        None => written,
    }
}
