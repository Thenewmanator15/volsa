//! What the tables Voxa writes (CSV) have in common: how a field is written.

/// One field of the table: in quotation marks, with any of its own doubled, when it holds a
/// comma, a quotation mark or a line break.
pub fn field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// A name as it is safe to hand a spreadsheet: one that begins like a formula is given an
/// apostrophe first, which a spreadsheet reads as "this is text".
pub fn not_a_formula(label: &str) -> String {
    if label.starts_with(['=', '+', '-', '@']) {
        format!("'{label}")
    } else {
        label.to_owned()
    }
}
