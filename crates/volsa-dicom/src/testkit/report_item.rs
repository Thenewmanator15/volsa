//! The content of a made-up report.

/// One item of the content of a made-up report. Each has a concept: the name of what it says.
#[derive(Debug, Clone)]
pub enum ReportItem {
    /// A section: a heading, with items under it.
    Container {
        /// The section's name.
        concept: String,
        /// What is in the section.
        items: Vec<ReportItem>,
    },
    /// An item whose value is text of a plain kind.
    Plain {
        /// `TEXT`, `DATE`, `TIME`, `DATETIME`, `PNAME` or `UIDREF`.
        value_type: String,
        /// What the value is of.
        concept: String,
        /// The value, written as the file would write it.
        value: String,
    },
    /// A number with its unit.
    Number {
        /// What was measured.
        concept: String,
        /// The number, written as the file would write it.
        value: String,
        /// The meaning of the unit's code.
        units: String,
    },
    /// A value chosen from a coding scheme.
    Code {
        /// What the value is of.
        concept: String,
        /// The meaning of the code.
        meaning: String,
    },
    /// An item of a kind that is not written out as text, such as `IMAGE`.
    Unshown {
        /// The item's value type.
        value_type: String,
        /// What the item is of.
        concept: String,
    },
}

impl ReportItem {
    /// A section called `concept` holding `items`.
    pub fn container(concept: &str, items: Vec<ReportItem>) -> Self {
        Self::Container {
            concept: concept.to_owned(),
            items,
        }
    }

    /// An item of the plain kind `value_type`, such as `TEXT`.
    pub fn plain(value_type: &str, concept: &str, value: &str) -> Self {
        Self::Plain {
            value_type: value_type.to_owned(),
            concept: concept.to_owned(),
            value: value.to_owned(),
        }
    }

    /// A text item.
    pub fn text(concept: &str, value: &str) -> Self {
        Self::plain("TEXT", concept, value)
    }

    /// A number with its unit.
    pub fn number(concept: &str, value: &str, units: &str) -> Self {
        Self::Number {
            concept: concept.to_owned(),
            value: value.to_owned(),
            units: units.to_owned(),
        }
    }

    /// A coded value.
    pub fn code(concept: &str, meaning: &str) -> Self {
        Self::Code {
            concept: concept.to_owned(),
            meaning: meaning.to_owned(),
        }
    }

    /// An item of a kind that is not written out as text.
    pub fn unshown(value_type: &str, concept: &str) -> Self {
        Self::Unshown {
            value_type: value_type.to_owned(),
            concept: concept.to_owned(),
        }
    }
}
